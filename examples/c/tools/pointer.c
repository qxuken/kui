/* kui_glfw.h's and kui_sdl3.h's pointer halves, checked (backlog F265):
 * on events made by hand, a click lands on its button, a double and a
 * triple click select a word and a line, a wheel's notch moves the
 * scroller under it - and the next one stays with that scroller until a
 * pause or the pointer moving begins another gesture - and the pointer
 * leaving the window is kui's to know.
 *
 * Nothing here calls GLFW or SDL, so neither is linked: KUI_GLFW_TIME()
 * reads this file's clock, and kui_glfw_install's chaining is run on the
 * callbacks it would set, found by a window that is only an address -
 * glfwSet*Callback handing back the host's callback is GLFW's half, which
 * takes a GLFW and a window. A header whose library is not installed is
 * skipped and said to be. */
#include "../common.h"

/* The clock the GLFW helpers read, in seconds. */
static double now_s = 1.0;
#define KUI_GLFW_TIME() now_s

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

#if defined(HAVE_GLFW) || defined(HAVE_SDL3)
static uint64_t edit, outer, inner;

/* A button "go" at the top (0..24), an editor holding "hello world" under
 * it (24..48), and under that a scroller (48..148) whose first child is a
 * smaller scroller (48..108) that moves 40 px. */
static void frame(KuiCtx *ui) {
    KUI_FRAME(ui, 400, 400, 1) {
        KUI_BOX_CLICK(ui, "go", "go", .width = {KUI_FIXED, 80}, .height = {KUI_FIXED, 24},
                      .label = KUI_STR("go")) {}
        KuiSpec field = {.width = {KUI_FIXED, 200}, .height = {KUI_FIXED, 24}, .label = KUI_STR("name")};
        edit = kui_text_edit(ui, KUI_STR("name"), KUI_STR("hello world"), NULL, 0, &field);
        KUI_BOX_KEY(ui, &outer, "outer", .width = {KUI_FIXED, 200}, .height = {KUI_FIXED, 100},
                    .overflow = KUI_SCROLL_Y) {
            KUI_BOX_KEY(ui, &inner, "inner", .width = {KUI_FIXED, 180}, .height = {KUI_FIXED, 60},
                        .overflow = KUI_SCROLL_Y) {
                KUI_BOX(ui, .width = {KUI_FIXED, 180}, .height = {KUI_FIXED, 100}) {}
            }
            KUI_BOX(ui, .width = {KUI_FIXED, 180}, .height = {KUI_FIXED, 400}) {}
        }
    }
}

/* Drains the queue; how many clicks said "go". */
static int clicks_on_go(KuiCtx *ui) {
    int n = 0;
    KuiEvent ev = KUI_EVENT_INIT;
    while (kui_poll_event(ui, &ev)) {
        KuiStr msg = {0};
        if (ev_is(&ev, "click") && kui_event_message(&ev, &msg) && kui_str_eq(msg, "go")) n++;
    }
    return n;
}

/* The editor's selection, as byte offsets; `to` below `from` asks only
 * that it be empty (a caret). */
static bool selected(KuiCtx *ui, uint32_t from, uint32_t to) {
    frame(ui);
    KuiAccessNode nodes[64];
    size_t n = kui_access_tree(ui, nodes, 64);
    for (size_t i = 0; i < n && i < 64; i++) {
        if (nodes[i].key != edit) continue;
        if (to < from) return nodes[i].selection_start == nodes[i].selection_end;
        return nodes[i].selection_start == from && nodes[i].selection_end == to;
    }
    return false;
}

/* A scroller's y offset after a frame. */
static float offset(KuiCtx *ui, uint64_t key) {
    frame(ui);
    float x = 0, y = 0;
    kui_scroll_offset(ui, key, &x, &y);
    return y;
}

static KuiCtx *fresh(void) {
    KuiCtx *ui = kui_ctx_new();
    kui_set_smooth_wheel(ui, false); /* a notch lands at once, not over frames */
    frame(ui);
    frame(ui);
    return ui;
}
#endif

#ifdef HAVE_GLFW
static int host_buttons, host_cursors;
static void host_button(GLFWwindow *w, int button, int action, int mods) {
    (void)w, (void)button, (void)action, (void)mods;
    host_buttons++;
}
static void host_cursor(GLFWwindow *w, double x, double y) {
    (void)w, (void)x, (void)y;
    host_cursors++;
}

