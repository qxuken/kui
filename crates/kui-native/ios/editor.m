#import <UIKit/UIKit.h>
#import <objc/runtime.h>

extern void kui_ios_text(uint64_t scene, int64_t session, const void *bytes, size_t length,
                         int start, int end, int markStart, int markEnd);

extern void kui_ios_key(uint64_t scene, int64_t owner, bool enter);

// UIKit supplies UITextInput, including UTF-16 selection and marked text.
// KUI draws the editor and receives the original view's touch events.
@interface KUIEditor : UITextView <UITextViewDelegate>
@property(nonatomic) int64_t owner;
@property(nonatomic) uint64_t scene;
@property(nonatomic) BOOL scheduled;
@property(nonatomic) BOOL multiline;
@end

@implementation KUIEditor
- (BOOL)pointInside:(CGPoint)point withEvent:(UIEvent *)event { return NO; }
- (void)queueUpdate {
    if (self.scheduled || !self.owner) return;
    self.scheduled = YES;
    dispatch_async(dispatch_get_main_queue(), ^{
        self.scheduled = NO;
        if (!self.owner) return;
        UITextRange *mark = self.markedTextRange;
        int lo = mark ? (int)[self offsetFromPosition:self.beginningOfDocument toPosition:mark.start] : -1;
        int hi = mark ? (int)[self offsetFromPosition:self.beginningOfDocument toPosition:mark.end] : -1;
        NSRange selected = self.selectedRange;
        if (selected.location == NSNotFound) return;
        NSData *data = [self.text dataUsingEncoding:NSUTF8StringEncoding];
        kui_ios_text(self.scene, self.owner, data.bytes, data.length, (int)selected.location,
                     (int)NSMaxRange(selected), lo, hi);
    });
}
- (BOOL)textView:(UITextView *)view shouldChangeTextInRange:(NSRange)range replacementText:(NSString *)text {
    if (!self.multiline && [text isEqualToString:@"\n"]) {
        kui_ios_key(self.scene, self.owner, true);
        return NO;
    }
    return YES;
}
- (void)deleteBackward {
    if (self.hasText) [super deleteBackward];
    else kui_ios_key(self.scene, self.owner, false);
}
- (void)textViewDidChange:(UITextView *)view { [self queueUpdate]; }
- (void)textViewDidChangeSelection:(UITextView *)view { [self queueUpdate]; }
- (void)setMarkedText:(NSString *)text selectedRange:(NSRange)range {
    [super setMarkedText:text selectedRange:range];
    [self queueUpdate];
}
- (void)unmarkText { [super unmarkText]; [self queueUpdate]; }
@end

