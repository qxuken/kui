#import <UIKit/UIKit.h>

static UIView *responder(UIView *view) {
    if (view.isFirstResponder) return view;
    for (UIView *child in view.subviews) {
        UIView *found = responder(child);
        if (found) return found;
    }
    return nil;
}

#include <stdatomic.h>
static atomic_size_t openCalls, firstLanguages, lastLanguages, checkCalls, lastChecked, lastRaw, lastEmitted;
static atomic_size_t refreshCalls, lastSamples, lastReady;

void kui_probe_backend_counts(int stage, size_t a, size_t b, size_t c) {
    if (stage == 0) {
        if (atomic_fetch_add(&openCalls, 1) == 0) atomic_store(&firstLanguages, a);
        atomic_store(&lastLanguages, a);
    } else if (stage == 1) {
        atomic_fetch_add(&checkCalls, 1);
        atomic_store(&lastChecked, a);
        atomic_store(&lastRaw, b);
        atomic_store(&lastEmitted, c);
    } else {
        atomic_fetch_add(&refreshCalls, 1);
        atomic_store(&lastSamples, b);
        atomic_store(&lastReady, c);
    }
}

extern void *kui_ios_spell_open(void);
extern void kui_ios_spell_close(void *);
extern void kui_ios_spell_check(void *, const uint8_t *, size_t, void *, void (*)(void *,size_t,size_t));
static void probeUTF16(void *ctx, size_t start, size_t end) {
    [(__bridge NSMutableArray *)ctx addObject:NSStringFromRange(NSMakeRange(start, end - start))];
}
extern NSArray<NSString *> *kui_ios_spell_languages(void);

extern NSIndexSet *kui_ios_spell_consensus(NSString *, NSDictionary *, NSDictionary *);

// Exercise missing and late-arriving assets without changing the user's
// keyboards. UIKit supplies the real alphabets for these writing systems.
bool kui_probe_spelling_scripts(void) {
    NSString *text = @"wrng خطأ";
    NSIndexSet *englishError = [NSIndexSet indexSetWithIndexesInRange:NSMakeRange(0, 4)];
    NSIndexSet *both = [NSIndexSet indexSetWithIndexesInRange:NSMakeRange(0, text.length)];
    NSMutableIndexSet *reported = [both mutableCopy];
    [reported removeIndex:4];
    NSCharacterSet *latin = [NSLocale localeWithLocaleIdentifier:@"en_US"].exemplarCharacterSet;
    NSCharacterSet *arabic = [NSLocale localeWithLocaleIdentifier:@"ar"].exemplarCharacterSet;
    NSDictionary *mixed = @{@"en":latin, @"ar":arabic}, *related = @{@"en":latin, @"fr":latin};
    NSDictionary *onlyEnglish = @{@"en":reported};
    return [kui_ios_spell_consensus(text, mixed, onlyEnglish) isEqual:englishError]
        && [kui_ios_spell_consensus(text, mixed, @{@"en":reported, @"ar":[NSIndexSet indexSet]})
               isEqual:englishError]
        && !kui_ios_spell_consensus(text, related, onlyEnglish).count
        && [kui_ios_spell_consensus(text, related, @{@"en":reported, @"fr":englishError})
               isEqual:englishError];
}

// This fixture is short. Mark each UTF-16 unit rejected by each configured
// UIKit dictionary, then convert the common runs using NSString itself.
void kui_probe_expected_ranges(const uint8_t *bytes, size_t len, void *ctx,
                               void (*emit)(void *, size_t, size_t)) {
    NSString *text = [[NSString alloc] initWithBytes:bytes length:len encoding:NSUTF8StringEncoding];
    NSArray *languages = kui_ios_spell_languages();
    if (!languages.count || !text.length) return;
    UITextChecker *checker = [UITextChecker new];
    NSMutableData *votes = [NSMutableData dataWithLength:text.length * sizeof(NSUInteger)];
    NSUInteger *rejected = votes.mutableBytes;
    NSRange full = NSMakeRange(0, text.length);
    for (NSString *language in languages) {
        NSUInteger cursor = 0;
        while (cursor < full.length) {
            NSRange error = [checker rangeOfMisspelledWordInString:text range:full startingAt:cursor
                                                              wrap:NO language:language];
            if (error.location == NSNotFound || error.location < cursor || !error.length ||
                NSMaxRange(error) > full.length) break;
            for (NSUInteger i = error.location; i < NSMaxRange(error); i++) rejected[i]++;
            cursor = NSMaxRange(error);
        }
    }
    for (NSUInteger i = 0; i < full.length;) {
        if (rejected[i] != languages.count) { i++; continue; }
        NSUInteger start = i++;
        while (i < full.length && rejected[i] == languages.count) i++;
        emit(ctx, [[text substringToIndex:start] lengthOfBytesUsingEncoding:NSUTF8StringEncoding],
                  [[text substringToIndex:i] lengthOfBytesUsingEncoding:NSUTF8StringEncoding]);
    }
}

