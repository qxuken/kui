#import <UIKit/UIKit.h>

// UITextChecker is main-actor isolated. Rust's dictionary worker waits only on
// UIKit; the event loop never waits for a check or for the worker to finish.
static void onMain(dispatch_block_t block) {
    if (NSThread.isMainThread) block();
    else dispatch_sync(dispatch_get_main_queue(), block);
}

extern void kui_ios_spell_readiness_changed(void);
// The simulator regression supplies this optional sink. Only request/result
// counts cross the probe; release apps have no sink or diagnostic storage.
extern void kui_probe_backend_counts(int stage, size_t a, size_t b, size_t c) __attribute__((weak_import));

@interface KUISpelling : NSObject
@property(nonatomic, strong) UITextChecker *checker;
@property(nonatomic, copy) NSArray<NSString *> *languages;
@property(nonatomic, copy) NSDictionary<NSString *, NSCharacterSet *> *letters;
@property(nonatomic, strong) NSMutableArray<NSString *> *samples;
@property(nonatomic) BOOL scheduled;
@property(nonatomic) BOOL languagesChanged;
@property(nonatomic) NSTimeInterval lastProbe;
- (void)observe:(NSSet<NSString *> *)answered text:(NSString *)text;
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

// UIKit can list a cold keyboard lexicon and temporarily accept all text.
// Keep that uncertainty local to this process, across worker cache reopens.
// Both sets are accessed only on UIKit's main thread.
static NSMutableSet<NSString *> *confirmed;
static NSMutableSet<NSString *> *provisional;

static NSIndexSet *nativeErrors(UITextChecker *checker, NSString *text, NSString *language) {
    NSMutableIndexSet *errors = [NSMutableIndexSet indexSet];
    NSRange full = NSMakeRange(0, text.length);
    NSUInteger cursor = 0;
    while (cursor < full.length) {
        __block NSRange miss;
        onMain(^{
            miss = [checker rangeOfMisspelledWordInString:text
                range:full startingAt:cursor wrap:NO language:language];
        });
        if (miss.location == NSNotFound || miss.location < cursor || !miss.length ||
            miss.length > full.length || miss.location > full.length - miss.length) break;
        [errors addIndexesInRange:miss];
        cursor = NSMaxRange(miss);
    }
    return errors;
}

@implementation KUISpelling
- (void)observe:(NSSet<NSString *> *)answered text:(NSString *)text {
    BOOL changed = NO;
    for (NSString *language in answered) {
        if ([confirmed containsObject:language]) continue;
        changed |= [provisional containsObject:language];
        [confirmed addObject:language];
    }
    BOOL pending = self.languages.count == 0;
    for (NSString *language in self.languages) {
        if (![confirmed containsObject:language]) {
            [provisional addObject:language];
            pending = YES;
        }
    }
    if (!pending) [self.samples removeAllObjects];
    if (text.length && pending) {
        // Keep at most four short excerpts on the window, never in the
        // process-wide readiness state. A full paragraph is always checked
        // by the normal worker.
        NSRange prefix = [text rangeOfComposedCharacterSequencesForRange:
                              NSMakeRange(0, MIN(text.length, 256))];
        NSString *sample = [text substringWithRange:prefix];
        if (![self.samples containsObject:sample]) {
            if (self.samples.count == 4) [self.samples removeObjectAtIndex:0];
            [self.samples addObject:sample];
        }
        if (!self.scheduled) {
            self.scheduled = YES;
            __weak KUISpelling *weak = self;
            for (NSNumber *seconds in @[@1, @4, @16]) {
                dispatch_after(dispatch_time(DISPATCH_TIME_NOW, seconds.longLongValue * NSEC_PER_SEC),
                               dispatch_get_main_queue(), ^{ [weak refresh]; });
            }
        }
    }
    if (changed) kui_ios_spell_readiness_changed();
}
- (void)refresh {
    self.lastProbe = NSProcessInfo.processInfo.systemUptime;
    if (self.languagesChanged) return;
    if (![self.languages isEqualToArray:kui_ios_spell_languages()]) {
        self.languagesChanged = YES;
        kui_ios_spell_readiness_changed();
        return;
    }
    if (!self.samples.count) return;
    // A checker created while keyboard assets are loading can keep returning
    // an empty result. Compare the actual saved sample on a fresh instance.
    UITextChecker *fresh = [UITextChecker new];
    NSMutableSet *answered = [NSMutableSet set];
    for (NSString *language in self.languages) {
        if ([confirmed containsObject:language]) continue;
        for (NSString *text in self.samples) {
            if (nativeErrors(fresh, text, language).count) {
                [answered addObject:language];
                break;
            }
        }
    }
    if (kui_probe_backend_counts) kui_probe_backend_counts(2, self.languages.count, self.samples.count, answered.count);
    if (answered.count) self.checker = fresh;
    [self observe:answered text:nil];
}
- (void)activated:(NSNotification *)note { [self refresh]; }
@end

