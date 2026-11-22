#include <android/api-level.h>
#include <android/font_matcher.h>
#include <stdint.h>
#include <string.h>

// 1 = matched, 0 = no usable match, -1 = API 28 (use its font configuration).
int kui_match_japanese(char *path, size_t capacity, uint32_t *index) {
    if (android_get_device_api_level() < 29) return -1;

    const uint16_t text[] = {0x65e5, 0x672c}; // 日本
    AFontMatcher *matcher = AFontMatcher_create();
    AFontMatcher_setLocales(matcher, "ja-JP");
    uint32_t length = 0;
    AFont *font = AFontMatcher_match(matcher, "sans-serif", text, 2, &length);
    const char *file = AFont_getFontFilePath(font);
    size_t face = AFont_getCollectionIndex(font);
    int matched = length == 2 && face <= UINT32_MAX && strlcpy(path, file, capacity) < capacity;
    if (matched) *index = (uint32_t)face;
    AFont_close(font);
    AFontMatcher_destroy(matcher);
    return matched;
}
