/* A host's own window, read by a screen reader (backlog F269): a GLFW
 * window with no GPU at all (GLFW_NO_API - the platform's accessibility
 * tree needs no pixels), kui.h's host loop on a small view, and
 * kui_access_attach on the window's native handle. VoiceOver, or anything
 * else asking the platform, finds the two buttons and the checkbox by
 * name; a press from it is a click here, and its focus is kui's focus.
 *
 *     cargo run -p kui-devtools --bin cbuild          builds it where GLFW is found
 *     ./target/debug/access_host                      the window, until it is closed
 *     ./target/debug/access_host --seconds 30         or for 30 seconds
 *     ./target/debug/access_host --check              attach and detach, and exit
 *
 * scripts/ax-host-audit.swift walks it with the macOS AX API, pressing and
 * focusing through the same calls VoiceOver makes, and closes it:
 *
 *     ./target/debug/access_host & swift scripts/ax-host-audit.swift $!; wait $!
 *
 * The view shows what the host saw, so the walk reads its answers back
 * through the platform (the count of harvests, the checkbox's state, the
 * focused control's name); the events are printed too, one line each, and
 * the exit status says which arrived: 1 a click on Harvest, 2 the checkbox
 * toggled, 4 focus on Plant, or-ed - 7 is all three; 100 is a refused
 * attach. --check exits 0 when its checks pass. */
#include <stdlib.h>
#include "../common.h"

#define GLFW_INCLUDE_NONE
#include <GLFW/glfw3.h>
#if defined(__APPLE__)
#define GLFW_EXPOSE_NATIVE_COCOA
#include <GLFW/glfw3native.h>
#elif defined(_WIN32)
#define GLFW_EXPOSE_NATIVE_WIN32
#include <GLFW/glfw3native.h>
#endif

/* The window's handle, as kui_access_attach takes it: the NSView on macOS,
 * the HWND on Windows, nothing on Linux (AT-SPI takes none). */
static void *native_handle(GLFWwindow *win) {
#if defined(__APPLE__)
    return (void *)glfwGetCocoaView(win);
#elif defined(_WIN32)
    return (void *)glfwGetWin32Window(win);
#else
    (void)win;
    return NULL;
#endif
}

typedef struct {
    int harvested;
    bool water;
    char focus[64]; /* the focused control's name, "none" */
    int saw;        /* the exit status: what arrived */
} Farm;

static void view(Farm *farm, KuiCtx *ui, float w, float h, float scale) {
    kui_frame_begin(ui, w, h, scale);
    KuiSpec root = {.width = {KUI_GROW, 1}, .height = {KUI_GROW, 1}, .gap = 10,
                    .pad_l = 16, .pad_r = 16, .pad_t = 16, .pad_b = 16};
    kui_root(ui, &root);
    KuiTextStyle text = {.size = 14};
    char line[96];
    snprintf(line, sizeof line, "Harvested %d", farm->harvested);
    kui_text(ui, KUI_STR(line), &text);
    snprintf(line, sizeof line, "Focus: %s", farm->focus);
    kui_text(ui, KUI_STR(line), &text);
    KuiSpec row = {.dir = KUI_ROW, .gap = 8};
    kui_open(ui, &row, NULL);
    kui_button(ui, KUI_STR("Harvest"), kui_value_str(KUI_STR("harvest")));
    kui_button(ui, KUI_STR("Plant"), kui_value_str(KUI_STR("plant")));
    kui_close(ui);
    KuiSpec box = {.checked = farm->water};
    kui_checkbox(ui, KUI_STR("Auto-water"), &box, kui_value_str(KUI_STR("water")));
    kui_frame_finish(ui);
}

/* Drains the queue into the model; whether anything changed. A press from
 * a screen reader is applied by the context at this poll (or at the next
 * input or frame_begin) and arrives here as the click a mouse makes. */
static bool drain(Farm *farm, KuiCtx *ui) {
    bool changed = false;
    KuiEvent ev = KUI_EVENT_INIT;
    while (kui_poll_event(ui, &ev)) {
        KuiStr msg = {0};
        if (!kui_event_message(&ev, &msg)) continue;
        if (kui_str_eq(msg, "harvest")) {
            farm->harvested++;
            farm->saw |= 1;
            printf("click Harvest (%d)\n", farm->harvested);
        } else if (kui_str_eq(msg, "plant")) {
            printf("click Plant\n");
        } else if (kui_str_eq(msg, "water")) {
            farm->water = !farm->water;
            farm->saw |= 2;
            printf("toggle Auto-water %s\n", farm->water ? "on" : "off");
        } else {
            continue;
        }
        changed = true;
    }
    return changed;
}

