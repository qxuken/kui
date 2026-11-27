package dev.kui;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.Handler;
import android.os.HandlerThread;
import android.os.LocaleList;
import android.os.SystemClock;
import android.view.textservice.SentenceSuggestionsInfo;
import android.view.textservice.SpellCheckerSession;
import android.view.textservice.SuggestionsInfo;
import android.view.textservice.TextInfo;
import android.view.textservice.TextServicesManager;
import java.text.BreakIterator;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.TreeMap;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

/** One native session per supported writing language. JNI calls run on KUI's spelling worker. */
final class KuiSpelling {
    private static final int MAX_TEXT = 4096, MAX_CHOICES = 6;
    private final Context context;
    private final SharedPreferences personal;
    private final HandlerThread callbacks = new HandlerThread("kui-text-services");
    private final Handler handler;
    // The Android listener delivers results to the Looper on which the session was opened.
    private final List<Channel> channels = new ArrayList<>();
    private boolean opened;
    private long retryAfter;

    KuiSpelling(Context context) {
        this.context = context;
        personal = context.getSharedPreferences("kui.spelling", Context.MODE_PRIVATE);
        callbacks.start();
        handler = new Handler(callbacks.getLooper());
    }

    private static final class Channel implements SpellCheckerSession.SpellCheckerSessionListener {
        SpellCheckerSession session;
        CompletableFuture<SentenceSuggestionsInfo> reply;

        @Override public void onGetSuggestions(SuggestionsInfo[] unused) {}
        @Override public void onGetSentenceSuggestions(SentenceSuggestionsInfo[] results) {
            if (reply != null) {
                reply.complete(results != null && results.length == 1 ? results[0] : null);
                reply = null;
            }
        }
    }

    private boolean add(TextServicesManager manager, Locale locale, boolean useSettings) {
        Channel channel = new Channel();
        channel.session = manager.newSpellCheckerSession(null, locale, channel, useSettings);
        if (channel.session == null) return false;
        channels.add(channel);
        return true;
    }

    private void open() {
        opened = true;
        TextServicesManager manager = context.getSystemService(TextServicesManager.class);
        if (manager == null) return;
        LocaleList preferred = context.getResources().getConfiguration().getLocales();
        Set<String> languages = new HashSet<>();
        // The settings-aware request also respects the user's system spelling switch.
        for (int i = 0; i < preferred.size(); i++) {
            Locale locale = preferred.get(i);
            if (add(manager, locale, true)) {
                languages.add(locale.getLanguage());
                break;
            }
        }
        if (channels.isEmpty() && !add(manager, null, true)) return;
        for (int i = 0; i < preferred.size(); i++) {
            Locale locale = preferred.get(i);
            if (!languages.contains(locale.getLanguage()) && add(manager, locale, false)) {
                languages.add(locale.getLanguage());
            }
        }
    }

    private void reset() {
        for (Channel channel : channels) {
            channel.reply = null;
            channel.session.close();
        }
        channels.clear();
        opened = false;
    }

    private List<SentenceSuggestionsInfo> query(String text, int limit) {
        CompletableFuture<List<SentenceSuggestionsInfo>> result = new CompletableFuture<>();
        handler.post(() -> {
            try {
                if (!opened && SystemClock.uptimeMillis() >= retryAfter) open();
                List<CompletableFuture<SentenceSuggestionsInfo>> replies = new ArrayList<>();
                TextInfo[] input = {new TextInfo(text)};
                for (Channel channel : channels) {
                    channel.reply = new CompletableFuture<>();
                    replies.add(channel.reply);
                    channel.session.getSentenceSuggestions(input, limit);
                }
                CompletableFuture.allOf(replies.toArray(new CompletableFuture<?>[0])).thenRun(() -> {
                    List<SentenceSuggestionsInfo> answers = new ArrayList<>();
                    for (CompletableFuture<SentenceSuggestionsInfo> reply : replies) answers.add(reply.join());
                    result.complete(answers);
                });
            } catch (RuntimeException e) {
                result.completeExceptionally(e);
            }
        });
        try {
            return result.get(3, TimeUnit.SECONDS);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        } catch (ExecutionException | TimeoutException ignored) {
            // A disconnected service may never reply. Retire its listeners so late replies
            // cannot be mistaken for the next paragraph. Back off before binding again.
        }
        handler.post(() -> {
            reset();
            retryAfter = SystemClock.uptimeMillis() + 30000;
        });
        return Collections.emptyList();
    }

