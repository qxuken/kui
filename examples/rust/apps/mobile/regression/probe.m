#import <UIKit/UIKit.h>

static UIView *responder(UIView *view) {
    if (view.isFirstResponder) return view;
    for (UIView *child in view.subviews) {
        UIView *found = responder(child);
        if (found) return found;
    }
    return nil;
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