// Read UIKit independently of the KUI worker when a spelling assertion fails.
// Installed keyboard and spelling languages vary between simulator images.
void kui_probe_spelling(const uint8_t *bytes, size_t len, void *ctx,
                        void (*report)(void *, const char *)) {
    NSString *text = [[NSString alloc] initWithBytes:bytes length:len encoding:NSUTF8StringEncoding];
    UITextChecker *checker = [UITextChecker new];
    NSMutableArray *keyboards = [NSMutableArray array];
    for (UITextInputMode *keyboard in UITextInputMode.activeInputModes)
        if (keyboard.primaryLanguage) [keyboards addObject:keyboard.primaryLanguage];
    NSMutableDictionary *ranges = [NSMutableDictionary dictionary];
    for (NSString *language in UITextChecker.availableLanguages) {
        NSMutableArray *answer = [NSMutableArray array];
        NSUInteger cursor = 0;
        while (cursor < text.length) {
            NSRange miss = [checker rangeOfMisspelledWordInString:text range:NSMakeRange(0,text.length)
                     startingAt:cursor wrap:NO language:language];
            if (miss.location == NSNotFound || NSMaxRange(miss) <= cursor) break;
            [answer addObject:NSStringFromRange(miss)];
            cursor = NSMaxRange(miss);
        }
        ranges[language] = answer;
    }
    NSString *worker = [NSString stringWithFormat:
        @"open=%zu first/last languages=%zu/%zu checks=%zu last checked/raw/emitted=%zu/%zu/%zu refreshes=%zu last samples/ready=%zu/%zu",
        atomic_load(&openCalls), atomic_load(&firstLanguages), atomic_load(&lastLanguages), atomic_load(&checkCalls),
        atomic_load(&lastChecked), atomic_load(&lastRaw), atomic_load(&lastEmitted),
        atomic_load(&refreshCalls), atomic_load(&lastSamples), atomic_load(&lastReady)];
    NSMutableArray *fresh = [NSMutableArray array];
    void *handle = kui_ios_spell_open();
    if (handle) {
        kui_ios_spell_check(handle, bytes, len, (__bridge void *)fresh, probeUTF16);
        kui_ios_spell_close(handle);
    }
    NSString *summary = [NSString stringWithFormat:@"Production worker %@ fresh bridge UTF16=%@ UIKit preferred=%@ keyboards=%@ configured=%@ nativeUTF16Ranges=%@",
                         worker, fresh, NSLocale.preferredLanguages, keyboards, kui_ios_spell_languages(), ranges];
    report(ctx, summary.UTF8String);
}

bool kui_probe_guess(const uint8_t *bytes, size_t len, void *ctx, void (*emit)(void *, const char *)) {
    NSString *word = [[NSString alloc] initWithBytes:bytes length:len encoding:NSUTF8StringEncoding];
    if ([UITextChecker hasLearnedWord:word]) return false;
    UITextChecker *checker = [UITextChecker new];
    for (NSString *language in kui_ios_spell_languages()) {
        for (NSString *guess in [checker guessesForWordRange:NSMakeRange(0, word.length)
                                                   inString:word language:language]) {
            if (guess.length && ![guess isEqualToString:word]) {
                emit(ctx, guess.UTF8String);
                return true;
            }
        }
    }
    return true;
}

bool kui_probe_unlearn(const uint8_t *bytes, size_t len) {
    NSString *word = [[NSString alloc] initWithBytes:bytes length:len encoding:NSUTF8StringEncoding];
    if (![UITextChecker hasLearnedWord:word]) return false;
    [UITextChecker unlearnWord:word];
    return ![UITextChecker hasLearnedWord:word];
}

bool kui_probe_step(int step) {
    static UITextView *previous;
    UITextView *field = nil;
    UIView *keyView = nil;
    for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
        if (![scene isKindOfClass:UIWindowScene.class]) continue;
        for (UIWindow *window in ((UIWindowScene *)scene).windows) {
            UIView *view = responder(window);
            if ([view isKindOfClass:UITextView.class]) field = (UITextView *)view;
            if ([view isKindOfClass:NSClassFromString(@"KUIKeyView")]) keyView = view;
        }
    }
    if (step == 12) return field == nil && keyView != nil;
    if (!field) return false;
    switch (step) {
        case 1:
            if (![field.text isEqualToString:@"a🙂b"]) return false;
            field.selectedRange = NSMakeRange(1, 2);
            break;
        case 2:
            if (!NSEqualRanges(field.selectedRange, NSMakeRange(1, 2))) return false;
            [field insertText:@"é"];
            break;
        case 3:
            if (!NSEqualRanges(field.selectedRange, NSMakeRange(2, 0))) return false;
            [field insertText:@"!"];
            break;
        case 4:
            if (![field.text isEqualToString:@"second"]) return false;
            field.selectedRange = NSMakeRange(6, 0);
            [field setMarkedText:@"か" selectedRange:NSMakeRange(1, 0)];
            previous = field;
            break;
        case 5:
            if (field == previous || ![field.text isEqualToString:@"aé!b"]) return false;
            [previous insertText:@"漏"];
            previous = nil;
            [field insertText:@"Z"];
            break;
        case 11:
            if (![field.text isEqualToString:@"aé!Zb"]) return false;
            [field setMarkedText:@"か" selectedRange:NSMakeRange(1, 0)];
            break;
        case 13:
            return [field.text isEqualToString:@"aé!Zb"];
        case 14:
            dispatch_async(dispatch_get_main_queue(), ^{
                [NSNotificationCenter.defaultCenter postNotificationName:UIApplicationWillTerminateNotification
                                                                   object:UIApplication.sharedApplication];
            });
            break;
    }
    return true;
}
