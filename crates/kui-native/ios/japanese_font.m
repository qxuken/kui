#import <Foundation/Foundation.h>
#import <CoreText/CoreText.h>

bool kui_japanese_face(char *path, size_t path_size, char *name, size_t name_size) {
    CTFontRef base = CTFontCreateUIFontForLanguage(kCTFontUIFontSystem, 16, CFSTR("ja"));
    if (!base) return false;
    CTFontRef face = CTFontCreateForStringWithLanguage(base, CFSTR("日本"), CFRangeMake(0, 2), CFSTR("ja"));
    CFRelease(base);
    if (!face) return false;
    NSURL *url = CFBridgingRelease(CTFontCopyAttribute(face, kCTFontURLAttribute));
    NSString *postscript = CFBridgingRelease(CTFontCopyPostScriptName(face));
    CFRelease(face);
    return [url getFileSystemRepresentation:path maxLength:path_size] &&
           [postscript getCString:name maxLength:name_size encoding:NSUTF8StringEncoding];
}
