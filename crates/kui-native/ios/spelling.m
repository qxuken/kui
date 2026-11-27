#import <UIKit/UIKit.h>

// UITextChecker is main-actor isolated. Rust's dictionary worker waits only on
// UIKit; the event loop never waits for a check or for the worker to finish.
static void onMain(dispatch_block_t block) {
    if (NSThread.isMainThread) block();
    else dispatch_sync(dispatch_get_main_queue(), block);
}

extern void kui_ios_spell_readiness_changed(void);

@interface KUISpelling : NSObject
@property(nonatomic, strong) UITextChecker *checker;
@property(nonatomic, copy) NSArray<NSString *> *languages;
@property(nonatomic, copy) NSDictionary<NSString *, NSCharacterSet *> *letters;
@property(nonatomic, strong) NSMutableDictionary<NSString *, NSString *> *pending;
@property(nonatomic) NSTimeInterval lastProbe;
- (BOOL)probePending;
- (void)refresh;
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

@implementation KUISpelling
// All changes to pending are on the main thread. Remove a language exactly
// once when UIKit begins returning real results for its writing system.
- (BOOL)probePending {
    self.lastProbe = NSProcessInfo.processInfo.systemUptime;
    NSUInteger before = self.pending.count;
    for (NSString *language in self.pending.allKeys) {
        NSString *sample = self.pending[language];
        NSRange miss = [self.checker rangeOfMisspelledWordInString:sample
            range:NSMakeRange(0, sample.length) startingAt:0 wrap:NO language:language];
        if (miss.location != NSNotFound) [self.pending removeObjectForKey:language];
    }
    return self.pending.count != before;
}
- (void)refresh {
    if (self.pending.count && [self probePending]) kui_ios_spell_readiness_changed();
}
- (void)activated:(NSNotification *)note {
    if (![self.languages isEqualToArray:kui_ios_spell_languages()]) kui_ios_spell_readiness_changed();
    else [self refresh];
}
@end

static void awaitLexicons(KUISpelling *dictionary) {
    // Start serving as soon as one dictionary responds. A missing secondary
    // dictionary cannot hold up words in another writing system.
    NSTimeInterval deadline = NSProcessInfo.processInfo.systemUptime + 5.0;
    NSTimeInterval delay = 0.05;
    for (;;) {
        __block BOOL ready;
        onMain(^{
            [dictionary probePending];
            ready = dictionary.pending.count < dictionary.languages.count;
        });
        NSTimeInterval remaining = deadline - NSProcessInfo.processInfo.systemUptime;
        if (ready || remaining <= 0) break;
        [NSThread sleepForTimeInterval:MIN(delay, remaining)];
        delay = MIN(delay * 2, 0.5);
    }
    onMain(^{
        if (dictionary.pending.count) {
            // Bounded follow-ups even if the document is idle. Later checks
            // and foreground changes give newly available assets another turn.
            __weak KUISpelling *weak = dictionary;
            for (NSNumber *seconds in @[@1, @4, @16]) {
                dispatch_after(dispatch_time(DISPATCH_TIME_NOW, seconds.longLongValue * NSEC_PER_SEC),
                               dispatch_get_main_queue(), ^{ [weak refresh]; });
            }
        }
        [NSNotificationCenter.defaultCenter addObserver:dictionary selector:@selector(activated:)
            name:UIApplicationDidBecomeActiveNotification object:nil];
        [NSNotificationCenter.defaultCenter addObserver:dictionary selector:@selector(activated:)
            name:UITextInputCurrentInputModeDidChangeNotification object:nil];
    });
}

