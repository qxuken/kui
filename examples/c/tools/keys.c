/* kui_glfw.h and kui_sdl3.h, checked (backlog F239): each table names the
 * keys kui's way, with their places, and the SDL forwarder - which calls
 * no SDL function, so it runs here on events made by hand - sends a
 * letter, a keypad digit, a named key and typed text the way the headers
 * say. GLFW's forwarder asks glfwGetKeyName, which needs a GLFW to link,
 * so only its table, its modifiers and its char callback are run. A
 * header whose library is not installed is skipped and said to be. */
#include "../common.h"

#if defined(__has_include)
#if __has_include(<GLFW/glfw3.h>)
#include <GLFW/glfw3.h>
#include "kui_glfw.h"
#define HAVE_GLFW 1
#endif
#if __has_include(<SDL3/SDL.h>)
#include <SDL3/SDL.h>
#include "kui_sdl3.h"
#define HAVE_SDL3 1
#endif
#endif

static bool str_is(const char *a, const char *b) {
    return a && b && strcmp(a, b) == 0;
}

/* One frame of a focused editor and a key sink; returns the sink's key in
 * `sink`, the editor's in `edit`. */
static void frame(KuiCtx *ui, uint64_t *edit, uint64_t *sink) {
    kui_frame_begin(ui, 200, 100, 1);
    KuiSpec field = {.width = {KUI_FIXED, 150}, .height = {KUI_FIXED, 24}, .label = KUI_STR("name")};
    *edit = kui_text_edit(ui, KUI_STR("name"), KUI_STR(""), NULL, 0, &field);
    KuiSpec keys = {.width = {KUI_FIXED, 150}, .height = {KUI_FIXED, 24}, .focusable = 1,
                    .role = KUI_ROLE_GROUP, .label = KUI_STR("keys"), .key_up = 1};
    *sink = kui_open_with(ui, KUI_STR("keys"), &keys, NULL, NULL, kui_value_str(KUI_STR("keys")), NULL);
    kui_close(ui);
    kui_frame_finish(ui);
}

/* The next key event's code, physical and location, into the buffers. */
static bool next_key(KuiCtx *ui, char *code, char *physical, char *location) {
    KuiEvent ev = KUI_EVENT_INIT;
    while (kui_poll_event(ui, &ev)) {
        if (!ev_is(&ev, "key")) continue;
        const KuiValue *data = ev_data(&ev);
        KuiStr c = {0}, p = {0}, l = {0};
        kui_value_as_str(kui_value_get(data, KUI_STR("code")), &c);
        kui_value_as_str(kui_value_get(data, KUI_STR("physical")), &p);
        kui_value_as_str(kui_value_get(data, KUI_STR("location")), &l);
        snprintf(code, 16, "%.*s", (int)c.len, (const char *)c.ptr);
        snprintf(physical, 16, "%.*s", (int)p.len, (const char *)p.ptr);
        snprintf(location, 16, "%.*s", (int)l.len, (const char *)l.ptr);
        return true;
    }
    return false;
}

