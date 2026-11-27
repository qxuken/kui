package dev.kui.editingime;

import android.service.textservice.SpellCheckerService;
import android.view.textservice.SuggestionsInfo;
import android.view.textservice.TextInfo;

/** The Android framework splits the sentence and reconstructs its UTF-16 offsets. */
public final class EditingSpellChecker extends SpellCheckerService {
    @Override public Session createSession() {
        return new Session() {
            @Override public void onCreate() {}
            @Override public SuggestionsInfo onGetSuggestions(TextInfo input, int limit) {
                if (input.getText().startsWith("kuizzz")
                    || ("кошка".equals(input.getText()) && !getLocale().startsWith("ru"))) {
                    return new SuggestionsInfo(SuggestionsInfo.RESULT_ATTR_LOOKS_LIKE_TYPO,
                        new String[]{"quiz"});
                }
                return new SuggestionsInfo(SuggestionsInfo.RESULT_ATTR_IN_THE_DICTIONARY, new String[0]);
            }
        };
    }
}