void *kui_ios_spell_open(void) {
    __block void *result = NULL;
    onMain(^{
        NSArray *languages = kui_ios_spell_languages();
        if (!languages.count) return;
        KUISpelling *dictionary = [KUISpelling new];
        dictionary.checker = [UITextChecker new];
        dictionary.languages = languages;
        dictionary.pending = [NSMutableDictionary dictionary];
        NSMutableDictionary *letters = [NSMutableDictionary dictionary];
        for (NSString *language in languages) {
            NSLocale *locale = [NSLocale localeWithLocaleIdentifier:language];
            NSMutableCharacterSet *alphabet = [locale.exemplarCharacterSet mutableCopy];
            [alphabet formIntersectionWithCharacterSet:NSCharacterSet.letterCharacterSet];
            letters[language] = alphabet ?: NSCharacterSet.letterCharacterSet;
            NSString *sample = readinessSample(language);
            if (sample.length) dictionary.pending[language] = sample;
        }
        dictionary.letters = letters;
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

// Only dictionaries covering this word's writing system have a vote. An
// asset still loading abstains on unrelated scripts and defers its own.
NSIndexSet *kui_ios_spell_consensus(NSString *text, NSDictionary<NSString *, NSCharacterSet *> *letters,
                                    NSDictionary<NSString *, NSIndexSet *> *checked) {
    NSMutableIndexSet *errors = [NSMutableIndexSet indexSet];
    for (NSString *language in checked) {
        [checked[language] enumerateRangesUsingBlock:^(NSRange range, BOOL * __unused stop) {
            NSString *word = [text substringWithRange:range].lowercaseString;
            if ([word rangeOfCharacterFromSet:letters[language]].location != NSNotFound)
                [errors addIndexesInRange:range];
        }];
    }
    if (letters.count == 1 || !errors.count) return errors;
    NSIndexSet *candidates = [errors copy];
    [candidates enumerateRangesUsingBlock:^(NSRange range, BOOL * __unused stop) {
        NSString *word = [text substringWithRange:range].lowercaseString;
        for (NSString *language in letters) {
            if ([word rangeOfCharacterFromSet:letters[language]].location == NSNotFound) continue;
            NSIndexSet *rejected = checked[language];
            if (!rejected) {
                [errors removeIndexesInRange:range];
                break;
            }
            NSMutableIndexSet *accepted = [NSMutableIndexSet indexSetWithIndexesInRange:range];
            [accepted removeIndexes:rejected];
            [errors removeIndexes:accepted];
        }
    }];
    return errors;
}

void kui_ios_spell_check(void *handle, const uint8_t *bytes, size_t len, void *context,
                         void (*emit)(void *, size_t, size_t)) {
    @autoreleasepool {
        NSString *text = string(bytes, len);
        if (!text.length) return;
        KUISpelling *dictionary = (__bridge KUISpelling *)handle;
        __block NSSet *pending;
        onMain(^{
            if (NSProcessInfo.processInfo.systemUptime - dictionary.lastProbe >= 30)
                [dictionary refresh];
            pending = [NSSet setWithArray:dictionary.pending.allKeys];
        });
        NSRange full = NSMakeRange(0, text.length);
        NSMutableDictionary *checked = [NSMutableDictionary dictionary];
        for (NSString *language in dictionary.languages) {
            if ([pending containsObject:language]) continue;
            NSMutableIndexSet *errors = [NSMutableIndexSet indexSet];
            NSUInteger cursor = 0;
            while (cursor < full.length) {
                __block NSRange miss;
                onMain(^{
                    miss = [dictionary.checker rangeOfMisspelledWordInString:text
                        range:full startingAt:cursor wrap:NO language:language];
                });
                if (miss.location == NSNotFound || miss.location < cursor || !miss.length ||
                    miss.length > full.length || miss.location > full.length - miss.length) break;
                [errors addIndexesInRange:miss];
                cursor = NSMaxRange(miss);
            }
            checked[language] = errors;
        }
        NSIndexSet *errors = kui_ios_spell_consensus(text, dictionary.letters, checked);
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
            if (dictionary.pending[language] ||
                [word.lowercaseString rangeOfCharacterFromSet:dictionary.letters[language]].location == NSNotFound)
                continue;
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
