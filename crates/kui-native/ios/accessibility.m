#import <UIKit/UIKit.h>

void kui_ios_announce(const uint8_t *bytes, size_t length, bool assertive) {
    NSString *text = [[NSString alloc] initWithBytes:bytes length:length encoding:NSUTF8StringEncoding];
    NSDictionary<NSAttributedStringKey, id> *attributes;
    if (@available(iOS 17.0, *)) {
        attributes = @{
            UIAccessibilitySpeechAttributeAnnouncementPriority:
                assertive ? UIAccessibilityPriorityHigh : UIAccessibilityPriorityLow,
        };
    } else {
        attributes = @{UIAccessibilitySpeechAttributeQueueAnnouncement: @(!assertive)};
    }
    NSAttributedString *message = [[NSAttributedString alloc] initWithString:text attributes:attributes];
    UIAccessibilityPostNotification(UIAccessibilityAnnouncementNotification, message);
}