void *kui_ios_spell_open(void) {
    __block void *result = NULL;
    onMain(^{
        NSArray *languages = kui_ios_spell_languages();
        // Keyboard languages can be empty on the first UIKit query. Keep
        // the worker alive so an activation or provisional check can retry.
        if (kui_probe_backend_counts) kui_probe_backend_counts(0, languages.count, 0, 0);
        KUISpelling *dictionary = [KUISpelling new];
        dictionary.checker = [UITextChecker new];
        dictionary.languages = languages;
        if (!confirmed) {
            confirmed = [NSMutableSet set];
            provisional = [NSMutableSet set];
        }
        dictionary.samples = [NSMutableArray array];
        dictionary.lastProbe = NSProcessInfo.processInfo.systemUptime;
        NSMutableDictionary *letters = [NSMutableDictionary dictionary];
        for (NSString *language in languages) {
            NSLocale *locale = [NSLocale localeWithLocaleIdentifier:language];
            NSMutableCharacterSet *alphabet = [locale.exemplarCharacterSet mutableCopy];
            [alphabet formIntersectionWithCharacterSet:NSCharacterSet.letterCharacterSet];
            letters[language] = alphabet ?: NSCharacterSet.letterCharacterSet;
        }
        dictionary.letters = letters;
        [NSNotificationCenter.defaultCenter addObserver:dictionary selector:@selector(activated:)
            name:UIApplicationDidBecomeActiveNotification object:nil];
        [NSNotificationCenter.defaultCenter addObserver:dictionary selector:@selector(activated:)
            name:UITextInputCurrentInputModeDidChangeNotification object:nil];
        result = (__bridge_retained void *)dictionary;
    });
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
        NSMutableDictionary *checked = [NSMutableDictionary dictionary];
        NSMutableSet *answered = [NSMutableSet set];
        NSString *lowercase = text.lowercaseString;
        __block UITextChecker *checker;
        onMain(^{
            checker = dictionary.checker;
            if (![[NSSet setWithArray:dictionary.languages] isSubsetOfSet:confirmed]) {
                checker = [UITextChecker new];
                dictionary.checker = checker;
            }
        });
        NSUInteger rawUnits = 0;
        for (NSString *language in dictionary.languages) {
            if ([lowercase rangeOfCharacterFromSet:dictionary.letters[language]].location == NSNotFound)
                continue;
            NSIndexSet *errors = nativeErrors(checker, text, language);
            rawUnits += errors.count;
            checked[language] = errors;
            if (errors.count) [answered addObject:language];
        }
        onMain(^{
            [dictionary observe:answered text:text];
            if (NSProcessInfo.processInfo.systemUptime - dictionary.lastProbe >= 30) [dictionary refresh];
        });
        NSIndexSet *errors = kui_ios_spell_consensus(text, dictionary.letters, checked);
        if (kui_probe_backend_counts) kui_probe_backend_counts(1, checked.count, rawUnits, errors.count);
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
            if ([word.lowercaseString rangeOfCharacterFromSet:dictionary.letters[language]].location == NSNotFound)
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