static void glfw_checks(void) {
    KuiCtx *ui = fresh();
    KuiGlfw g = {.ctx = ui};

    /* A move, a press and a release: the button's click. */
    kui_glfw_cursor(&g, 40, 12);
    kui_glfw_button(&g, GLFW_MOUSE_BUTTON_LEFT, GLFW_PRESS, 0);
    kui_glfw_button(&g, GLFW_MOUSE_BUTTON_LEFT, GLFW_RELEASE, 0);
    check(clicks_on_go(ui) == 1, "glfw: a press and release on the button click it");
    frame(ui);

    /* Two presses 0.1 s apart on the editor's first word: a double click,
     * the word; a third, the line; one after a pause, a single again. */
    kui_glfw_cursor(&g, 10, 36);
    now_s = 2.0;
    kui_glfw_button(&g, GLFW_MOUSE_BUTTON_LEFT, GLFW_PRESS, 0);
    kui_glfw_button(&g, GLFW_MOUSE_BUTTON_LEFT, GLFW_RELEASE, 0);
    now_s = 2.1;
    kui_glfw_button(&g, GLFW_MOUSE_BUTTON_LEFT, GLFW_PRESS, 0);
    kui_glfw_button(&g, GLFW_MOUSE_BUTTON_LEFT, GLFW_RELEASE, 0);
    check(g.clicks == 2 && selected(ui, 0, 5), "glfw: a double click counts 2 and selects the word");
    now_s = 2.2;
    kui_glfw_button(&g, GLFW_MOUSE_BUTTON_LEFT, GLFW_PRESS, 0);
    kui_glfw_button(&g, GLFW_MOUSE_BUTTON_LEFT, GLFW_RELEASE, 0);
    check(g.clicks == 3 && selected(ui, 0, 11), "glfw: a triple click counts 3 and selects the line");
    now_s = 3.0;
    kui_glfw_button(&g, GLFW_MOUSE_BUTTON_LEFT, GLFW_PRESS, 0);
    kui_glfw_button(&g, GLFW_MOUSE_BUTTON_LEFT, GLFW_RELEASE, 0);
    check(g.clicks == 1, "glfw: a press after a pause counts 1");
    now_s = 3.1;
    kui_glfw_cursor(&g, 30, 36);
    kui_glfw_button(&g, GLFW_MOUSE_BUTTON_LEFT, GLFW_PRESS, 0);
    kui_glfw_button(&g, GLFW_MOUSE_BUTTON_LEFT, GLFW_RELEASE, 0);
    check(g.clicks == 1, "glfw: a press 20 px from the last counts 1");
    /* Off the editor, whose context menu would take the pointer. */
    kui_glfw_cursor(&g, 300, 300);
    kui_glfw_button(&g, GLFW_MOUSE_BUTTON_RIGHT, GLFW_PRESS, 0);
    kui_glfw_button(&g, GLFW_MOUSE_BUTTON_RIGHT, GLFW_RELEASE, 0);
    check(g.clicks == 1 && g.click_x == 30, "glfw: another button counts nothing");
    clicks_on_go(ui);

    /* A notch over the inner scroller moves it to its limit; the next,
     * in the same gesture, stays with it; one after a pause begins a
     * gesture there, at its limit, which passes to the outer one. */
    kui_glfw_cursor(&g, 50, 60);
    frame(ui);
    now_s = 4.0;
    kui_glfw_scroll(&g, 0, -1);
    check(offset(ui, inner) == 40 && offset(ui, outer) == 0, "glfw: a notch moves the scroller under it");
    now_s = 4.05;
    kui_glfw_scroll(&g, 0, -1);
    check(offset(ui, inner) == 40 && offset(ui, outer) == 0, "glfw: a notch with no pause stays with its scroller");
    now_s = 4.5;
    kui_glfw_scroll(&g, 0, -1);
    check(offset(ui, outer) == 40, "glfw: after a pause the notch begins a gesture of its own");
    /* Back up, no pause: the outer scroller, latched - unless the pointer
     * moved, which begins a gesture at the inner one, which can go up. */
    now_s = 4.52;
    kui_glfw_cursor(&g, 51, 60);
    now_s = 4.55;
    kui_glfw_scroll(&g, 0, 1);
    check(offset(ui, inner) == 0 && offset(ui, outer) == 40, "glfw: the pointer moving begins a wheel's gesture");
#if defined(__APPLE__)
    /* macOS: a delta with a fraction is a trackpad's points over 10, and a
     * whole one in its gesture is too. */
    now_s = 5.0;
    kui_glfw_scroll(&g, 0, -0.5);
    check(offset(ui, inner) == 5, "glfw: a fraction on macOS is a swipe's points");
    now_s = 5.01;
    kui_glfw_scroll(&g, 0, -1);
    check(offset(ui, inner) == 15, "glfw: and a whole delta in the swipe is points too");
#endif

    /* Leaving the window, and coming back. */
    kui_glfw_cursor(&g, 40, 12);
    frame(ui);
    check(kui_pointer_wants(ui), "glfw: the pointer over the button is the UI's");
    kui_glfw_enter(&g, 0);
    frame(ui);
    check(!kui_pointer_wants(ui), "glfw: leaving the window, it is no longer");
    kui_glfw_enter(&g, 1);
    kui_glfw_cursor(&g, 40, 12);
    frame(ui);
    check(kui_pointer_wants(ui), "glfw: back over the button, it is again");
    clicks_on_go(ui);

    /* kui_glfw_install's callbacks: each finds its state by the window and
     * calls the host's callback before kui. */
    static char window_a, window_b;
    GLFWwindow *wa = (GLFWwindow *)(void *)&window_a, *wb = (GLFWwindow *)(void *)&window_b;
    KuiGlfw a = {.ctx = ui, .prev_button = host_button, .prev_cursor = host_cursor};
    KuiGlfw b = {.ctx = ui};
    kui_glfw__link(&a, wa);
    kui_glfw__link(&b, wb);
    check(kui_glfw__find(wa) == &a && kui_glfw__find(wb) == &b, "glfw: each window finds its own state");
    kui_glfw__on_cursor(wa, 40, 12);
    kui_glfw__on_button(wa, GLFW_MOUSE_BUTTON_LEFT, GLFW_PRESS, 0);
    kui_glfw__on_button(wa, GLFW_MOUSE_BUTTON_LEFT, GLFW_RELEASE, 0);
    check(host_cursors == 1 && host_buttons == 2, "glfw: the host's callbacks still fire");
    check(clicks_on_go(ui) == 1, "glfw: and kui hears the click after them");
    kui_glfw__on_button(wb, GLFW_MOUSE_BUTTON_LEFT, GLFW_PRESS, 0);
    kui_glfw__on_button(wb, GLFW_MOUSE_BUTTON_LEFT, GLFW_RELEASE, 0);
    check(host_buttons == 2, "glfw: another window's callbacks chain to its own");
    kui_glfw__unlink(&a);
    check(kui_glfw__find(wa) == NULL && kui_glfw__find(wb) == &b && a.window == NULL,
          "glfw: an uninstalled window is forgotten, the others kept");
    kui_glfw__on_button(wa, GLFW_MOUSE_BUTTON_LEFT, GLFW_PRESS, 0);
    check(host_buttons == 2, "glfw: and its callbacks do nothing");
    kui_glfw__unlink(&b);
    kui_ctx_free(ui);
}
#endif