/* The focused control's name, off the access tree the platform reads. */
static bool note_focus(Farm *farm, KuiCtx *ui) {
    KuiAccessNode nodes[32];
    size_t n = kui_access_tree(ui, nodes, sizeof nodes / sizeof nodes[0]);
    char now[64] = "none";
    for (size_t i = 0; i < n && i < 32; i++) {
        if ((nodes[i].flags & KUI_ACCESS_FOCUSED) && nodes[i].name.len > 0) {
            snprintf(now, sizeof now, "%.*s", (int)nodes[i].name.len, (const char *)nodes[i].name.ptr);
        }
    }
    if (strcmp(now, farm->focus) == 0) return false;
    snprintf(farm->focus, sizeof farm->focus, "%s", now);
    if (strcmp(now, "Plant") == 0) farm->saw |= 4;
    printf("focus %s\n", now);
    return true;
}

/* --check: a window that is never shown, attached, detached and attached
 * again (the view restored and taking a new bridge), for a round with a
 * display and no AX client. */
static int check_only(GLFWwindow *win) {
    KuiCtx *ui = kui_ctx_new();
    Farm farm = {.focus = "none"};
    view(&farm, ui, 360, 200, 1);
    check(kui_access_attach(ui, native_handle(win)), "kui_access_attach takes the window");
    check(kui_owed(ui) == 0, "and owes nothing while nobody asks");
    view(&farm, ui, 360, 200, 1);
    kui_access_detach(ui);
    check(kui_access_attach(ui, native_handle(win)), "a detached window takes a bridge again");
    KuiCtx *other = kui_ctx_new();
#if defined(__APPLE__)
    check(!kui_access_attach(other, native_handle(win)), "a view takes one bridge");
#endif
    kui_ctx_free(other);
    kui_ctx_free(ui); /* detaches */
    if (fails) {
        fprintf(stderr, "access_host: %d check(s) failed\n", fails);
        return 1;
    }
    printf("access host OK: attached, detached, attached again\n");
    return 0;
}

int main(int argc, char **argv) {
    if (!abi_ok()) return 1;
    bool only_check = false;
    double seconds = 0;
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--check") == 0) only_check = true;
        else if (strcmp(argv[i], "--seconds") == 0 && i + 1 < argc) seconds = atof(argv[++i]);
    }
    if (!glfwInit()) {
        /* No display (a headless CI runner): nothing to attach to. */
        printf("access_host: skipped, GLFW has no display\n");
        return 0;
    }
    glfwWindowHint(GLFW_CLIENT_API, GLFW_NO_API);
    /* Hidden until attached: Windows' adapter refuses a shown window. */
    glfwWindowHint(GLFW_VISIBLE, GLFW_FALSE);
    GLFWwindow *win = glfwCreateWindow(360, 200, "kui access host", NULL, NULL);
    if (!win) {
        printf("access_host: skipped, GLFW made no window\n");
        glfwTerminate();
        return 0;
    }
    if (only_check) {
        int code = check_only(win);
        glfwDestroyWindow(win);
        glfwTerminate();
        return code;
    }

    KuiCtx *ui = kui_ctx_new();
    Farm farm = {.focus = "none"};
    if (!kui_access_attach(ui, native_handle(win))) {
        fprintf(stderr, "access_host: kui_access_attach refused the window\n");
        return 100;
    }
    glfwShowWindow(win);
    printf("attached\n");
    fflush(stdout);

    double end = seconds > 0 ? glfwGetTime() + seconds : 0;
    while (!glfwWindowShouldClose(win) && (end == 0 || glfwGetTime() < end)) {
        /* Asleep until something happens: input, or a screen reader's ask,
         * which wakes this wait on macOS and Windows. On Linux the ask
         * arrives on the bridge's own thread, so the wait is bounded. */
        double wait = end > 0 ? end - glfwGetTime() : 1.0;
#if !defined(__APPLE__) && !defined(_WIN32)
        if (wait > 0.1) wait = 0.1;
#endif
        if (kui_owed(ui)) glfwPollEvents();
        else if (wait > 0) glfwWaitEventsTimeout(wait);

        int fw, fh, ww, wh;
        glfwGetFramebufferSize(win, &fw, &fh);
        glfwGetWindowSize(win, &ww, &wh);
        float scale = ww > 0 ? (float)fw / (float)ww : 1;
        kui_env_set(ui, 0, glfwGetWindowAttrib(win, GLFW_FOCUSED));

        drain(&farm, ui);
        view(&farm, ui, (float)ww, (float)wh, scale);
        /* What the frame raised, and the focus a client moved: either
         * changes what the view says, so the frame is built again. */
        bool again = drain(&farm, ui);
        again |= note_focus(&farm, ui);
        if (again) view(&farm, ui, (float)ww, (float)wh, scale);
        fflush(stdout);
    }

    kui_access_detach(ui);
    kui_ctx_free(ui);
    glfwDestroyWindow(win);
    glfwTerminate();
    printf("saw %d\n", farm.saw);
    return farm.saw;
}
