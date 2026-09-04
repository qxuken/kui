/* kui with C as the *extension*, not the host.
 *
 * counter.c in this directory is the other direction: C owns main(), calls
 * kui_run and links libkui_ffi. Here the host is a Rust app
 * (examples/c/panel.rs) that owns the window and the left
 * side of the frame, and this file is a shared library it dlopens. The
 * panel below draws into the host's frame, keeps its own state and gets
 * its own clicks back - the host never sees them, and this file never sees
 * the host's.
 *
 * It links against nothing: every kui_* call here is left undefined and
 * resolved from the host executable at load, the way a Lua C module
 * resolves lua_*. See ../../crates/kui-ffi/src/ext.rs for the loader and
 * ./build.sh for the two flags each side needs.
 *
 * The same panel as examples/lua/panel.lua, deliberately: the
 * extension contract is the contract, and the language is a detail.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "kui.h"

#define MAX_TODOS 32
#define TODO_LEN 64

typedef struct Panel {
    char todos[MAX_TODOS][TODO_LEN];
    int done[MAX_TODOS];
    int count;
} Panel;

/* -- small helpers ------------------------------------------------------- */

static bool is(KuiStr s, const char *lit) {
    size_t n = strlen(lit);
    return s.len == n && memcmp(s.ptr, lit, n) == 0;
}

/* Substring test against a borrowed KuiStr, which is not NUL-terminated. */
static bool contains(const char *hay, KuiStr needle) {
    if (needle.len == 0) return true;
    size_t n = strlen(hay);
    if (needle.len > n) return false;
    for (size_t i = 0; i + needle.len <= n; i++)
        if (memcmp(hay + i, needle.ptr, needle.len) == 0) return true;
    return false;
}

static KuiValue *msg(const char *kind) {
    KuiValue *m = kui_value_map();
    kui_value_map_set(m, KUI_STR("kind"), kui_value_str(KUI_STR(kind)));
    return m;
}

/* -- the extension ------------------------------------------------------- */

uint32_t kui_ext_abi(void) { return KUI_ABI_VERSION; }

const char *kui_ext_name(void) { return "c panel"; }

void *kui_ext_init(void) {
    Panel *p = calloc(1, sizeof *p);
    if (!p) return NULL;
    static const char *seed[] = {"ship the layout solver", "wire up wgpu",
                                 "write this panel"};
    for (size_t i = 0; i < sizeof seed / sizeof *seed; i++)
        snprintf(p->todos[p->count++], TODO_LEN, "%s", seed[i]);
    return p;
}

void kui_ext_free(void *user) { free(user); }

void kui_ext_view(void *user, KuiCtx *ui) {
    Panel *p = (Panel *)user;
    if (!p) return;

    /* Opened as a child of whatever the host's root is - extensions append
     * after the host's own view, so this lands to the right of it. */
    KuiSpec panel = {
        .dir = KUI_COLUMN,
        .width = {KUI_FIXED, 300}, .height = {KUI_GROW, 1},
        .pad_l = 16, .pad_r = 16, .pad_t = 16, .pad_b = 16,
        .gap = 10,
        .bg = 0x14161eff, .radius = 10,
        .border_w = 1, .border_color = 0x2a2d3aff,
    };
    kui_open(ui, &panel, NULL);
    {
        int remaining = 0;
        for (int i = 0; i < p->count; i++)
            if (!p->done[i]) remaining++;
        char left[32];
        snprintf(left, sizeof left, " · %d left", remaining);
        KuiSpan head[] = {
            {KUI_STR("c panel"), 0, 0},
            {KUI_STR(left), 0x8a8fa3ff, 0},
        };
        KuiTextStyle muted = {.size = 12, .color = 0x8a8fa3ff};
        kui_rich_text(ui, head, 2, &muted);

        /* Declared before the list, so the filter it holds is readable in
         * time to build one. The core retains the text under the returned
         * key, and the key is stable across frames (it is hashed from the
         * label) - so the filter is a query, not state this file keeps.
         * panel.lua has to stash the key because it asks before declaring;
         * here the declaration comes first and the key never outlives the
         * frame. */
        uint64_t filter_key = kui_text_input(ui, KUI_STR("filter"), KUI_STR(""));
        KuiStr filter = {0};
        kui_edit_text(ui, filter_key, &filter);

        KuiSpec list = {
            .dir = KUI_COLUMN,
            .height = {KUI_GROW, 1},
            .overflow = KUI_SCROLL_Y,
        };
        kui_open(ui, &list, NULL);
        for (int i = 0; i < p->count; i++) {
            if (!contains(p->todos[i], filter)) continue;

            /* Consumed by kui_open; routed back to kui_ext_on_event because
             * the runner tagged this node with our origin, not the host's. */
            KuiValue *toggle = msg("toggle");
            kui_value_map_set(toggle, KUI_STR("index"), kui_value_int(i));
            KuiSpec row = {
                .dir = KUI_ROW, .gap = 8, .cross_align = KUI_CENTER,
                .pad_t = 4, .pad_b = 4,
                .tooltip = p->done[i] ? KUI_STR("click to reopen")
                                      : KUI_STR("click to finish"),
            };
            kui_open(ui, &row, toggle);
            {
                char line[TODO_LEN + 8];
                snprintf(line, sizeof line, "%s %s", p->done[i] ? "[x]" : "[ ]",
                         p->todos[i]);
                KuiTextStyle style = {
                    .size = 14,
                    .color = p->done[i] ? 0x5c6174ff : 0xe8e8eaff,
                };
                kui_text(ui, KUI_STR(line), &style);
            }
            kui_close(ui);
        }
        kui_close(ui);

        KuiSpec buttons = {.dir = KUI_ROW, .gap = 8};
        kui_open(ui, &buttons, NULL);
        {
            kui_button(ui, KUI_STR("add"), msg("add"));
            kui_button(ui, KUI_STR("clear done"), msg("clear"));
        }
        kui_close(ui);
    }
    kui_close(ui);
}

void kui_ext_on_event(void *user, const KuiEvent *ev) {
    Panel *p = (Panel *)user;
    if (!p || !ev->payload) return;
    const KuiValue *kind = kui_value_get(ev->payload, KUI_STR("kind"));
    KuiStr s;
    if (!kind || !kui_value_as_str(kind, &s)) return;

    if (is(s, "toggle")) {
        const KuiValue *index = kui_value_get(ev->payload, KUI_STR("index"));
        int64_t i = 0;
        if (index && kui_value_as_int(index, &i) && i >= 0 && i < p->count)
            p->done[i] = !p->done[i];
    } else if (is(s, "add")) {
        if (p->count < MAX_TODOS) {
            snprintf(p->todos[p->count], TODO_LEN, "todo #%d", p->count + 1);
            p->done[p->count] = 0;
            p->count++;
        }
    } else if (is(s, "clear")) {
        int kept = 0;
        for (int i = 0; i < p->count; i++) {
            if (p->done[i]) continue;
            if (kept != i) memcpy(p->todos[kept], p->todos[i], TODO_LEN);
            p->done[kept++] = 0;
        }
        p->count = kept;
    }
    /* {kind="changed"} from the filter editor lands here too, and needs
     * nothing: view() reads the editor's text back every frame. */
}