#ifdef HAVE_SDL3
static SDL_Event motion(float x, float y) {
    SDL_Event e = {0};
    e.type = SDL_EVENT_MOUSE_MOTION;
    e.motion.x = x;
    e.motion.y = y;
    return e;
}

static SDL_Event button(uint8_t which, bool down, uint8_t clicks, float x, float y) {
    SDL_Event e = {0};
    e.type = down ? SDL_EVENT_MOUSE_BUTTON_DOWN : SDL_EVENT_MOUSE_BUTTON_UP;
    e.button.button = which;
    e.button.down = down;
    e.button.clicks = clicks;
    e.button.x = x;
    e.button.y = y;
    return e;
}

static SDL_Event wheel(float y, double at) {
    SDL_Event e = {0};
    e.type = SDL_EVENT_MOUSE_WHEEL;
    e.wheel.y = y;
    e.wheel.timestamp = (Uint64)(at * 1e9);
    return e;
}

/* A press and release of the left button with SDL's count. */
static bool sdl_click(KuiSdl3 *s, uint8_t clicks, float x, float y) {
    SDL_Event down = button(SDL_BUTTON_LEFT, true, clicks, x, y), up = button(SDL_BUTTON_LEFT, false, clicks, x, y);
    bool pressed = kui_sdl3_event(s, &down);
    bool released = kui_sdl3_event(s, &up);
    return pressed && released;
}

