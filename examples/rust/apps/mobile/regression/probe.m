#import <UIKit/UIKit.h>

static UIView *responder(UIView *view) {
    if (view.isFirstResponder) return view;
    for (UIView *child in view.subviews) {
        UIView *found = responder(child);
        if (found) return found;
    }
    return nil;
}

extern NSArray<NSString *> *kui_ios_spell_languages(void);

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
    NSString *summary = [NSString stringWithFormat:@"UIKit preferred=%@ keyboards=%@ configured=%@ nativeUTF16Ranges=%@",
                         NSLocale.preferredLanguages, keyboards, kui_ios_spell_languages(), ranges];
    report(ctx, summary.UTF8String);
}

void kui_probe_unlearn(const uint8_t *bytes, size_t len) {
    NSString *word = [[NSString alloc] initWithBytes:bytes length:len encoding:NSUTF8StringEncoding];
    [UITextChecker unlearnWord:word];
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