    private boolean typo(SuggestionsInfo info) {
        if (info == null) return false;
        int flags = info.getSuggestionsAttributes();
        return (flags & SuggestionsInfo.RESULT_ATTR_LOOKS_LIKE_TYPO) != 0
            && (flags & (SuggestionsInfo.RESULT_ATTR_IN_THE_DICTIONARY
                        | SuggestionsInfo.RESULT_ATTR_DONT_SHOW_UI_FOR_SUGGESTIONS)) == 0;
    }

    private boolean known(String word) {
        return personal.contains(word.toLowerCase(Locale.ROOT));
    }

    private void checkChunk(String text, int base, List<Integer> offsets) {
        TreeMap<Integer, Integer> common = null;
        for (SentenceSuggestionsInfo answer : query(text, 1)) {
            TreeMap<Integer, Integer> ranges = new TreeMap<>();
            if (answer == null) return;
            for (int i = 0; i < answer.getSuggestionsCount(); i++) {
                int start = answer.getOffsetAt(i), length = answer.getLengthAt(i);
                if (typo(answer.getSuggestionsInfoAt(i)) && start >= 0 && length > 0
                    && start <= text.length() - length && !known(text.substring(start, start + length))) {
                    ranges.put(start, start + length);
                }
            }
            if (common == null) common = ranges;
            else common.entrySet().retainAll(ranges.entrySet());
            if (common.isEmpty()) return;
        }
        if (common != null) common.forEach((start, end) -> {
            offsets.add(base + start);
            offsets.add(base + end);
        });
    }

    int[] check(String text) {
        List<Integer> offsets = new ArrayList<>();
        BreakIterator words = null;
        for (int start = 0; start < text.length();) {
            int end = text.length();
            if (end - start > MAX_TEXT) {
                if (words == null) {
                    words = BreakIterator.getWordInstance();
                    words.setText(text);
                }
                end = words.preceding(start + MAX_TEXT + 1);
                if (end <= start) {
                    // One token cannot fit in a Binder request without splitting it.
                    start = words.following(start + MAX_TEXT);
                    if (start == BreakIterator.DONE) break;
                    continue;
                }
            }
            checkChunk(text.substring(start, end), start, offsets);
            start = end;
        }
        int[] result = new int[offsets.size()];
        for (int i = 0; i < offsets.size(); i++) result[i] = offsets.get(i);
        return result;
    }

    // null means the word is no longer marked, an empty array still allows learning it.
    String[] choices(String word) {
        if (word.isEmpty() || word.length() > MAX_TEXT || known(word)) return null;
        List<SentenceSuggestionsInfo> answers = query(word, MAX_CHOICES);
        if (answers.isEmpty()) return null;
        Set<String> choices = new LinkedHashSet<>();
        for (SentenceSuggestionsInfo answer : answers) {
            if (answer == null) return null;
            boolean rejected = false;
            for (int i = 0; i < answer.getSuggestionsCount(); i++) {
                SuggestionsInfo info = answer.getSuggestionsInfoAt(i);
                if (!typo(info) || answer.getOffsetAt(i) != 0 || answer.getLengthAt(i) != word.length()) continue;
                rejected = true;
                for (int j = 0; j < info.getSuggestionsCount() && choices.size() < MAX_CHOICES; j++) {
                    String choice = info.getSuggestionAt(j);
                    if (choice != null && !choice.isEmpty() && !choice.equals(word)) choices.add(choice);
                }
            }
            if (!rejected) return null;
        }
        return choices.toArray(new String[0]);
    }

    boolean learn(String word) {
        // Android 6+ restricts UserDictionary writes to IMEs and spellchecker services.
        return !word.isEmpty() && personal.edit().putBoolean(word.toLowerCase(Locale.ROOT), true).commit();
    }

    void close() {
        handler.post(() -> {
            try { reset(); }
            finally { callbacks.quitSafely(); }
        });
    }
}