void kui_ios_editor(UIView *parent, uint64_t scene, int64_t owner, int kind, const void *bytes, size_t length,
                    int anchor, int caret, bool show, bool refresh) {
    static char association;
    KUIEditor *view = objc_getAssociatedObject(parent, &association);
    if (view && (view.owner != owner || !kind)) {
        view.owner = 0;
        view.delegate = nil;
        if (!kind) [view resignFirstResponder];
        [view removeFromSuperview];
        objc_setAssociatedObject(parent, &association, nil, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
        view = nil;
    }
    if (!kind) {
        // Removing the responder can transfer first-responder status to
        // winit's UIKeyInput parent. End editing for the whole window.
        [parent.window endEditing:YES];
        if (parent.isFirstResponder) [parent resignFirstResponder];
        return;
    }
    if (!view) {
        view = [[KUIEditor alloc] initWithFrame:CGRectMake(0, 0, 1, 1)];
        view.owner = owner;
        view.scene = scene;
        view.alpha = 0.01;
        view.accessibilityElementsHidden = YES;
        view.multiline = (kind & 0x20000) != 0;
        view.returnKeyType = view.multiline ? UIReturnKeyDefault : UIReturnKeyDone;
        view.secureTextEntry = (kind & 0x80) != 0;
        view.delegate = view;
        [parent addSubview:view];
        objc_setAssociatedObject(parent, &association, view, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    }
    if (refresh) {
        NSString *value = [[NSString alloc] initWithBytes:bytes length:length encoding:NSUTF8StringEncoding];
        if (![view.text isEqualToString:value]) view.text = value;
        view.selectedRange = NSMakeRange(MIN(anchor,caret), abs(caret-anchor));
    }
    if (show && parent.window.isKeyWindow && !view.isFirstResponder) [view becomeFirstResponder];
}

extern void kui_ios_hardware_key(uint64_t scene, int64_t owner, uint32_t code,
                                 const char *text, uint32_t modifiers, bool pressed);

// A modal editor takes hardware presses without advertising UITextInput.
@interface KUIKeyView : UIView
@property(nonatomic) uint64_t scene;
@property(nonatomic) int64_t owner;
@end

@implementation KUIKeyView
- (BOOL)canBecomeFirstResponder { return YES; }
- (BOOL)pointInside:(CGPoint)point withEvent:(UIEvent *)event { return NO; }

- (NSSet<UIPress *> *)sendKeys:(NSSet<UIPress *> *)presses down:(BOOL)down {
    NSMutableSet<UIPress *> *unhandled = [NSMutableSet set];
    for (UIPress *press in presses) {
        UIKey *key = press.key;
        if (!key) {
            [unhandled addObject:press];
            continue;
        }
        UIKeyModifierFlags flags = key.modifierFlags;
        uint32_t mods = ((flags & UIKeyModifierShift) ? 1 : 0)
                      | ((flags & UIKeyModifierControl) ? 2 : 0)
                      | ((flags & UIKeyModifierAlternate) ? 4 : 0)
                      | ((flags & UIKeyModifierCommand) ? 8 : 0);
        kui_ios_hardware_key(self.scene, self.owner, (uint32_t)key.keyCode,
                             key.characters.UTF8String, mods, down);
    }
    return unhandled;
}

- (void)pressesBegan:(NSSet<UIPress *> *)presses withEvent:(UIPressesEvent *)event {
    NSSet<UIPress *> *unhandled = [self sendKeys:presses down:YES];
    if (unhandled.count) [super pressesBegan:unhandled withEvent:event];
}
- (void)pressesEnded:(NSSet<UIPress *> *)presses withEvent:(UIPressesEvent *)event {
    NSSet<UIPress *> *unhandled = [self sendKeys:presses down:NO];
    if (unhandled.count) [super pressesEnded:unhandled withEvent:event];
}
- (void)pressesCancelled:(NSSet<UIPress *> *)presses withEvent:(UIPressesEvent *)event {
    NSSet<UIPress *> *unhandled = [self sendKeys:presses down:NO];
    if (unhandled.count) [super pressesCancelled:unhandled withEvent:event];
}
@end

void kui_ios_key_mode(UIView *parent, uint64_t scene, int64_t owner, bool enabled) {
    static char association;
    KUIKeyView *view = objc_getAssociatedObject(parent, &association);
    if (!enabled) {
        [view resignFirstResponder];
        [view removeFromSuperview];
        objc_setAssociatedObject(parent, &association, nil, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
        return;
    }
    if (!view) {
        view = [[KUIKeyView alloc] initWithFrame:CGRectMake(0, 0, 1, 1)];
        view.accessibilityElementsHidden = YES;
        [parent addSubview:view];
        objc_setAssociatedObject(parent, &association, view, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    }
    view.scene = scene;
    view.owner = owner;
    if (parent.window.isKeyWindow) [view becomeFirstResponder];
}

// UISceneSession.persistentIdentifier outlives the scene's windows and views.
extern void kui_scene_foreground(const char *session, bool foreground);
extern void kui_scene_changed(const char *session, const char *document);
// UIKit can launch just one member of a formerly visible window group.
// Only bring back sessions UIKit still owns, and only on a cold launch.
static NSString *const KUIVisibleSessions = @"KUIVisibleSceneSessions";

static void rememberVisibleGroup(UIWindowScene *source) {
    NSMutableArray *ids = [NSMutableArray array];
    for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
        if (![scene isKindOfClass:UIWindowScene.class] ||
            ((UIWindowScene *)scene).screen != source.screen) continue;
        if (scene.activationState == UISceneActivationStateForegroundActive ||
            scene.activationState == UISceneActivationStateForegroundInactive)
            [ids addObject:scene.session.persistentIdentifier];
    }
    if (ids.count) [NSUserDefaults.standardUserDefaults setObject:ids forKey:KUIVisibleSessions];
}

@interface KUISceneDelegate : UIResponder <UIWindowSceneDelegate>
@property(nonatomic, strong) NSUserActivity *document;
@end
@implementation KUISceneDelegate
- (void)scene:(UIScene *)scene willConnectToSession:(UISceneSession *)session options:(UISceneConnectionOptions *)options {
    self.document = options.userActivities.anyObject ?: session.stateRestorationActivity;
    NSString *note = self.document.userInfo[@"document"] ?: @"";
    kui_scene_changed(session.persistentIdentifier.UTF8String, note.UTF8String);
}
- (NSUserActivity *)stateRestorationActivityForScene:(UIScene *)scene { return self.document; }
- (void)sceneWillEnterForeground:(UIScene *)scene { kui_scene_foreground(scene.session.persistentIdentifier.UTF8String, true); }
- (void)sceneDidBecomeActive:(UIScene *)scene {
    if (![scene isKindOfClass:UIWindowScene.class]) return;
    static dispatch_once_t coldLaunch;
    dispatch_once(&coldLaunch, ^{
        NSArray *previous = [NSUserDefaults.standardUserDefaults stringArrayForKey:KUIVisibleSessions];
        if (![previous containsObject:scene.session.persistentIdentifier]) return;
        UISceneActivationRequestOptions *options = [UISceneActivationRequestOptions new];
        options.requestingScene = scene;
        for (UISceneSession *session in UIApplication.sharedApplication.openSessions) {
            UIScene *connected = session.scene;
            if ([previous containsObject:session.persistentIdentifier] && session != scene.session &&
                (!connected || connected.activationState == UISceneActivationStateBackground ||
                 connected.activationState == UISceneActivationStateUnattached))
                [UIApplication.sharedApplication requestSceneSessionActivation:session userActivity:nil options:options
                    errorHandler:^(NSError *error) { NSLog(@"KUI restoring window: %@", error.localizedDescription); }];
        }
    });
    rememberVisibleGroup((UIWindowScene *)scene);
}
- (void)sceneDidEnterBackground:(UIScene *)scene {
    kui_scene_foreground(scene.session.persistentIdentifier.UTF8String, false);
    // Sample after the other scenes have received the same app-wide transition.
    dispatch_async(dispatch_get_main_queue(), ^{
        if ([scene isKindOfClass:UIWindowScene.class]) rememberVisibleGroup((UIWindowScene *)scene);
    });
}
- (void)sceneDidDisconnect:(UIScene *)scene { kui_scene_changed(scene.session.persistentIdentifier.UTF8String, NULL); }
@end

void kui_open_scene(const char *source, const char *document) {
    NSUserActivity *activity = [[NSUserActivity alloc] initWithActivityType:@"dev.kui.open-document"];
    activity.userInfo = @{@"document": [NSString stringWithUTF8String:document]};
    UISceneActivationRequestOptions *options = [UISceneActivationRequestOptions new];
    NSString *session = [NSString stringWithUTF8String:source];
    for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
        if ([scene.session.persistentIdentifier isEqualToString:session]) options.requestingScene = scene;
    }
    [UIApplication.sharedApplication requestSceneSessionActivation:nil userActivity:activity options:options
        errorHandler:^(NSError *error) { NSLog(@"KUI scene activation: %@", error.localizedDescription); }];
}
