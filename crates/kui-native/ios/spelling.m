#import <UIKit/UIKit.h>

// UITextChecker is main-actor isolated. Rust's dictionary worker waits only on
// UIKit; the event loop never waits for a check or for the worker to finish.
static void onMain(dispatch_block_t block) {
    if (NSThread.isMainThread) block();
    else dispatch_sync(dispatch_get_main_queue(), block);
}

@interface KUISpelling : NSObject
@property(nonatomic, strong) UITextChecker *checker;
@property(nonatomic, copy) NSArray<NSString *> *languages;
@end
@implementation KUISpelling
@end

static NSString *string(const uint8_t *bytes, size_t len) {
    return [[NSString alloc] initWithBytes:bytes length:len encoding:NSUTF8StringEncoding];
}

// Snapshot the active writing languages on the UIKit thread.
NSArray<NSString *> *kui_ios_spell_languages(void) {
    NSArray<NSString *> *available = UITextChecker.availableLanguages;
    NSMutableOrderedSet<NSString *> *languages = [NSMutableOrderedSet orderedSet];
    NSMutableOrderedSet<NSString *> *preferences =
        [NSMutableOrderedSet orderedSetWithArray:NSLocale.preferredLanguages];
    for (UITextInputMode *keyboard in UITextInputMode.activeInputModes) {
        if (keyboard.primaryLanguage.length) [preferences addObject:keyboard.primaryLanguage];
    }
    for (NSString *preferred in preferences) {
        NSString *code = [NSLocale localeWithLocaleIdentifier:preferred].languageCode;
        NSMutableArray<NSString *> *related = [NSMutableArray array];
        for (NSString *candidate in available) {
            if ([[NSLocale localeWithLocaleIdentifier:candidate].languageCode isEqual:code])
                [related addObject:candidate];
        }
        if (!related.count) continue;
        NSArray *matches = [NSBundle preferredLocalizationsFromArray:related
                                                      forPreferences:@[preferred]];
        [languages addObject:matches.firstObject ?: related.firstObject];
    }
    return languages.array;
}

// UITextChecker can temporarily accept everything while a keyboard lexicon
// loads, even when it is already listed in availableLanguages. A clean result
// at that point must not seed the window's paragraph cache. Probe the actual
// spell-check operation in each language's own script on this worker.
static NSString *readinessSample(NSString *language) {
    NSLocale *locale = [NSLocale localeWithLocaleIdentifier:language];
    NSString *name = [[locale localizedStringForLanguageCode:locale.languageCode]
                            lowercaseStringWithLocale:locale];
    NSMutableString *sample = [NSMutableString string];
    [name enumerateSubstringsInRange:NSMakeRange(0, name.length)
                            options:NSStringEnumerationByComposedCharacterSequences
                         usingBlock:^(NSString *part, NSRange range __unused,
                                      NSRange enclosing __unused, BOOL *stop __unused) {
        if ([part rangeOfCharacterFromSet:NSCharacterSet.letterCharacterSet].location != NSNotFound) {
            for (int repeat = 0; repeat < 3; repeat++) [sample appendString:part];
        }
    }];
    return sample;
}

static void awaitLexicons(KUISpelling *dictionary) {
    NSMutableDictionary<NSString *, NSString *> *pending = [NSMutableDictionary dictionary];
    for (NSString *language in dictionary.languages) {
        NSString *sample = readinessSample(language);
        if (sample.length) pending[language] = sample;
    }
    // Some writing systems need no spelling dictionaries, and a device may
    // have no usable asset. Neither condition can hold up the worker forever.
    NSTimeInterval deadline = NSProcessInfo.processInfo.systemUptime + 5.0;
    NSTimeInterval delay = 0.05;
    while (pending.count) {
        for (NSString *language in pending.allKeys) {
            NSString *sample = pending[language];
            __block NSRange miss;
            onMain(^{
                miss = [dictionary.checker rangeOfMisspelledWordInString:sample
                    range:NSMakeRange(0, sample.length) startingAt:0 wrap:NO language:language];
            });
            if (miss.location != NSNotFound) [pending removeObjectForKey:language];
        }
        NSTimeInterval remaining = deadline - NSProcessInfo.processInfo.systemUptime;
        if (!pending.count || remaining <= 0) break;
        [NSThread sleepForTimeInterval:MIN(delay, remaining)];
        delay = MIN(delay * 2, 0.5);
    }
}