int main(void) {
    if (!abi_ok()) return 1;
    int ran = 0;
#ifdef HAVE_GLFW
    {
        ran++;
        uint32_t loc = 99;
        check(str_is(kui_glfw_physical(GLFW_KEY_A, &loc), "a") && loc == 0, "glfw: A is a");
        check(str_is(kui_glfw_physical(GLFW_KEY_KP_1, &loc), "1") && loc == KUI_KLOC_NUMPAD,
              "glfw: the keypad's 1 is 1 on the keypad");
        check(str_is(kui_glfw_physical(GLFW_KEY_RIGHT_SHIFT, &loc), "shift") && loc == KUI_KLOC_RIGHT,
              "glfw: the right shift");
        check(str_is(kui_glfw_physical(GLFW_KEY_LEFT_BRACKET, NULL), "["), "glfw: [");
        check(kui_glfw_physical(GLFW_KEY_WORLD_1, &loc) == NULL, "glfw: a key kui names none for");
        check(kui_glfw_mods(GLFW_MOD_SHIFT | GLFW_MOD_SUPER | GLFW_MOD_CAPS_LOCK)
                  == (KUI_KMOD_SHIFT | KUI_KMOD_SUPER | KUI_KLOCK_CAPS),
              "glfw: the modifiers and a lock");
        KuiCtx *ui = kui_ctx_new();
        uint64_t edit, sink;
        frame(ui, &edit, &sink);
        kui_focus(ui, edit);
        frame(ui, &edit, &sink);
        kui_glfw_char(ui, 0x416); /* Ж */
        kui_glfw_char(ui, 'x');
        KuiStr text = {0};
        check(kui_edit_text(ui, edit, &text) && text.len == 3 && memcmp(text.ptr, "\xd0\x96x", 3) == 0,
              "glfw: the char callback types into the editor");
        kui_ctx_free(ui);
    }
#else
    printf("keys: no <GLFW/glfw3.h>; kui_glfw.h not checked\n");
#endif
#ifdef HAVE_SDL3
    {
        ran++;
        uint32_t loc = 99;
        check(str_is(kui_sdl3_physical(SDL_SCANCODE_RETURN, &loc), "enter") && loc == 0, "sdl3: Return is enter");
        check(str_is(kui_sdl3_physical(SDL_SCANCODE_KP_ENTER, &loc), "enter") && loc == KUI_KLOC_NUMPAD,
              "sdl3: the keypad's Enter");
        check(str_is(kui_sdl3_physical(SDL_SCANCODE_RGUI, &loc), "super") && loc == KUI_KLOC_RIGHT, "sdl3: right GUI");
        check(str_is(kui_sdl3_physical(SDL_SCANCODE_MEDIA_PLAY_PAUSE, NULL), "mediaplaypause"), "sdl3: a media key");
        check(kui_sdl3_physical(SDL_SCANCODE_NONUSBACKSLASH, &loc) == NULL, "sdl3: a key kui names none for");
        check(kui_sdl3_mods(SDL_KMOD_RCTRL | SDL_KMOD_NUM) == (KUI_KMOD_CTRL | KUI_KLOCK_NUM),
              "sdl3: the modifiers and a lock");

        KuiCtx *ui = kui_ctx_new();
        uint64_t edit, sink;
        frame(ui, &edit, &sink);
        kui_focus(ui, sink);
        frame(ui, &edit, &sink);
        char code[16], physical[16], location[16];
        /* Shift+A on a layout that calls the key "a": code "A". */
        SDL_KeyboardEvent a = {.scancode = SDL_SCANCODE_A, .key = SDLK_A, .mod = SDL_KMOD_LSHIFT, .down = true};
        kui_sdl3_key(ui, &a);
        check(next_key(ui, code, physical, location) && str_is(code, "A") && str_is(physical, "a"),
              "sdl3: shift+a is code A at a");
        /* The keypad's 1: its keycode is no character, so the place names it. */
        SDL_KeyboardEvent kp = {.scancode = SDL_SCANCODE_KP_1, .key = SDLK_KP_1, .down = true};
        kui_sdl3_key(ui, &kp);
        check(next_key(ui, code, physical, location) && str_is(code, "1") && str_is(location, "numpad"),
              "sdl3: the keypad's 1 on the keypad");
        /* AZERTY's A key, where US has Q: code a, physical q. */
        SDL_KeyboardEvent az = {.scancode = SDL_SCANCODE_Q, .key = SDLK_A, .down = true};
        kui_sdl3_key(ui, &az);
        check(next_key(ui, code, physical, location) && str_is(code, "a") && str_is(physical, "q"),
              "sdl3: the layout's letter at the US place");
        /* A named key goes through the press: the sink hears it too. */
        SDL_KeyboardEvent f5 = {.scancode = SDL_SCANCODE_F5, .key = SDLK_F5, .down = true};
        kui_sdl3_key(ui, &f5);
        check(next_key(ui, code, physical, location) && str_is(code, "f5"), "sdl3: F5");
        SDL_KeyboardEvent up = f5;
        up.down = false;
        kui_sdl3_key(ui, &up);
        check(next_key(ui, code, physical, location) && str_is(code, "f5"), "sdl3: and its release");

        /* Typing reaches an editor once, from the text event: the key's
         * own press types nothing. */
        kui_focus(ui, edit);
        frame(ui, &edit, &sink);
        SDL_KeyboardEvent h = {.scancode = SDL_SCANCODE_H, .key = SDLK_H, .down = true};
        kui_sdl3_key(ui, &h);
        SDL_TextInputEvent t = {.text = "h"};
        kui_sdl3_text(ui, &t);
        SDL_KeyboardEvent sp = {.scancode = SDL_SCANCODE_SPACE, .key = SDLK_SPACE, .down = true};
        kui_sdl3_key(ui, &sp);
        SDL_TextInputEvent ts = {.text = " "};
        kui_sdl3_text(ui, &ts);
        KuiStr text = {0};
        check(kui_edit_text(ui, edit, &text) && text.len == 2 && memcmp(text.ptr, "h ", 2) == 0,
              "sdl3: h and a space typed once each");
        kui_ctx_free(ui);
    }
#else
    printf("keys: no <SDL3/SDL.h>; kui_sdl3.h not checked\n");
#endif
    if (fails) {
        fprintf(stderr, "keys: %d check(s) failed\n", fails);
        return 1;
    }
    printf("keys: %d of 2 tables checked OK\n", ran);
    return 0;
}
