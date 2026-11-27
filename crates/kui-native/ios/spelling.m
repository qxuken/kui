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

void *kui_ios_spell_open(void) {
    __block void *result = NULL;
    onMain(^{
        NSArray<NSString *> *available = UITextChecker.availableLanguages;
        if (!available.count) return;
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
        if (!languages.count) return;
        KUISpelling *dictionary = [KUISpelling new];
        dictionary.checker = [UITextChecker new];
        dictionary.languages = languages.array;
        result = (__bridge_retained void *)dictionary;
    });
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
        [errors enumerateRangesUsingBlock:^(NSRange range, BOOL *stop) {
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