static void sdl3_checks(void) {
    KuiCtx *ui = fresh();
    KuiSdl3 s = {.ctx = ui};

    SDL_Event e = motion(40, 12);
    check(kui_sdl3_event(&s, &e), "sdl3: a motion is forwarded");
    check(sdl_click(&s, 1, 40, 12) && clicks_on_go(ui) == 1, "sdl3: a press and release on the button click it");
    frame(ui);

    /* SDL's count: 2 a word, 3 the line, 4 round to a caret. */
    e = motion(10, 36);
    kui_sdl3_event(&s, &e);
    sdl_click(&s, 1, 10, 36);
    sdl_click(&s, 2, 10, 36);
    check(selected(ui, 0, 5), "sdl3: SDL's double click selects the word");
    sdl_click(&s, 3, 10, 36);
    check(selected(ui, 0, 11), "sdl3: its triple click the line");
    sdl_click(&s, 4, 10, 36);
    check(selected(ui, 1, 0), "sdl3: and a fourth is a single click, a caret");
    clicks_on_go(ui);

    /* The wheel, by the events' timestamps. */
    e = motion(50, 60);
    kui_sdl3_event(&s, &e);
    frame(ui);
    e = wheel(-1, 10.0);
    check(kui_sdl3_event(&s, &e) && offset(ui, inner) == 40, "sdl3: a notch moves the scroller under it");
    e = wheel(-1, 10.05);
    kui_sdl3_event(&s, &e);
    check(offset(ui, inner) == 40 && offset(ui, outer) == 0, "sdl3: a notch with no pause stays with its scroller");
    e = wheel(-1, 10.5);
    kui_sdl3_event(&s, &e);
    check(offset(ui, outer) == 40, "sdl3: after a pause the notch begins a gesture of its own");

    /* Leaving the window, and coming back. */
    e = motion(40, 12);
    kui_sdl3_event(&s, &e);
    frame(ui);
    check(kui_pointer_wants(ui), "sdl3: the pointer over the button is the UI's");
    SDL_Event leave = {0};
    leave.type = SDL_EVENT_WINDOW_MOUSE_LEAVE;
    check(kui_sdl3_event(&s, &leave), "sdl3: the pointer leaving is forwarded");
    frame(ui);
    check(!kui_pointer_wants(ui), "sdl3: leaving the window, it is no longer the UI's");
    SDL_Event enter = leave;
    enter.type = SDL_EVENT_WINDOW_MOUSE_ENTER;
    kui_sdl3_event(&s, &enter);
    kui_sdl3_event(&s, &e);
    frame(ui);
    check(kui_pointer_wants(ui), "sdl3: back over the button, it is again");

    /* Keys and text go the same way; what it does not know, and another
     * window's events, it says it did not take. */
    kui_focus(ui, edit);
    frame(ui);
    SDL_Event key = {0};
    key.type = SDL_EVENT_KEY_DOWN;
    key.key.scancode = SDL_SCANCODE_END;
    key.key.key = SDLK_END;
    key.key.down = true;
    SDL_Event text = {0};
    text.type = SDL_EVENT_TEXT_INPUT;
    text.text.text = "!";
    check(kui_sdl3_event(&s, &key) && kui_sdl3_event(&s, &text), "sdl3: a key and its text are forwarded");
    KuiStr typed = {0};
    check(kui_edit_text(ui, edit, &typed) && typed.len == 12 && memcmp(typed.ptr, "hello world!", 12) == 0,
          "sdl3: End, then the text typed at the end");
    SDL_Event quit = {0};
    quit.type = SDL_EVENT_QUIT;
    check(!kui_sdl3_event(&s, &quit), "sdl3: an event it does not forward is not taken");
    s.window = 7;
    e = motion(40, 12);
    e.motion.windowID = 8;
    check(!kui_sdl3_event(&s, &e), "sdl3: nor is another window's");
    e.motion.windowID = 7;
    check(kui_sdl3_event(&s, &e), "sdl3: its own window's is");
    clicks_on_go(ui);
    kui_ctx_free(ui);
}
#endif

int main(void) {
    if (!abi_ok()) return 1;
    int ran = 0;
#ifdef HAVE_GLFW
    ran++;
    glfw_checks();
#else
    printf("pointer: no <GLFW/glfw3.h>; kui_glfw.h not checked\n");
#endif
#ifdef HAVE_SDL3
    ran++;
    sdl3_checks();
#else
    printf("pointer: no <SDL3/SDL.h>; kui_sdl3.h not checked\n");
#endif
    if (fails) {
        fprintf(stderr, "pointer: %d check(s) failed\n", fails);
        return 1;
    }
    printf("pointer: %d of 2 headers checked OK\n", ran);
    return 0;
}