void *kui_ios_spell_open(void) {
    __block void *result = NULL;
    onMain(^{
        NSArray *languages = kui_ios_spell_languages();
        if (!languages.count) return;
        KUISpelling *dictionary = [KUISpelling new];
        dictionary.checker = [UITextChecker new];
        dictionary.languages = languages;
        result = (__bridge_retained void *)dictionary;
    });
    if (result) {
        @autoreleasepool { awaitLexicons((__bridge KUISpelling *)result); }
    }
    return result;
}

void kui_ios_spell_close(void *handle) {
    onMain(^{ (void)CFBridgingRelease(handle); });
}

void kui_ios_spell_check(void *handle, const uint8_t *bytes, size_t len, void *context,
                         void (*emit)(void *, size_t, size_t)) {
    @autoreleasepool {
        NSString *text = string(bytes, len);
        if (!text.length) return;
        KUISpelling *dictionary = (__bridge KUISpelling *)handle;
        NSRange full = NSMakeRange(0, text.length);
        NSMutableIndexSet *errors = [NSMutableIndexSet indexSetWithIndexesInRange:full];
        // Remove the ranges accepted by each dictionary. Different token
        // boundaries in two languages can still agree on part of a word.
        for (NSString *language in dictionary.languages) {
            NSUInteger cursor = 0;
            while (cursor < full.length && errors.count) {
                __block NSRange miss;
                onMain(^{
                    miss = [dictionary.checker rangeOfMisspelledWordInString:text
                        range:full startingAt:cursor wrap:NO language:language];
                });
                if (miss.location == NSNotFound || miss.location < cursor ||
                    !miss.length || miss.length > full.length || miss.location > full.length - miss.length) {
                    [errors removeIndexesInRange:NSMakeRange(cursor, full.length - cursor)];
                    break;
                }
                [errors removeIndexesInRange:NSMakeRange(cursor, miss.location - cursor)];
                cursor = NSMaxRange(miss);
            }
        }
        [errors enumerateRangesUsingBlock:^(NSRange range, BOOL * __unused stop) {
            emit(context, range.location, NSMaxRange(range));
        }];
    }
}

bool kui_ios_spell_choices(void *handle, const uint8_t *bytes, size_t len, void *context,
                           void (*emit)(void *, const uint8_t *, size_t)) {
    __block BOOL canLearn = NO;
    onMain(^{
        NSString *word = string(bytes, len);
        if (!word.length || [UITextChecker hasLearnedWord:word]) return;
        canLearn = YES;
        KUISpelling *dictionary = (__bridge KUISpelling *)handle;
        NSMutableSet *seen = [NSMutableSet setWithObject:word];
        for (NSString *language in dictionary.languages) {
            for (NSString *guess in [dictionary.checker guessesForWordRange:NSMakeRange(0, word.length)
                             inString:word language:language]) {
                if (!guess.length || [seen containsObject:guess]) continue;
                [seen addObject:guess];
                emit(context, (const uint8_t *)guess.UTF8String,
                     [guess lengthOfBytesUsingEncoding:NSUTF8StringEncoding]);
                if (seen.count >= 9) return;
            }
        }
    });
    return canLearn;
}

bool kui_ios_spell_learn(const uint8_t *bytes, size_t len) {
    __block BOOL learned = NO;
    onMain(^{
        NSString *word = string(bytes, len);
        if (!word.length) return;
        [UITextChecker learnWord:word];
        learned = [UITextChecker hasLearnedWord:word];
    });
    return learned;
}
