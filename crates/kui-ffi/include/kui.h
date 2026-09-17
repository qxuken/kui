/* kui C API — see crates/kui-ffi/src/ for the implementation (types.rs
 * mirrors these structs; each kui_* function's module is named for its
 * concern).
 *
 * Conventions:
 *  - Strings are UTF-8 (ptr, len) pairs; use KUI_STR("literal").
 *  - KuiValue* from kui_value_* constructors is owned by you until passed to
 *    a function documented as consuming it (on_click args, map_set values).
 *  - Event payloads from kui_poll_event are borrowed: valid until the next
 *    poll on the same context. Payloads inside callbacks are borrowed for the
 *    duration of the callback.
 *  - Coordinates are logical pixels; draw data comes back in physical pixels.
 *  - Every struct below is marked [in], [out], [out[]] or [lib], which says
 *    who allocates it and who writes it. Read "Who writes what" first.
 */
#ifndef KUI_H
#define KUI_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

/* -- ABI version ----------------------------------------------------------
 *
 * KUI_ABI_VERSION is the ABI this header describes; kui_abi_version() is the
 * one the library you actually loaded implements. They are settled against
 * each other at build time only if you built both, which a host that links
 * a shared libkui_ffi did not. Check them once, before anything else:
 *
 *     if (kui_abi_version() != KUI_ABI_VERSION) {
 *         fprintf(stderr, "libkui is ABI %u, this build wants %u\n",
 *                 kui_abi_version(), KUI_ABI_VERSION);
 *         return 1;
 *     }
 *
 * Equality, not >=: the mismatch this exists to catch is a *newer* library
 * against an older host, which is the direction that corrupts memory rather
 * than merely missing a feature.
 *
 * The number bumps when the layout of any struct below changes - [in],
 * [out], [out[]] or [lib], in any way, appended fields included - and
 * when an existing function's signature changes (ABI 12, ABI 16). It does
 * not bump for a new function: a host that does not call one is
 * unaffected, and one that does fails to link, which is loud.
 *
 * Until 2026-09-14 this block exempted an [in] append: "the library only
 * reads those, so a host that predates the field passes a shorter struct
 * and gets the zeroed default". The library does no such thing. It reads
 * the whole struct - kui_open copies *spec, kui_window_declare reads
 * every field of its config - so a host that reserved the shorter
 * KuiSpec had the new field read from whatever followed it on its stack:
 * for a
 * KuiStr, a pointer of garbage, dereferenced when its length was not zero
 * (backlog AR50). A size the host writes is what makes an append
 * safe, and only the [out] structs carry one; giving every KuiSpec
 * literal a `size` was the tax ADR 0006 declined, and the amendment
 * there takes the other way out: one rule for every struct, and an [in]
 * append is a recompile - which every host in this tree did anyway,
 * having always been built against the header it linked. The appends
 * KuiSpec, KuiTextStyle and KuiAudio took after ABI 9, 11, 13, 14 and 15
 * went without a bump under the old rule; they are behind ABI 16 now, so
 * a host that checks the number is served from here on.
 *
 * It bumps per change, not per release, so what follows is a log of breaks
 * and not a list of published versions. 1 through 5 all came and went
 * between two releases and no release carried any of them: this scheme
 * landed after 0.1.0-alpha.5, and 0.1.0-alpha.6 is the first version to
 * have a number at all - 6. A gap is normal, and a number you never saw
 * published is one nothing was published against. It costs you nothing,
 * because the check above is equality: you compare your header's number
 * with the library you loaded, and never reason about the distance between
 * two. The entries below are kept so a host crossing several bumps at once
 * can read what each of them changed. The same log lives beside the
 * constant it describes, in crates/kui-ffi/src/abi.rs; a bump writes an
 * entry in both, and this is the copy a C host reads.
 *
 * ABI 4 was the first bump to append to an [out] struct: KuiEvent gained
 * `window`. If you set `size` (KUI_EVENT_INIT does) you need no source
 * change for it - the library writes the prefix your build reserved and
 * stops. The version still bumps, because a host that skipped this check
 * would otherwise get that short write without ever having asked for it.
 *
 * ABI 5 is multi-window (docs/adr/0004-multi-window.md, step 3), and its
 * two breaks are source breaks you see at compile time: the uint32_t array
 * kui_take_window_commands filled became the KuiWindowCommand [out] struct
 * kui_take_window_command pops (a command now names its window, and an
 * open carries a config), and kui_env_set_window leads with the window id.
 * Edit the drain loop and the env call; nothing else changes meaning.
 *
 * ABI 6 appends width/height to KuiWindowCommand, for the KUI_CMD_SET_SIZE
 * kui_set_window_size queues (ADR 0004 step 5). Set `size` (as
 * KUI_WINDOW_COMMAND_INIT does) and you need no source change: the library
 * writes the prefix your build reserved and stops. Nor can the new verb
 * reach a host that never calls kui_set_window_size - only that call
 * produces it. The version bumps for the host that skipped this check.
 *
 * ABI 7 is the popup (ADR 0004 step 4): KuiWindowConfig gains the four
 * anchor_* floats a popup is placed against, and KuiWindowCommand appends
 * owner. This is the first growth `size` cannot absorb (ABI 11 is the
 * second), and it is worth knowing why: KuiWindowCommand embeds a
 * KuiWindowConfig by value, so appending inside the config moves
 * everything after it and lifts the floor kui_take_window_command will
 * accept past the whole size of the ABI-6 struct. A binary that was not
 * recompiled is refused rather than short-written - its drain loop sees
 * an empty queue instead of its windows - so this is the release where
 * checking kui_abi_version() first is the difference between a message
 * and a mystery. Recompile and nothing in your source changes.
 *
 * ABI 8 appends bg to KuiSpan (backlog C22). An [in] struct, but one that
 * travels as an array - kui_rich_text and kui_measure_rich_text take
 * `const KuiSpan *spans, size_t span_count` - so the append moved the
 * stride, and a binary that was not recompiled would hand the library
 * elements it reads at the wrong places. Recompile and nothing in your
 * source changes; a zeroed bg is none.
 *
 * ABI 9 appends fragments, fragment_count and time to KuiDrawData, for
 * the fragment element (ADR 0015). KuiDrawData leads with `size`, so this
 * is the compatible kind of append: set it (as KUI_DRAW_DATA_INIT does)
 * and the library writes the prefix your build reserved and stops, so a
 * host that reserved through atlas_epoch keeps drawing frames and never
 * sees the three new fields. It has nothing to miss either, unless it
 * asked - only kui_fragment_add and kui_fragment produce a
 * KUI_QUAD_FRAGMENT quad, and a plugin that calls them on your context is
 * one whose kui_ext_abi the host already matched against this same
 * number. The version bumps for the host that skipped the check.
 *
 * ABI 10 appends reply_sink to KuiEvent, and it is the one entry here
 * that does not bump for the host's sake: another size-led [out] append,
 * so a host reserving the ABI-9 layout keeps polling correctly and simply
 * never sees the field - which is right, because the field is not for a
 * host to read. The bump is for the other side of an extension. kui_reply
 * used to find its sink in a thread_local, which is one sink per copy of
 * this library in the process, and a plugin does not always share the
 * host's copy - on Windows it cannot, since a DLL may not leave kui_reply
 * undefined and resolve it from the executable the way ELF does, and
 * every reply then landed in a list nobody read. The sink now travels on
 * the event as a pointer into the copy that opened it. No plugin's source
 * changes; a plugin binary built against ABI 9 must not be handed an
 * ABI 10 event, and kui_ext_abi is where that is refused.
 *
 * ABI 11 takes the clip off KuiQuad and puts it behind an index into the
 * new KuiDrawData.clips. This is the second growth `size` cannot absorb
 * (ABI 7 was the first), for the other reason: KuiQuad is [lib], an array
 * you stride with your own sizeof, so KuiDrawData's handshake says
 * nothing about its elements. The struct got 28 bytes shorter and every
 * field after `kind` moved, which a binary that was not recompiled reads
 * as garbage from quad 1 onward whatever quad 0 looked like - read-only
 * garbage, so it draws nonsense rather than corrupting anything, and
 * nothing refuses the frame the way an [out] reservation too small to
 * write is refused. kui_abi_version() is the whole of the warning here.
 * The break buys its keep: the clip was a rect and four radii carried on
 * a struct written once per quad and then walked again by the fade pass,
 * by the backend's upload and by the previous frame `depart` keeps, for a
 * value nearly every quad of a frame shares. Your source changes in one
 * place - read dd.clips[q.clip] where you read q.clip and q.clip_radius;
 * entry zero clips nothing, so there is no null case.
 *
 * ABI 12 appends origin_line to kui_cells (ADR 0017 decision 4): the
 * absolute line a grid's row 0 is, so a terminal's selection keeps its
 * ends across a scroll. This is the case the rules above do not cover -
 * not an [out] struct's layout, not an [in] struct's append, not a new
 * function, but an existing function's *signature*. A host that does not
 * recompile passes one argument too few and the library reads whatever is
 * in that register, which is exactly the silent failure the version check
 * exists to turn into a message. Recompile and pass 0 to keep what you
 * had.
 *
 * ABI 13 appends `checked` to KuiMenuItem (the menu bar, ADR 0018): a row
 * that is a setting rather than a command draws a checkmark. An [in]
 * struct, which the rule as it then stood exempted - but this one
 * travels as an ARRAY, so the append moves the stride and every row after
 * the first is read from the wrong bytes. Same reason ABI 8 bumped for
 * KuiSpan. Recompile; a zeroed tail is `checked = 0`, which is what every
 * row had before.
 *
 * ABI 14 appends `textures` and `texture_count` to KuiDrawData for ADR
 * 0025's texture-backed images - an [out] append the size handshake
 * covers, so a host reserving the ABI-13 layout keeps working and never
 * sees a KUI_QUAD_TEXTURE quad's side entry (it draws that quad as a solid,
 * wrongly and harmlessly, as a pre-segment host draws a segment). The bump
 * is for KUI_QUAD_TEXTURE itself: a ninth kind a host's own renderer may
 * want to refuse by version rather than meet by surprise.
 *
 * ABI 15 appends image_source, image_texture and image_uv to
 * KuiFragmentDraw, for the fragment image input (kui_fragment_with). An
 * ARRAY element again, so the append moves the stride - the KuiSpan and
 * KuiMenuItem exception, for the same reason. Recompile; a host that never
 * reads `fragments` has nothing to change, and one that does now has a
 * texture to bind for a draw whose image_source says so. The same version
 * adds KuiMetrics with kui_metrics / kui_metrics_set (a new [out] struct,
 * no bump of its own), and, still at 15, KuiColorToken / KuiLengthToken
 * with kui_tokens_set, kui_token_color and kui_token_length (two new [in]
 * arrays and three functions - nothing the library writes moved).
 * Still at 15: KuiColorOp / KuiDerivedToken with kui_tokens_derive (ADR
 * 0028) - two more [in] arrays and one function. And still at 15, the
 * verb table's C column (backlog B1a): kui_cell_selection, kui_set_inspect,
 * kui_nodes, kui_devtools, kui_devtools_dock, kui_set_devtools_theme and
 * kui_set_devtools_legend - seven functions, no struct.
 *
 * ABI 16 gives kui_run_with a KuiRunConfig (backlog AR27): a third
 * argument, between the title and the view. The struct is new, so no
 * layout a host had moved; the bump is ABI 12's case again - an existing
 * function's *signature* - since a host that did not recompile passes one
 * argument too few and the library reads its view callback out of the
 * register the config should be in. The same number stands over the
 * rule change above (backlog AR50): an [in] append bumps from here on. Recompile: kui_run is unchanged, and
 * kui_run_with(ctx, title, NULL, view, on_event, user) is what the five-
 * argument call was.
 *
 * ABI 17 appends the underline's own colour and shape to KuiTextStyle and
 * KuiSpan (underline_color, underline_style) and `ul` to KuiCell - three
 * [in] appends under the withdrawn rule, two of them array elements whose
 * stride moved. Recompile; a zeroed field is what the struct meant before.
 *
 * ABI 18 appends on_drop and drop_bg to KuiSpec for the drop zone (ADR
 * 0031) - the first [in] append under the amended rule: the library reads
 * the whole struct, so a host that did not recompile would have the two
 * read from past its end. Recompile; a zeroed tail is no zone and no
 * colour. The same version adds kui_input_drag_files, kui_input_drop_files,
 * kui_input_drag_cancel, kui_is_drop_target and kui_drop_target - five
 * functions, nothing the library writes moved. Still at 18:
 * kui_set_devtools_tab and kui_devtools_current_tab - two functions, no
 * struct - and kui_press_and_hold, one more.
 */
#define KUI_ABI_VERSION 18u
uint32_t kui_abi_version(void);

/* -- Who writes what ------------------------------------------------------
 *
 * [in]     You allocate and fill it; the library reads it. Zero-initialize
 *          and set what you need - a zeroed field is the documented default.
 *          A later kui may append fields, and when it does it bumps
 *          KUI_ABI_VERSION: the library reads the whole struct, so a
 *          shorter one is not fine (the ABI block says why this once
 *          claimed otherwise). Recompile, and the zeroed new field is
 *          its default.
 *          KuiSpec, KuiSizing, KuiKeyframe, KuiEnter, KuiTextStyle, KuiSpan,
 *          KuiCell, KuiMenuItem, KuiMenu, KuiPlay, KuiAudio, KuiWindowConfig,
 *          KuiRunConfig, KuiColorToken, KuiLengthToken, KuiColorOp,
 *          KuiDerivedToken.
 *
 * [out]    You allocate it; the library WRITES it. These lead with a
 *          `uint32_t size` you set to sizeof the struct, and the library
 *          writes no further than that - which is what lets a later kui
 *          append a field without writing past the end of what your build
 *          reserved. Start from the KUI_*_INIT initializer:
 *
 *              KuiEvent ev = KUI_EVENT_INIT;
 *              while (kui_poll_event(ctx, &ev)) { ... }
 *
 *          A call whose `size` is below the ABI-1 layout (which is what a
 *          zeroed or never-set one looks like) writes nothing and returns
 *          false, rather than guessing; it also consumes nothing, so a
 *          refused kui_poll_event leaves the event queued (the payload from
 *          the previous poll is released either way). On return
 *          `size` holds how many bytes were filled, which is stable across
 *          a loop that reuses one struct.
 *          KuiEvent, KuiDrawData, KuiTextMetrics, KuiScrollGeometry,
 *          KuiWindowCommand, KuiMenuAction, KuiTextHit, KuiCaretRect,
 *          KuiLayoutRect, KuiTheme, KuiMetrics.
 *
 * [out[]]  You allocate an ARRAY; the library fills up to `cap` elements.
 *          A `size` field cannot help here: the library strides by its own
 *          sizeof, so element 1 lands past your element 1 no matter what
 *          element 0 says, and the write happens before any in-band
 *          handshake could be read. If one of these ever has to grow, it
 *          grows by gaining an explicit stride argument - a source break
 *          every host sees - and by bumping KUI_ABI_VERSION. Until then the
 *          version check is the whole guard.
 *          KuiAccessNode, KuiAccessRun, KuiWarning, KuiAudioCommand,
 *          KuiAnnouncement.
 *
 * [lib]    The library allocates it; you read it. KuiQuad, KuiClip,
 *          KuiFragmentDraw and KuiTextureDraw, through KuiDrawData.quads /
 *          .clips / .fragments / .textures. The same stride problem mirrored - you walk the
 *          array with your sizeof - except read-only, so a mismatch
 *          misreads every element after the first rather than corrupting
 *          anything. Guarded by KUI_ABI_VERSION.
 *
 * KuiStr is the exception and is frozen: it crosses both ways (kui_edit_text
 * and kui_value_as_str write one) and will never be more than (ptr, len).
 * KuiEvent also reaches kui_run's callback as a library-owned
 * `const KuiEvent *`; one struct behind a pointer is safe to append to, so
 * it is the [out] use above that pins the type.
 */

typedef struct KuiCtx KuiCtx;
typedef struct KuiValue KuiValue;
/* Where a reply goes, carried on the event an extension's kui_ext_on_event
 * is handed and passed straight back to kui_reply. Opaque on purpose: it is
 * a function pointer into the copy of the library that opened it, which is
 * how a reply reaches the host even when the plugin was linked against a
 * different copy - the case Windows forces, since a DLL may not leave
 * kui_reply undefined and take it from the executable the way ELF does.
 * Never allocate, dereference or store one. */
typedef struct KuiReplySink KuiReplySink;

typedef struct KuiStr {
    const uint8_t *ptr;
    size_t len;
} KuiStr;

#define KUI_STR(s) ((KuiStr){(const uint8_t *)(s), strlen(s)})

/* Whether a borrowed string is `lit`. Every string this API hands back is a
 * (ptr, len) pair into memory the library owns, NUL-terminated by nothing,
 * so `strcmp` is wrong on it and every C host writes this on its first day
 * - reading an event's `kind`, a slot's name. Here once instead. */
static inline bool kui_str_eq(KuiStr s, const char *lit) {
    size_t n = strlen(lit);
    return s.len == n && memcmp(s.ptr, lit, n) == 0;
}

/* Sizing tags */
enum { KUI_FIT = 0, KUI_GROW = 1, KUI_FIXED = 2, KUI_PERCENT = 3 };
/* KuiSpec.min_w / min_h: the node's own fit size as its floor (`minWidth:
 * "fit"` elsewhere) - a grow child that never goes below its content. Any
 * negative min means this; the name is the one to write. */
#define KUI_MIN_FIT (-1.0f)
/* Directions. KUI_TABLE is a column whose rows' children line up in
 * columns (docs/adr/0033): the nth in-flow child of every row is a cell of
 * column n, and a column is as wide as its widest cell, so a label column
 * sits at its longest label with no width picked by hand. A cell's width
 * sizes its column (KUI_FIT and KUI_FIXED are content, KUI_GROW grows the
 * column, KUI_PERCENT takes its cut of the row), its min_w / max_w clamp
 * it, and a bare kui_text in a row is a cell held to its column. The rows
 * are rows - give them {KUI_GROW, 1} for the columns to grow into - with
 * their own gap, padding, bg, hover and click; a row of a table never
 * wraps. Everything else is the column's: gap is between rows, scroll y
 * scrolls them. */
enum { KUI_COLUMN = 0, KUI_ROW = 1, KUI_TABLE = 2 };
/* Alignment */
enum { KUI_START = 0, KUI_CENTER = 1, KUI_END = 2 };
/* Quad kinds */
/* KUI_QUAD_GLYPH_SUBPIXEL: atlas rgb are per-channel coverages (needs
 * per-channel / dual-source blending; else use the atlas alpha as a mask).
 * Only produced after kui_set_subpixel_text(ctx, true). */
/* KUI_QUAD_SHADOW: `color` fills a rounded rect inset from the quad by
   `blur` on every side, its edge ramped over `blur` px; ignores uv,
   border_color and border_w. */
/* KUI_QUAD_SEGMENT: a round-capped stroke between two endpoints. `uv` holds
 * them as float bits - x0, y0, x1, y1 in physical px, memcpy each uint32_t
 * into a float - `border_w` is the stroke width and `color` the stroke; the
 * quad is the bounding box padded past the edge ramp. A renderer evaluates
 * the capsule SDF against the fragment position. Ignores radius,
 * border_color and blur. (docs/adr/0010-a-segment-primitive.md) */
/* KUI_QUAD_FRAGMENT: a box a registered WGSL function paints. `uv[0]` is an
 * index into KuiDrawData.fragments, which carries the handle and the
 * sixteen parameters; the other three words are zero. rect, radius, clip
 * and clip_radius are the node's and mean what they always do. `color.a`
 * is the group opacity and the rest of `color` is unused, because the
 * fragment returns its own colour; border_color, border_w and blur are
 * zero. Get the WGSL with kui_fragment_source, which wraps the app's
 * function in the prelude and epilogue the core validated it against.
 * (docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md) */
/* KUI_QUAD_TEXTURE: a registered image drawn from a texture of its own
 * rather than the atlas - one that did not fit a page, or whose pixels
 * kui_image_update replaced. `uv[0]` is an index into
 * KuiDrawData.textures, which carries the handle, the pixels' size and
 * revision, and the texel rect to show IN THAT TEXTURE; the other three
 * words are zero. Everything else is what a KUI_QUAD_IMAGE's is: get the
 * bytes with kui_image_pixels, upload them when `rev` moved, bind that
 * texture in the atlas's place and draw it as an image. On both image
 * kinds `border_w` is the `sampling` flag: 0 linear, 1 nearest.
 * (docs/adr/0025-the-image-is-the-canvas.md) */
enum { KUI_QUAD_SOLID = 0, KUI_QUAD_GLYPH_MASK = 1, KUI_QUAD_GLYPH_COLOR = 2,
       KUI_QUAD_IMAGE = 3, KUI_QUAD_GLYPH_SUBPIXEL = 4, KUI_QUAD_SHADOW = 5,
       KUI_QUAD_SEGMENT = 6, KUI_QUAD_FRAGMENT = 7, KUI_QUAD_TEXTURE = 8 };
/* How an image's texels are read between pixels (kui_image_with) */
enum { KUI_SAMPLING_LINEAR = 0, KUI_SAMPLING_NEAREST = 1 };
/* How an image's pixels meet its box (kui_image_with): stretched, the
 * largest rect of its aspect that fits (centred), or the box filled and
 * the rest cropped (centred). The box itself is the same in every mode. */
enum { KUI_FIT_FILL = 0, KUI_FIT_CONTAIN = 1, KUI_FIT_COVER = 2 };
/* Font families (KuiTextStyle.family) */
enum { KUI_FONT_SANS = 0, KUI_FONT_SERIF = 1, KUI_FONT_MONO = 2 };
/* Line breaking (KuiTextStyle.wrap) */
enum { KUI_WRAP_WORD = 0, KUI_WRAP_GLYPH = 1, KUI_WRAP_NONE = 2 };
/* Span flags */
enum {
    KUI_SPAN_BOLD = 1u << 0,
    KUI_SPAN_ITALIC = 1u << 1,
    KUI_SPAN_UNDERLINE = 1u << 2,     /* a line under the span, where the face puts it */
    KUI_SPAN_STRIKETHROUGH = 1u << 3, /* a line through it */
};
/* KuiTextStyle.decoration: the same two lines over a whole text. */
enum { KUI_DECO_UNDERLINE = 1u << 0, KUI_DECO_STRIKETHROUGH = 1u << 1 };
/* An underline's shape (KuiTextStyle.underline_style, KuiSpan.underline_style):
 * the face's line, a wave three strokes tall with a six-stroke period, dots
 * two strokes across and four apart. A non-solid style implies the underline. */
enum { KUI_UNDERLINE_SOLID = 0, KUI_UNDERLINE_WAVY = 1, KUI_UNDERLINE_DOTTED = 2 };
/* Overflow flags */
enum { KUI_CLIP = 1u << 0, KUI_SCROLL_X = 1u << 1, KUI_SCROLL_Y = 1u << 2 };
/* Editing keys (kui_input_key) */
enum {
    KUI_KEY_LEFT = 0, KUI_KEY_RIGHT, KUI_KEY_UP, KUI_KEY_DOWN,
    KUI_KEY_HOME, KUI_KEY_END, KUI_KEY_PAGE_UP, KUI_KEY_PAGE_DOWN,
    KUI_KEY_BACKSPACE, KUI_KEY_DELETE, KUI_KEY_ENTER, KUI_KEY_TAB,
    KUI_KEY_SELECT_ALL, KUI_KEY_ESCAPE, KUI_KEY_UNDO, KUI_KEY_REDO,
};
/* Modifier bits (kui_input_key) */
enum { KUI_MOD_SHIFT = 1u << 0, KUI_MOD_WORD = 1u << 1, KUI_MOD_DOC = 1u << 2 };
/* Physical modifier bits (kui_input_modifiers) */
enum {
    KUI_KMOD_SHIFT = 1u << 0,
    KUI_KMOD_CTRL = 1u << 1,
    KUI_KMOD_ALT = 1u << 2,
    KUI_KMOD_SUPER = 1u << 3,
};
/* Text edit flags (kui_text_edit). AUTOFOCUS asks once: the editor takes
 * focus on the frame the flag starts being declared, and only while
 * nothing holds focus (docs/adr/0022-focus-regions.md, decision 9). WRAP
 * is the wrap row declared on a single-line field: it folds to its width
 * by KuiTextStyle.wrap the way a document does, and keeps a field's
 * keyboard - Enter submits, no newline is admitted, the caret opens at
 * the end. A MULTILINE editor wraps either way. */
enum { KUI_EDIT_MULTILINE = 1u << 0, KUI_EDIT_AUTOFOCUS = 1u << 1, KUI_EDIT_WRAP = 1u << 2 };
/* Float modes (KuiSpec.float_mode). For the named presets the other
 * bindings take ("below", "above", ...), see kui_spec_float_preset. */
enum { KUI_FLOAT_NONE = 0, KUI_FLOAT_PARENT = 1, KUI_FLOAT_VIEWPORT = 2 };
/* Window-chrome roles (KuiSpec.window_role). Chrome nodes turn input into
 * window commands (kui_take_window_command), never events. */
enum {
    KUI_WINDOW_NONE = 0,
    KUI_WINDOW_DRAG = 1,
    KUI_WINDOW_CLOSE = 2,
    KUI_WINDOW_MINIMIZE = 3,
    KUI_WINDOW_MAXIMIZE = 4,
};
/* Easing curves (KuiSpec.easing). */
enum {
    KUI_EASE_OUT = 0,
    KUI_EASE_LINEAR = 1,
    KUI_EASE_IN = 2,
    KUI_EASE_IN_OUT = 3,
    KUI_EASE_SPRING = 4, /* damped spring; transition_ms is the response time */
    KUI_EASE_BOUNCY = 5,
};
/* Window commands (KuiWindowCommand.kind, kui_take_window_command). The
 * first four are what chrome nodes ask for, about the window they were
 * drawn in; KUI_CMD_OPEN and KUI_CMD_CLOSE are also what the declared
 * window set's diff decides (kui_window_declare). The last two are what
 * the app itself asked for (kui_set_window_size / kui_focus_window). */
enum {
    KUI_CMD_START_DRAG = 1,
    KUI_CMD_CLOSE = 2,
    KUI_CMD_MINIMIZE = 3,
    KUI_CMD_TOGGLE_MAXIMIZE = 4,
    KUI_CMD_OPEN = 5,
    KUI_CMD_SET_SIZE = 6,
    KUI_CMD_FOCUS = 7,
    /* Draw cmd.window again: another window's input changed what it shows
     * (the devtools window hovering a node the main window outlines). A host
     * that redraws every window on every event may ignore it. */
    KUI_CMD_REDRAW = 8,
};

/* [in] */
typedef struct KuiSizing {
    uint32_t tag;
    float value;
} KuiSizing;

/* How keyframes cycle (KuiSpec.repeat): CSS animation-direction. */
enum {
    KUI_REPEAT_NORMAL = 0,
    KUI_REPEAT_REVERSE = 1,
    KUI_REPEAT_ALTERNATE = 2,
    KUI_REPEAT_ALTERNATE_REVERSE = 3,
};
/* Which KuiKeyframe fields are set (KuiKeyframe.set bits). */
enum {
    KUI_KF_AT = 1u << 0,
    KUI_KF_WIDTH = 1u << 1,
    KUI_KF_HEIGHT = 1u << 2,
    KUI_KF_BG = 1u << 3,
    KUI_KF_RADIUS = 1u << 4,
    KUI_KF_OPACITY = 1u << 5,
};

/* One CSS-style keyframe stop. A zeroed stop sets nothing: `set` says which
 * fields count, so 0 stays a legal value for each. Stops without KUI_KF_AT
 * spread evenly (a lone stop sits at 1 and animates from the node's own
 * value); declared `at`s must not decrease. Sizings animate their amount
 * only, in the form the spec's own width/height declares. */
/* [in] */
typedef struct KuiKeyframe {
    uint32_t set;
    float at; /* 0..1 */
    KuiSizing width, height;
    uint32_t bg; /* 0xRRGGBBAA */
    float radius;
    float opacity; /* group opacity 0..1 */
} KuiKeyframe;

/* Which KuiEnter fields are set (KuiEnter.set bits); 0 = no entrance. */
enum {
    KUI_ENTER_OFFSET = 1u << 0,
    KUI_ENTER_WIDTH = 1u << 1,
    KUI_ENTER_HEIGHT = 1u << 2,
    KUI_ENTER_BG = 1u << 3,
    KUI_ENTER_RADIUS = 1u << 4,
    KUI_ENTER_OPACITY = 1u << 5,
};

/* Where a node starts the first frame it is seen (KuiSpec.enter): the slots
 * `set` names ease in from these values over transition_ms instead of
 * snapping — dx/dy slide it in from that far away (logical px), bg fades
 * the node and opacity the whole subtree in. A node drawn again after a
 * frame away enters again. Zeroed = none. */
/* [in] */
typedef struct KuiEnter {
    uint32_t set;
    float dx, dy;
    KuiSizing width, height;
    uint32_t bg; /* 0xRRGGBBAA */
    float radius;
    float opacity; /* group opacity 0..1; 0 fades the subtree in */
} KuiEnter;

/* [in] Zero-initialized KuiSpec is a fit-sized transparent column. Colors
 * are 0xRRGGBBAA with 0 meaning "none". Fields mirror the shared prop schema
 * (crates/kui-core/src/schema.rs) and are append-only, which is safe here
 * precisely because the library only reads this one: see "Who writes what".
 * `tooltip` was appended that way, and did not bump KUI_ABI_VERSION. */
typedef struct KuiSpec {
    KuiSizing width, height;
    float min_w, max_w, min_h, max_h; /* clamps; max 0 = unconstrained,
                                         min KUI_MIN_FIT = the fit size */
    uint32_t dir; /* KUI_COLUMN / KUI_ROW / KUI_TABLE */
    float pad_l, pad_r, pad_t, pad_b;
    float gap;
    uint32_t main_align, cross_align;
    uint32_t bg;
    uint32_t border_color;
    float border_w;
    float radius;
    uint32_t overflow; /* KUI_CLIP | KUI_SCROLL_X | KUI_SCROLL_Y */
    /* Out-of-flow positioning: 0 = in flow, KUI_FLOAT_PARENT/VIEWPORT anchors.
     * Attach points use KUI_START/CENTER/END; dx/dy is a logical-px offset.
     * kui_spec_float_preset fills all of these from a preset name. */
    uint32_t float_mode;
    uint32_t float_anchor_x, float_anchor_y;
    uint32_t float_self_x, float_self_y;
    float float_dx, float_dy;
    uint32_t float_fit; /* non-zero: flip across the anchor / clamp to stay in the viewport */
    uint32_t hoverable; /* non-zero: hover-track without a click payload (kui_is_hovered) */
    uint32_t window_role; /* KUI_WINDOW_*; makes this node window chrome */
    /* > 0: ease sizing/color/radius changes over this many ms (needs a stable
     * key via kui_open_keyed, and kui_set_time each frame). */
    float transition_ms;
    uint32_t easing; /* KUI_EASE_* */
    uint32_t slide;  /* non-zero: also ease the position (siblings slide) */
    /* Declarative pointer styling, resolved by the core when the node opens
     * (no kui_is_hovered round trip; eases with transition_ms). 0 = none.
     * Any of these makes the node hover-tracked. */
    uint32_t hover_bg;   /* 0xRRGGBBAA while hovered (or its hover group is) */
    uint32_t pressed_bg; /* 0xRRGGBBAA while pressed */
    /* Hover group name (empty = none): members show hover_bg / pressed_bg
     * together (a split button, a multi-piece shape). Hashed, not retained. */
    KuiStr hover_group;
    /* Per-corner radii: with per_corner non-zero, radius_tl..radius_bl are
     * the four corner radii (clockwise from the top-left) and `radius` is
     * ignored; zero keeps the uniform `radius` on every corner. */
    uint32_t per_corner;
    float radius_tl, radius_tr, radius_br, radius_bl;
    /* CSS-style keyframes: the slots the stops name (width, height, bg,
     * radius) cycle through them over transition_ms — forever, without the
     * view redrawing, sampled off kui_set_time's clock — in the `repeat`
     * direction (KUI_REPEAT_*), held back by delay_ms so siblings given
     * different delays run out of phase. Slots no stop names still ease
     * toward what the spec declares. The array is read while the node
     * opens and not retained; NULL / 0 = none. */
    uint32_t repeat;
    float delay_ms;
    const KuiKeyframe *keyframes;
    size_t keyframes_len;
    /* Entrance transition (see KuiEnter); `set` = 0 leaves first sight
     * snapping as before. Needs transition_ms (defaults to 200 if 0) and a
     * stable key; with `slide` the position keeps easing afterwards, without
     * it only the entrance moves. */
    KuiEnter enter;
    /* Registered sounds (kui_sound_add) played when the node is clicked /
     * the pointer enters it; 0 = none. Either makes the node hover-tracked. */
    uint64_t click_sound;
    uint64_t hover_sound;
    /* Layout tag (NULL = none): the rect layout gave the node arrives as
     * {kind="layout", x, y, w, h, parent={x,y,w,h}, tag} (logical px,
     * viewport coords) on its first frame and whenever it changes — never
     * on a frame that left it alone. Borrowed: cloned while the node opens,
     * so you keep ownership; kui_value_null() asks for untagged events.
     * Needs a stable key (kui_open_keyed). */
    const KuiValue *on_layout;
    /* Accessibility (docs/adr/0001-accessibility-as-data.md). role: KUI_ROLE_*
     * (0 = unset: the core derives one — an on_click node is a button, an
     * editor a text input, a scrolling box a scroll view, a plain box
     * nothing; KUI_ROLE_NONE hides the node and its subtree). label: the
     * accessible name (empty = none; a button, link, tab or heading is then
     * named by the text inside it, an image or icon button has no name and
     * the core warns). checked: a checkbox / radio / switch role's on
     * state (selected / expanded are below). value_*: a slider role's position and range, each present
     * when its KUI_VALUE_* bit is in value_set. */
    uint32_t role;
    KuiStr label;
    uint32_t checked;
    uint32_t value_set;
    float value_now, value_min, value_max;
    /* On a KUI_ROLE_LINE of a custom editor (an on_key sink with a
     * KUI_ROLE_TEXT_INPUT / MULTILINE_TEXT_INPUT role that draws its own
     * text): the caret's byte offset into the line's text, and the byte
     * offset of the selection's other end; present when KUI_VALUE_CARET /
     * KUI_VALUE_ANCHOR are in value_set. KUI_VALUE_CARET_SOLID beside
     * KUI_VALUE_CARET says the caret is solid — a block caret in a modal
     * editor's normal mode — so kui_has_caret leaves it out and no blink
     * clock is armed on it, while it still anchors the IME and reads to
     * assistive technology. */
    uint32_t caret;
    uint32_t selection_anchor;
    /* Keyboard focus (docs/adr/0002-keyboard-focus-as-data.md). focusable:
     * non-zero puts the node in the Tab ring (and a click focuses it)
     * without a click payload or a control role; editors, on_key sinks,
     * on_click nodes and the control roles are focusable already.
     * disabled: non-zero makes the node inert — no click, drag or key sink,
     * no hover / pressed / focus background, skipped by Tab, reported to
     * assistive technology (hover tracking stays so a tooltip can say why).
     * focus_bg: 0xRRGGBBAA while the node holds keyboard-visible focus (Tab
     * or assistive technology put it there); 0 = the core's default ring. */
    uint32_t focusable;
    uint32_t disabled;
    uint32_t focus_bg;
    /* Hover hint (empty = none), the `tooltip` prop of the other bindings:
     * makes the node hover-tracked, becomes its accessible description,
     * and floats the hint below it while the pointer is over it. kui_close
     * draws that float, so this only applies to nodes opened with the
     * kui_open* family; for a hint that always draws, or one around custom
     * content, call kui_tooltip / kui_tooltip_with yourself. Borrowed
     * while the node opens. */
    KuiStr tooltip;
    /* Modal surface, NULL = none (docs/adr/0003-modal-surfaces.md): while
     * this node is declared the Tab ring is its subtree, everything
     * outside it is inert to the pointer, the wheel and assistive
     * technology (window chrome stays live), and Escape or a press
     * outside emits {kind="dismiss", reason="escape"|"outside", tag} on
     * it — the core closes nothing, the app stops opening the node. The
     * last one declared in tree order is the one in effect (a confirm
     * inside a dialog); a modal that must cover the app is a float.
     * Borrowed: cloned while the node opens, so you keep ownership;
     * kui_value_null() asks for the behaviour without a tag. */
    const KuiValue *modal;
    /* Context menu, NULL = none: a secondary-button press over this node
     * emits {kind="contextmenu", x, y, tag} on it, at the logical point to
     * open the menu at. The press does nothing else — it moves no focus,
     * places no caret and produces no click, so right-clicking a selection
     * keeps it. Routed like a click: the topmost node under the pointer is
     * the one asked. Borrowed: cloned while the node opens, so you keep
     * ownership; kui_value_null() asks for the behaviour without a tag. */
    const KuiValue *on_context_menu;
    /* KUI_CURSOR_* (0 = unset). The pointer shape while the pointer is
     * over this node. Unset, the pointer is the I-beam over an editor or
     * a selection scope and the arrow over everything else — an on_click,
     * focusable or on_drag node included, as a native button is — so a
     * hand (KUI_CURSOR_POINTER) over a control, a KUI_CURSOR_GRAB over a
     * handle (KUI_CURSOR_GRABBING while its drag runs, declared by the
     * view as its drag state changes), a splitter's KUI_CURSOR_EW_RESIZE
     * and a disabled control's KUI_CURSOR_NOT_ALLOWED are all declared.
     * kui_button / kui_button_with declare the hand themselves. A node
     * with nothing but a cursor is hover-tracked so it can be found. */
    uint32_t cursor;
    /* Selection and disclosure. selected: non-zero when this node is the
     * current one of its set - the shown tab, the picked row, the link for
     * the page you are on. A KUI_ROLE_TAB reports the state either way (its
     * siblings read as "not selected"); a row or a link reports it only
     * where this is set, since an ordinary list or navigation bar is not a
     * selection. expanded: KUI_EXPANDED_* for a node that shows and hides
     * something (0 = unset: it does not expand, and a reader says nothing
     * about it). "3 of 7" is not declared - the core numbers a
     * KUI_ROLE_LIST's rows and a KUI_ROLE_TAB_LIST's tabs itself, and
     * reports them in KuiAccessNode.pos_in_set / set_size. */
    uint32_t selected;
    uint32_t expanded;
    /* Group opacity: with opacity_set non-zero, `opacity` (0..1) fades this
     * node and its whole subtree. The bit exists so 0 stays expressible —
     * without it, a zeroed struct could not tell "opaque" from "invisible".
     * It is a per-quad alpha multiply, not an offscreen composite, so
     * overlapping pieces of one subtree show their seams through the fade.
     * Layout, hit-testing and the access tree are untouched: an invisible
     * subtree still takes clicks, exactly like CSS opacity: 0. Eases with
     * transition_ms, and enter.opacity fades a panel in. */
    uint32_t opacity_set;
    float opacity;
    /* Drop shadow: the node's rounded rect, moved by shadow_x/shadow_y,
     * grown by shadow_spread and blurred over shadow_blur, painted in
     * shadow_color behind the node (CSS box-shadow without the inset and
     * multi-shadow forms). 0xRRGGBBAA with 0 = no shadow: nothing else here
     * draws without a color, and a color on its own is a hard shadow
     * exactly behind the node. Outer shadows only, and the shape is not
     * knocked out of the middle, so a translucent bg shows it through.
     * All four numbers are logical px and ease with transition_ms. */
    uint32_t shadow_color;
    float shadow_blur;
    float shadow_x, shadow_y;
    float shadow_spread;
    /* Non-zero: children that do not fit the main-axis content box start a
     * new line instead of overflowing it (or shrinking to fit) — a tag
     * list, a chip toolbar, a button row that reflows when the window
     * narrows. cross_gap is the space between the lines; gap stays the
     * space between children along one. Rows only: a column, or a row with
     * KUI_OVERFLOW_SCROLL_X, lays out as if this were 0 and raises a
     * "wrap-ignored" warning (kui_take_warnings). */
    uint32_t wrap_children;
    float cross_gap;
    /* Non-zero: where focus lands when the modal scope containing this
     * node is entered - the first node in the modal's Tab ring declaring
     * it, instead of the ring's first, so a destructive confirm opens on
     * its Cancel rather than on whichever control comes first
     * (docs/adr/0003-modal-surfaces.md). Read on entry only: a Tab press
     * afterwards stands, and the scope re-entered (a nested confirm
     * closing) leaves focus where it was. A node the ring skips
     * (disabled, KUI_ROLE_NONE, not focusable) is not a candidate, and
     * with no candidate the entry is the ring's first node as before. */
    uint32_t initial_focus;
    /* Exit transition (see KuiEnter, which an exit reuses — an exit is an
     * entrance read the other way). With `set` non-zero and a transition_ms,
     * the frame after the view stops declaring this node its subtree is
     * copied out of the last frame that had it and replayed: frozen where
     * layout left it, painted on top of everything and outside every clip,
     * and inert — no clicks, no Tab stop, no access row — while the slots
     * `set` names ease from where they were to these values. Then it is
     * dropped; so is a ghost whose key the view declares again, so a toast
     * dismissed and re-shown never doubles. `set` = 0, or no transition_ms,
     * leaves a removed node vanishing at once as before. Needs a stable key.
     * width/height resize the departing node's own box only: what is inside
     * it is a picture and is not laid out again. The store holds 512 nodes
     * and a frame's removal is judged whole: one that does not fit takes
     * the room from the oldest exits still in flight, and one larger than
     * the budget on its own animates nothing (every node of it vanishes at
     * once) with an "exit-budget" warning for the frame. */
    KuiEnter exit;
    /* KUI_LIVE_*: when the text inside this node changes, a screen reader
     * reads the change without being asked. A node that declares it is
     * semantic, so a plain box marked live is not elided from the access
     * tree; put it on the smallest node holding the message, since
     * everything inside a live node is live. For a one-off with no node
     * behind it, kui_announce is the other half. See
     * docs/adr/0008-live-regions-and-announcements.md. */
    uint32_t live;
    /* Non-zero, with a non-NULL on_key on kui_open_with: the sink hears
     * releases too, as the same {kind="key"} payload with phase="up" (a
     * null `text`, `repeat` false) - for a held-key interaction: WASD,
     * press-and-hold, a key that arms a mode while it is down. A key only
     * comes up where it went down (a release whose press the sink never got
     * resolves nothing), and focus leaving while a key is held delivers the
     * "up" first, so nothing is left stuck down. Zero: presses only, which
     * is what a keymap wants - one that heard both halves would run every
     * binding twice. */
    uint32_t key_up;
    /* What a slider role's position reads as, empty for none (ARIA's
     * aria-valuetext). Without one a reader has only value_now and the
     * range and says a percentage - 25 in [5..60] is "36 percent" - so a
     * value whose unit carries the meaning says it here: "25 minutes". It
     * replaces the number in the reading rather than joining it, and a
     * nudge announces the new text; putting the reading in `label` instead
     * renames the control on every nudge. Copied while the node opens, and
     * read back as KuiAccessNode.value (KUI_ACCESS_HAS_VALUE). */
    KuiStr value_text;
    /* The accessible description (empty = none): the extra sentence a
     * reader says after the name, for what the name cannot say on its own -
     * what a button will do, why a control is disabled, what format a field
     * wants. `tooltip` above is the shorthand that also draws the string and
     * hover-tracks the node; this is the description alone, for a hint that
     * is spoken and never drawn. Both write the one slot and this one is
     * applied second, so it wins over a `tooltip` on the same node. It reads
     * only on a node that reaches the access tree - a role, a label, a
     * control - since a plain box is elided and takes its description with
     * it. Borrowed while the node opens. */
    KuiStr description;
    /* Non-zero: ask for another frame after this one, every frame this node
     * is declared. What a kui_fragment reading `time` needs; opt-in,
     * because it takes the loop off input-driven. One node is enough. */
    uint32_t animate;
    /* Non-zero: paint this node's background in the OS accent colour the
     * host pushed through kui_env_set_system, keeping `bg` where it never
     * said what the accent is. On kui_button_with it takes the hover and
     * pressed shades and the label colour with it, so <button accent> is
     * one field rather than a palette. Appended after ABI 9 without a bump,
     * under the [in] rule as it then stood (see the ABI block). */
    uint32_t accent;
    /* Non-zero: this node is a selection scope. The text of every node
     * inside it is one selectable run, in tree order, and a press-drag
     * across them selects the lot - three labels in a column under one
     * `selectable` select as three lines of one text
     * (docs/adr/0017-selection-as-a-scope.md). Declared on the container
     * and not on each label. The selection is the window's: starting one
     * anywhere clears the last, an editor's included, and kui_copy_text
     * reads whichever exists. Text scrolled out of view inside the scope
     * is still part of it - selection and copy reach it, hit-testing does
     * not. Appended after ABI 11 without a bump, under the [in] rule as
     * it then stood (see the ABI block); zero is not a selection scope,
     * which is what every node was before this. */
    uint32_t selectable;
    /* Force-click tag: a press that deepens past the second stage of a
     * Force Touch trackpad over this node emits {kind:"forceclick", x, y,
     * tag} on it, at the point it happened
     * (docs/adr/0017-selection-as-a-scope.md). Routed like
     * on_context_menu - topmost node, no focus moved, no caret placed, no
     * click - and the ordinary click that press produces still arrives
     * afterwards. Text needs none of this: a force click over an editor or
     * a `selectable` scope selects the word and asks the host for its
     * definition panel. macOS-only in practice, and switchable off there,
     * so nothing may be reachable only this way. Borrowed while the node
     * opens; appended after ABI 11 without a bump, under the [in] rule as
     * it then stood (see the ABI block). */
    const KuiValue *on_force_click;
    /* Non-zero: this node's subtree is a focus region - a Tab ring of its
     * own that the ring outside never enters and that never leaves: a
     * devtools dock, an inspector beside the app
     * (docs/adr/0022-focus-regions.md). Entered on purpose:
     * kui_focus_region moves focus in (to what the region last held, else
     * its initial_focus, else its first stop) and a press inside it, or a
     * focus on a node in it, enters it too; Tab then walks that ring alone
     * and wraps inside it. A region that stops being declared hands focus
     * back to what the main ring last held. Only the ring is scoped: keys
     * bubble through the boundary to the sink above, the pointer and
     * assistive technology see a plain node, and a `modal` in effect is the
     * ring wherever it sits. Appended after ABI 13 without a bump, under
     * the [in] rule as it then stood (see the ABI block). */
    uint32_t focus_region;
    /* When this node's scrollbars are drawn: KUI_SCROLLBAR_* (the
     * `scrollbar` row's index plus one), 0 for the default, which is
     * KUI_SCROLLBAR_VISIBLE. The three after it style the thumb: its
     * width at rest in logical px (0 = the stock 4; under the pointer or
     * dragged it is 2 px wider), its colour at rest and under the pointer
     * as 0xRRGGBBAA (0 = the theme's scrollbar / scrollbar_active roles).
     * Appended after ABI 13 without a bump, under the [in] rule as it then
     * stood (see the ABI block). */
    uint32_t scrollbar;
    float scrollbar_width;
    uint32_t scrollbar_color;
    uint32_t scrollbar_active_color;
    /* Non-zero: scroll anchoring on a scrolling node (CSS's overflow-anchor):
     * the first child in view keeps its place on screen when the content
     * before it changes size - a chat that prepends history, a log that
     * inserts above the viewport. The child is found by key, so give the
     * rows stable keys; on the scroll axis that is the node's main axis
     * only. Appended after ABI 14 without a bump, under the [in] rule as
     * it then stood (see the ABI block). */
    uint32_t anchor;
    /* Scroll tag: the wheel over this node emits {kind:"scroll", x, y, dx,
     * dy, lines, tag} on it instead of scrolling anything - dx/dy the
     * delta in logical px as the driver reported it (positive dy is the
     * wheel rolling up), x/y the pointer, lines the whole lines a `cells`
     * grid's delta covers (positive = later history, the sign origin_line
     * grows in; the fraction is carried to the next notch) and null on
     * any other node. The node takes the wheel: it reaches no scroller
     * above it, and a scroller inside it still wins. The core moves
     * nothing - a grid re-declares origin_line, a canvas zooms. A
     * drag-select held past a grid's top or bottom edge arrives here too,
     * once a frame with the lines that frame scrolled by
     * (docs/adr/0029-a-selection-follows-the-pointer-past-the-edge.md).
     * Borrowed while the node opens; appended after ABI 15 without a
     * bump, under the [in] rule as it then stood (see the ABI block). */
    const KuiValue *on_scroll;
    /* Drop-zone tag (docs/adr/0031-a-drop-zone-is-a-row-and-the-files-are-
     * an-event.md): files dragged in from the OS over this node emit
     * {kind:"drop", phase:"enter"|"move"|"leave"|"drop", paths, x, y, tag}
     * on it - paths the OS paths as strings, x/y the pointer in logical
     * viewport coordinates (absent on leave). The zone under the files is
     * the topmost zone by paint order: a node inside a zone is the zone's,
     * and a node that is no zone and has none enclosing it is looked past,
     * so an overlay shown on enter cannot make the zone lose the files. No
     * leave follows a drop. Borrowed while the node opens. ABI 18. */
    const KuiValue *on_drop;
    /* Background while dragged files are over this node, 0xRRGGBBAA (0 =
     * none): wins over pressed_bg, focus_bg and hover_bg, clears when they
     * leave, land or the drag is cancelled; eases with transition. ABI 18. */
    uint32_t drop_bg;
} KuiSpec;

/* When a scrolling node's bars are drawn (KuiSpec.scrollbar): the schema
 * index plus one, so zero is the default. VISIBLE draws the stock overlay
 * thumb whenever the content overflows; HIDDEN draws none and takes no
 * press on the track (the wheel, the keyboard, kui_reveal and the caret
 * still scroll); AUTO shows the bar while the scroll state is changing -
 * the offset or the extent moved, the pointer is on the track, a thumb is
 * dragged - and for a second after, then fades it out over a quarter of
 * one, asking for frames from the last change until it has faded (and
 * none while the pointer holds it). AUTO needs the driver's clock
 * (kui_set_time) and is VISIBLE without one. */
enum {
    KUI_SCROLLBAR_VISIBLE = 1,
    KUI_SCROLLBAR_HIDDEN = 2,
    KUI_SCROLLBAR_AUTO = 3,
};

/* Disclosure state (KuiSpec.expanded): the schema index plus one, so zero
 * can mean "this node does not expand". */
enum {
    KUI_EXPANDED_COLLAPSED = 1,
    KUI_EXPANDED_EXPANDED = 2,
};

/* Live-region politeness (KuiSpec.live, KuiAnnouncement.live): the schema
 * index itself, not the index plus one — unlike a disclosure, "not a live
 * region" is what a zeroed field already means, so there is no unset state
 * to reserve zero for. */
enum {
    KUI_LIVE_OFF = 0,
    KUI_LIVE_POLITE = 1,
    KUI_LIVE_ASSERTIVE = 2,
};

/* Pointer shapes (KuiSpec.cursor), and what kui_cursor_shape answers with. */
enum {
    KUI_CURSOR_DEFAULT = 1, KUI_CURSOR_TEXT, KUI_CURSOR_POINTER,
    KUI_CURSOR_GRAB, KUI_CURSOR_GRABBING, KUI_CURSOR_NOT_ALLOWED,
    KUI_CURSOR_EW_RESIZE, KUI_CURSOR_NS_RESIZE, KUI_CURSOR_NWSE_RESIZE,
    KUI_CURSOR_NESW_RESIZE,
};

/* Roles (KuiSpec.role, KuiAccessNode.role). Declarable on any node:
 * KUI_ROLE_NONE through KUI_ROLE_GROUP, plus KUI_ROLE_TEXT_INPUT,
 * KUI_ROLE_MULTILINE_TEXT_INPUT and KUI_ROLE_LINE (which an app that
 * draws its own text declares to make a key sink an editor and to mark
 * that editor's lines) and the three ADR 0007 appended,
 * KUI_ROLE_RADIO_GROUP through KUI_ROLE_MENU_ITEM. A role can only be
 * appended (KUI_ROLE_* is the position in Rust's Role::ALL plus one, and
 * the Lua and Node wires carry the same index), so what is declarable is
 * a list rather than a range. The rest the core derives from what a node
 * is.
 *
 * KUI_ROLE_RADIO_GROUP, KUI_ROLE_TAB_LIST, KUI_ROLE_MENU and KUI_ROLE_LIST
 * are the composite containers: one holding focusable KUI_ROLE_RADIO,
 * KUI_ROLE_TAB, KUI_ROLE_MENU_ITEM or KUI_ROLE_LIST_ITEM children is a
 * single Tab stop with the arrow keys moving inside it
 * (docs/adr/0007-composite-keyboard-patterns.md). Nothing declares that:
 * the core derives it from the roles and from which nodes are focusable. */
enum {
    KUI_ROLE_NONE = 1, KUI_ROLE_BUTTON, KUI_ROLE_CHECKBOX, KUI_ROLE_RADIO,
    KUI_ROLE_SWITCH, KUI_ROLE_SLIDER, KUI_ROLE_TAB, KUI_ROLE_TAB_LIST,
    KUI_ROLE_LINK, KUI_ROLE_HEADING, KUI_ROLE_LIST, KUI_ROLE_LIST_ITEM,
    KUI_ROLE_IMAGE, KUI_ROLE_DIALOG, KUI_ROLE_GROUP,
    KUI_ROLE_WINDOW, KUI_ROLE_TITLE_BAR, KUI_ROLE_STATIC_TEXT,
    KUI_ROLE_TEXT_INPUT, KUI_ROLE_MULTILINE_TEXT_INPUT, KUI_ROLE_SCROLL_VIEW,
    KUI_ROLE_LINE,
    KUI_ROLE_RADIO_GROUP, KUI_ROLE_MENU, KUI_ROLE_MENU_ITEM,
    KUI_ROLE_TERMINAL, /* derived from kui_cells; its rows are the value */
};

/* How a composite container arranges its items
 * (KuiAccessNode.orientation, 0 = unset). Derived from the container's
 * own KuiSpec.dir and never declared: the layout is what arranges the
 * items. An announcement, not a gate — both arrow pairs move inside a
 * composite whatever this says. */
enum {
    KUI_ORIENTATION_HORIZONTAL = 1,
    KUI_ORIENTATION_VERTICAL = 2,
};
/* Which of KuiSpec.value_now / value_min / value_max / caret /
 * selection_anchor are set, and whether the caret is solid (no caret to
 * blink; see KuiSpec.caret). */
enum {
    KUI_VALUE_NOW = 1u << 0,
    KUI_VALUE_MIN = 1u << 1,
    KUI_VALUE_MAX = 1u << 2,
    KUI_VALUE_CARET = 1u << 3,
    KUI_VALUE_ANCHOR = 1u << 4,
    KUI_VALUE_CARET_SOLID = 1u << 5,
};
/* Actions assistive technology can request (KuiAccessNode.actions bits,
 * kui_input_access). */
enum {
    KUI_ACCESS_CLICK = 1u << 0,
    KUI_ACCESS_FOCUS = 1u << 1,
    KUI_ACCESS_BLUR = 1u << 2,
    KUI_ACCESS_SET_VALUE = 1u << 3,
    KUI_ACCESS_INCREMENT = 1u << 4,
    KUI_ACCESS_DECREMENT = 1u << 5,
    KUI_ACCESS_SCROLL_INTO_VIEW = 1u << 6,
    KUI_ACCESS_SCROLL_UP = 1u << 7,
    KUI_ACCESS_SCROLL_DOWN = 1u << 8,
    KUI_ACCESS_SCROLL_LEFT = 1u << 9,
    KUI_ACCESS_SCROLL_RIGHT = 1u << 10,
    KUI_ACCESS_SET_TEXT_SELECTION = 1u << 11,
    KUI_ACCESS_REPLACE_SELECTED_TEXT = 1u << 12,
};
/* KuiAccessNode.flags: which optional fields hold, and state. */
enum {
    KUI_ACCESS_HAS_VALUE = 1u << 0,
    KUI_ACCESS_HAS_SELECTION = 1u << 1,
    KUI_ACCESS_FOCUSED = 1u << 2,
    KUI_ACCESS_CHECKED_SET = 1u << 3,
    KUI_ACCESS_CHECKED = 1u << 4,
    KUI_ACCESS_HAS_NUMBER = 1u << 5,
    KUI_ACCESS_HAS_MIN = 1u << 6,
    KUI_ACCESS_HAS_MAX = 1u << 7,
    KUI_ACCESS_HAS_SCROLL = 1u << 8,
    KUI_ACCESS_HAS_TEXT_SELECTION = 1u << 9,
    KUI_ACCESS_DISABLED = 1u << 10, /* declared disabled: inert, not a Tab stop */
    KUI_ACCESS_MODAL = 1u << 11,    /* the frame's modal surface (aria-modal) */
    /* Whether the node has a selected state at all, and what it is. */
    KUI_ACCESS_SELECTED_SET = 1u << 12,
    KUI_ACCESS_SELECTED = 1u << 13,
    /* Whether the node expands, and whether it is open. */
    KUI_ACCESS_EXPANDED_SET = 1u << 14,
    KUI_ACCESS_EXPANDED = 1u << 15,
    /* pos_in_set holds (on an item), set_size holds (on its container). */
    KUI_ACCESS_HAS_POS_IN_SET = 1u << 16,
    KUI_ACCESS_HAS_SET_SIZE = 1u << 17,
    /* The node declared KuiSpec.live, and which politeness. Two flag bits
     * rather than a `live` field: KuiAccessNode is [out[]], so the host
     * allocates the array and appending to it would be an ABI break. */
    KUI_ACCESS_LIVE_POLITE = 1u << 18,
    KUI_ACCESS_LIVE_ASSERTIVE = 1u << 19,
};

/* [out[]] One queued announcement (kui_take_announcements): something to say
 * once, with no node behind it. `live` is KUI_LIVE_POLITE or
 * KUI_LIVE_ASSERTIVE, never KUI_LIVE_OFF. Strings are borrowed until the
 * next kui_take_announcements on the context.
 * See docs/adr/0008-live-regions-and-announcements.md. */
typedef struct KuiAnnouncement {
    KuiStr text;
    uint32_t live;
} KuiAnnouncement;

/* [out[]] One node of the access tree (kui_access_tree): what assistive technology
 * sees. Plain boxes are elided, so `parent` is the nearest semantic
 * ancestor (0 for the root). Rects are logical px in viewport coordinates.
 * Strings are borrowed until the next kui_access_tree on the context. */
typedef struct KuiAccessNode {
    uint64_t key;
    uint64_t parent;
    uint32_t origin;
    uint32_t role;    /* KUI_ROLE_* */
    uint32_t flags;   /* KUI_ACCESS_HAS_* / FOCUSED / CHECKED / SELECTED / EXPANDED */
    uint32_t actions; /* KUI_ACCESS_* the node accepts */
    KuiStr name;
    KuiStr description;
    /* The node's one string value (KUI_ACCESS_HAS_VALUE): an editor's text,
     * or a slider's value_text - a slider that named its reading reads as
     * that string instead of its number. */
    KuiStr value;
    float x, y, w, h;
    uint32_t caret, selection_start, selection_end; /* byte offsets into value */
    float value_now, value_min, value_max;          /* a slider's position and range */
    float scroll_x, scroll_y, scroll_max_x, scroll_max_y; /* a scroll view's offsets */
    /* An editor's caret (focus_*) and the selection's other end (anchor_*)
     * as run positions (KUI_ACCESS_HAS_TEXT_SELECTION): a run key from
     * kui_access_runs and a character index into that run. */
    uint64_t anchor_run;
    uint32_t anchor_char;
    uint64_t focus_run;
    uint32_t focus_char;
    uint32_t run_count; /* how many runs kui_access_runs returns */
    /* "3 of 7", derived from the list or tab list holding this node: the
     * item's zero-based ordinal (KUI_ACCESS_HAS_POS_IN_SET) and, on that
     * container, how many items it holds (KUI_ACCESS_HAS_SET_SIZE). The
     * count sits on the container, not on each item, which is how
     * AccessKit models a set (ARIA repeats aria-setsize on every item). */
    uint32_t pos_in_set;
    uint32_t set_size;
    /* How a composite container arranges its items (KUI_ORIENTATION_*,
     * 0 = unset: this node is not one). Derived from its own dir. */
    uint32_t orientation;
} KuiAccessNode;

/* [out[]] One laid-out run of an editor's text (kui_access_runs): what a screen
 * reader reads by character and word. `text` ends with "\n" (a character
 * of zero width) when the line continues into another; `line` is a
 * buffer line (a KUI_ROLE_LINE ordinal for a custom editor) and
 * start/end the run's byte range in that line's text. Character positions
 * are relative to x. Arrays and strings are borrowed until the next
 * kui_access_tree / kui_access_runs on the context. */
typedef struct KuiAccessRun {
    uint64_t key;
    uint32_t line, start, end;
    KuiStr text;
    float x, y, w, h;
    uint32_t char_count;
    const uint8_t *char_lengths;
    const float *char_positions;
    const float *char_widths;
    uint32_t word_start_count;
    const uint8_t *word_starts;
    uint32_t rtl;
} KuiAccessRun;

/* [out] What text measures (kui_measure_text): logical px at the scale of
 * the current or last frame; `lines` after wrapping. */
typedef struct KuiTextMetrics {
    uint32_t size; /* = sizeof(KuiTextMetrics) in, bytes filled out */
    float width, height;
    uint32_t lines;
} KuiTextMetrics;
#define KUI_TEXT_METRICS_INIT ((KuiTextMetrics){ .size = sizeof(KuiTextMetrics) })

/* [in] One cell of a kui_cells grid: a Unicode scalar, colours as
 * 0xRRGGBBAA (bg 0 = none), KUI_CELL_* bits. Travels as an array, so a
 * change here is an ABI bump. */
typedef struct KuiCell {
    uint32_t ch, fg, bg, flags;
    uint32_t ul; /* the underline's own colour (SGR 58), 0 = fg. ABI 17: cells travel as
                    an array, so this append moved the stride. */
} KuiCell;
enum {
    KUI_CELL_BOLD = 1u << 0,
    KUI_CELL_ITALIC = 1u << 1,
    KUI_CELL_UNDERLINE = 1u << 2,
    KUI_CELL_STRIKETHROUGH = 1u << 3,
    KUI_CELL_WIDE = 1u << 4, /* the glyph spans this cell and the next, left blank */
    KUI_CELL_WAVY = 1u << 5, /* the underline is a wave (SGR 4:3); implies it */
    KUI_CELL_DOTTED = 1u << 6, /* the underline is dotted (SGR 4:4); implies it */
};
/* kui_cells cursor_shape: 0 = none. */
enum { KUI_CELL_CURSOR_BLOCK = 1, KUI_CELL_CURSOR_BAR = 2, KUI_CELL_CURSOR_UNDERLINE = 3 };

/* -- Context menus (docs/adr/0017-selection-as-a-scope.md) ----------------
 *
 * A menu is a list of items and a point, not a node: the core holds the one
 * a window has open and draws it, so a host asks for one with a call the
 * way it asks for focus, and hears what was chosen as an ordinary event.
 * A right-click the host does not claim with `on_context_menu` opens the
 * stock menu by itself where there is anything standard to offer - an
 * editor, or a `selectable` scope - so this is for the menus a host wants
 * of its own. */

/* KuiMenuItem.role. A standard role means the core does what it can with
 * the row: KUI_MENU_SELECT_ALL it performs, the clipboard three it turns
 * into a KuiMenuAction, and a KUI_MENU_LOOK_UP is the host's panel to show.
 * KUI_MENU_CUSTOM is an item only the host can carry out. */
enum {
    KUI_MENU_CUSTOM = 0,
    KUI_MENU_SEPARATOR = 1,
    KUI_MENU_CUT = 2,
    KUI_MENU_COPY = 3,
    KUI_MENU_PASTE = 4,
    KUI_MENU_SELECT_ALL = 5,
    KUI_MENU_LOOK_UP = 6,
};

/* [in] One row of a menu. `label` may be empty for a standard role, which
 * then reads the way this library words it. `id` is what the row posts when
 * chosen (NULL posts its text); `accel` is drawn right-aligned and bound to
 * nothing - the shortcut is the host's, and this only says which one.
 * `enabled` zero draws the row dimmed and inert, which is what a menu does
 * with an impossible item: the row stays where it is rather than vanishing
 * and moving every row under it. */
typedef struct KuiMenuItem {
    KuiStr label;
    uint32_t role;
    uint32_t enabled;
    const KuiValue *id;
    KuiStr accel;
    /* Non-zero draws a checkmark beside the row, and sets the platform's
     * own check state where the host renders the menu: a setting the row
     * *is* (View > Show Sidebar), not a command it runs. */
    uint32_t checked;
} KuiMenuItem;

/* -- The application menu bar ---------------------------------------------
 *
 * docs/adr/0018-a-menu-bar-the-app-declares.md. The bar is the same rows
 * one level up: kui_menu_bar_declare takes a list of menus, each a label
 * and the KuiMenuItems above, and the declaration is sticky and diffed the
 * way kui_window_title is. Where the platform owns a menu bar the host
 * hands it over; everywhere else kui_menu_bar draws it into the frame.
 * Either way a chosen row posts the same {kind:"menu", role, item} event
 * the context menu posts. */

/* [in] One menu of the bar. `items` is `count` rows; `enabled` zero dims
 * the whole menu and opens nothing. On macOS the first menu is the
 * application menu, which the OS titles with the app's own name whatever
 * `label` says. */
typedef struct KuiMenu {
    KuiStr label;
    const KuiMenuItem *items;
    size_t count;
    uint32_t enabled;
} KuiMenu;

/* KuiMenu item flags, as kui_menu_bar_item reports them. */
enum {
    KUI_MENU_ITEM_ENABLED = 1u << 0,
    KUI_MENU_ITEM_CHECKED = 1u << 1,
};

/* KuiMenuAction.kind. LOOK_UP carries the text to show a definition panel
 * for, and only ever reaches a host that said it can show one. */
enum {
    KUI_MENU_ACTION_SET_CLIPBOARD = 0,
    KUI_MENU_ACTION_PASTE = 1,
    KUI_MENU_ACTION_LOOK_UP = 2,
};

/* [out] What choosing a row left for the host: the clipboard, which is the
 * host's in this library. SET_CLIPBOARD carries the text to put there - the
 * core worked out *what*, which is the half only it can do - and PASTE asks
 * for what is there, which the host delivers back with kui_input_commit: a
 * focused editor takes it as typing, a focused on_key sink hears it as
 * {kind:"text"}. */
typedef struct KuiMenuAction {
    uint32_t size; /* = sizeof(KuiMenuAction) in, bytes filled out */
    uint32_t kind;
    KuiStr text; /* borrowed until the next kui_take_menu_action */
    /* The same selection with the formatting this library knows about -
     * bold, italic, a span's declared colour
     * (docs/adr/0017-selection-as-a-scope.md). Empty when there is none,
     * and never a replacement for `text`: a clipboard whose only flavour
     * is HTML pastes markup into every plain-text field on the machine.
     * Borrowed like `text`. */
    KuiStr html;
    /* KUI_MENU_ACTION_LOOK_UP only: where to anchor the panel - the
     * baseline origin of the selection's first line, logical viewport px.
     * Zero for every other kind. */
    float x;
    float y;
} KuiMenuAction;
#define KUI_MENU_ACTION_INIT ((KuiMenuAction){ .size = sizeof(KuiMenuAction) })

/* [out] Where a point landed in the text a keyed node drew (kui_text_hit):
 * a byte offset into that text - across the node's text runs in order, the
 * way the access tree reads a `line`, a `role="none"` subtree skipped - and
 * the visual row within that node, counted across every run the key covers
 * by where the rows sit (a row of inline runs is one row, a wrapped run as
 * many as it wrapped to; not the ordinal `line` node a pointer event's
 * `line` names - backlog AR30). */
typedef struct KuiTextHit {
    uint32_t size; /* = sizeof(KuiTextHit) in, bytes filled out */
    uint32_t line;
    uint64_t byte;
} KuiTextHit;
#define KUI_TEXT_HIT_INIT ((KuiTextHit){ .size = sizeof(KuiTextHit) })

/* [out] A caret rect (kui_caret_rect): logical px in viewport coordinates,
 * zero wide, one line tall. */
typedef struct KuiCaretRect {
    uint32_t size; /* = sizeof(KuiCaretRect) in, bytes filled out */
    float x, y, w, h;
} KuiCaretRect;
#define KUI_CARET_RECT_INIT ((KuiCaretRect){ .size = sizeof(KuiCaretRect) })

/* [out] What the last layout resolved for a scroll container
 * (kui_scroll_geometry): its box, its content size and the clamped offset,
 * logical px in viewport coordinates. */
typedef struct KuiScrollGeometry {
    uint32_t size; /* = sizeof(KuiScrollGeometry) in, bytes filled out */
    float x, y, w, h;
    float content_w, content_h;
    /* Where it is scrolled to, always a position within the content: the
     * retained offset clamped to the travel below. */
    float offset_x, offset_y;
    /* How far it can travel; zero on an axis that does not scroll. */
    float max_offset_x, max_offset_y;
} KuiScrollGeometry;
#define KUI_SCROLL_GEOMETRY_INIT ((KuiScrollGeometry){ .size = sizeof(KuiScrollGeometry) })

/* [out[]] A silent misconfiguration the core noticed (kui_take_warnings). `code`
 * is stable — "grow-weight-ignored", "transition-auto-key",
 * "duplicate-key" — `key` the node it is about, `message` for people.
 * Strings are borrowed until the next kui_take_warnings on the context. */
typedef struct KuiWarning {
    KuiStr code;
    uint64_t key;
    KuiStr message;
} KuiWarning;

/* -- Audio ----------------------------------------------------------------
 * Sounds are resources, playback is commands: kui_run plays them through
 * the bundled device; a host with its own loop drains
 * kui_take_audio_commands and reports finished playbacks with
 * kui_audio_ended. Volumes are linear amplitude (0..1), durations ms. */

/* [in] Options for kui_play. NULL = defaults; a struct is read literally, so start
 * from KUI_PLAY_INIT (volume 1) rather than zero. */
typedef struct KuiPlay {
    float volume;
    uint32_t looped;
    float fade_in_ms;
} KuiPlay;
#define KUI_PLAY_INIT ((KuiPlay){ .volume = 1.0f })

/* [in] What a kui_audio node declares; read literally (start from KUI_AUDIO_INIT). */
typedef struct KuiAudio {
    uint64_t src;    /* a registered sound */
    float volume;
    uint32_t looped;
    uint32_t paused; /* holds the playback; resumes when cleared */
    uint32_t finish; /* removal releases the playback to play out, rather
                        than stopping it; a loop still stops */
} KuiAudio;
#define KUI_AUDIO_INIT(id) ((KuiAudio){ .src = (id), .volume = 1.0f })

/* Audio command kinds (KuiAudioCommand.kind). */
enum {
    KUI_AUDIO_PLAY = 1,          /* playback, sound, volume, ms = fade-in, looped */
    KUI_AUDIO_STOP = 2,          /* playback, ms = fade-out */
    KUI_AUDIO_SET_VOLUME = 3,    /* playback, volume, ms = tween */
    KUI_AUDIO_PAUSE = 4,         /* playback, ms = fade-out */
    KUI_AUDIO_RESUME = 5,        /* playback, ms = fade-in */
    KUI_AUDIO_MASTER_VOLUME = 6, /* volume, ms = tween */
    KUI_AUDIO_UNLOAD = 7,        /* sound: drop any decoded copy */
};
/* [out[]] One queued command (kui_take_audio_commands). */
typedef struct KuiAudioCommand {
    uint32_t kind;
    uint64_t playback;
    uint64_t sound;
    float volume;
    float ms;
    uint32_t looped;
} KuiAudioCommand;

/* [in] Zero-initialized KuiTextStyle picks defaults (16px, default foreground). */
typedef struct KuiTextStyle {
    float size;
    float line_height; /* <= 0: default (size * 1.35) */
    uint32_t color;    /* 0: default foreground */
    uint32_t family;   /* KUI_FONT_* ; 0 = sans */
    uint64_t font;     /* registered font handle (kui_font_add*); non-zero overrides family */
    uint32_t wrap;     /* KUI_WRAP_* ; 0 = between words */
    uint32_t max_lines; /* at most this many lines; 0 = unlimited */
    uint32_t ellipsis; /* non-zero: end the last line with "..." when cut off (one line unless max_lines) */
    KuiStr features;   /* OpenType features, "tag=value ..." (bare tag = 1, -tag = 0), e.g.
                          "liga=0 calt=0" to keep a coding font's ligatures apart; zeroed =
                          the font's defaults. Appended without a bump, under the [in] rule as
                          it then stood (see the ABI block). */
    uint32_t decoration; /* KUI_DECO_* : underline / strikethrough over every glyph, paint only.
                            Appended without a bump, the same way. */
    uint32_t underline_color; /* 0xRRGGBBAA, the underline's own; 0 = the text's. Non-zero implies
                                 KUI_DECO_UNDERLINE. ABI 17. */
    uint32_t underline_style; /* KUI_UNDERLINE_* ; non-solid implies KUI_DECO_UNDERLINE. ABI 17. */
} KuiTextStyle;

/* [in] One run of a rich-text paragraph. */
typedef struct KuiSpan {
    KuiStr text;
    uint32_t color; /* 0: inherit paragraph color */
    uint32_t flags; /* KUI_SPAN_* */
    uint32_t bg;    /* 0xRRGGBBAA behind the span's glyphs alone, one rect per line it
                       covers, so it follows the span across a wrap; 0 = none. ABI 8:
                       spans travel as an array, so this append moved the stride. */
    uint32_t underline_color; /* the underline's own, 0 = the span's; non-zero implies
                                 KUI_SPAN_UNDERLINE. ABI 17. */
    uint32_t underline_style; /* KUI_UNDERLINE_* ; non-solid implies KUI_SPAN_UNDERLINE. ABI 17. */
} KuiSpan;

/* The window an app starts in - the one kui_run opens - which is always
 * live and is named "main". Other windows get the id their KUI_CMD_OPEN
 * carried. */
#define KUI_WINDOW_MAIN 0u

/* What kind of surface a declared window is (KuiWindowConfig.kind).
 * KUI_WINDOW_KIND_POPUP is a menu surface: borderless, off the taskbar,
 * owned by the window that declared it and closed when that window closes,
 * placed against KuiWindowConfig.anchor_* in screen coordinates rather than
 * clamped into a viewport, and non-activating - it must not take OS focus,
 * or opening a combobox would blur the field that opened it, so while it is
 * up you route the owner's keys to it and leave the owner's env `focused`
 * true. Reach for it only for the placements an in-window float cannot
 * make: a list taller than the window, a menu near an edge with nowhere
 * in-window to go, a panel beside the app.
 *
 * Any other value opens a normal window - so a host built against a later
 * header degrades to a window rather than to nothing - and raises the
 * unknown-window-kind warning saying so. */
#define KUI_WINDOW_KIND_NORMAL 0u
#define KUI_WINDOW_KIND_POPUP 1u

/* Why a window was asked to go away (kui_window_dismissed). */
#define KUI_DISMISS_OUTSIDE 0u
#define KUI_DISMISS_ESCAPE 1u

/* [in] What a declared window is (kui_window_declare), and what a
 * KUI_CMD_OPEN carries back out. Read literally, so start from
 * KUI_WINDOW_CONFIG_INIT - a normal, activating 640x480 window -
 * KUI_WINDOW_POPUP_INIT for a non-activating popup, or pass NULL for
 * exactly the first. A zero width or height means the default. */
typedef struct KuiWindowConfig {
    uint32_t kind;      /* KUI_WINDOW_KIND_*; anything else warns and opens normal */
    float width, height; /* initial inner size, logical px */
    uint32_t activates; /* whether opening it takes OS focus */
    /* KUI_WINDOW_KIND_POPUP only: what the popup is placed against, in the
     * declaring window's own logical coordinates - which is exactly the
     * rect an onLayout event reports for the field the menu belongs to, so
     * you need no new geometry query. Resolve it to screen coordinates
     * against that window's position. */
    float anchor_x, anchor_y, anchor_w, anchor_h;
} KuiWindowConfig;
#define KUI_WINDOW_CONFIG_INIT \
    ((KuiWindowConfig){ .kind = KUI_WINDOW_KIND_NORMAL, .width = 640, .height = 480, .activates = 1 })
#define KUI_WINDOW_POPUP_INIT \
    ((KuiWindowConfig){ .kind = KUI_WINDOW_KIND_POPUP, .activates = 0 })

/* [out] One window command (kui_take_window_command): what a chrome node
 * asked for, or what the declared window set decided. Plain data - an open
 * carries no title; the window's first frame declares one through
 * kui_window_title - so nothing borrowed enters your drain loop. */
typedef struct KuiWindowCommand {
    uint32_t size;   /* = sizeof(KuiWindowCommand) in, bytes filled out */
    uint32_t kind;   /* KUI_CMD_* */
    uint32_t window; /* which window; for KUI_CMD_OPEN the new window's id */
    uint16_t origin; /* KUI_CMD_OPEN: whose declaration won (0 = you, 1+ = an extension) */
    KuiWindowConfig config; /* KUI_CMD_OPEN only */
    float width, height;    /* KUI_CMD_SET_SIZE only: the size asked for, logical px */
    uint32_t owner;         /* KUI_CMD_OPEN: the window whose frame declared this one -
                             * a popup's owner, whose position its anchor is measured
                             * against and whose closing closes it (the core queues
                             * that KUI_CMD_CLOSE for you) */
} KuiWindowCommand;
#define KUI_WINDOW_COMMAND_INIT ((KuiWindowCommand){ .size = sizeof(KuiWindowCommand) })

/* [out] One polled event. `size` leads it so that a field appended later
 * reaches a host that has not recompiled as a shorter write, not a longer
 * one - `window` is the first field that actually did (ABI 4). A host built
 * against ABI 3 reserves through `payload`, which is still the ABI-1 floor,
 * so it keeps polling correctly and simply never sees `window`. */
typedef struct KuiEvent {
    uint32_t size;           /* = sizeof(KuiEvent) in, bytes filled out */
    uint16_t origin;         /* which frontend drew the node: 0 = you, 1+ = extensions */
    uint64_t key;
    const KuiValue *payload; /* borrowed; may be NULL */
    uint32_t window;         /* which window it came from: the KuiCtx's kui_env_set_window id */
    /* Where kui_reply sends a reply to this event (ABI 10). NULL on every
     * event you polled yourself; set only on the one an extension's
     * kui_ext_on_event is handed, and only for that call. Not yours to
     * read or write - pass the event back to kui_reply and it is used for
     * you. See KuiReplySink. */
    KuiReplySink *reply_sink;
    /* The key of the slot whose fill drew the node - what kui_key_of answers
     * for the slot's full name - or 0 for a node you drew yourself. One
     * extension fills many slots, so `origin` cannot say which; this routes
     * an event by the slot it came from. Appended under the [out] rule (no
     * bump): a host reserving the older layout never sees it. */
    uint64_t slot;
} KuiEvent;
#define KUI_EVENT_INIT ((KuiEvent){ .size = sizeof(KuiEvent) })

/* [lib] One quad of the display list, read through KuiDrawData.quads. You
 * stride the array with your own sizeof, so its layout is pinned by
 * KUI_ABI_VERSION rather than by anything in-band. */
typedef struct KuiQuad {
    float x, y, w, h;        /* physical pixels */
    float color[4];
    float border_color[4];
    float radius[4];         /* corner radii, clockwise from the top-left */
    float border_w;
    float blur;              /* KUI_QUAD_SHADOW: blur radius, also how far the rect is inflated */
    uint32_t kind;           /* KUI_QUAD_* */
    /* Which entry of KuiDrawData.clips clips this quad. An index and not
     * the clip itself since ABI 11: a clip is 32 bytes and a frame has a
     * handful of them, so carrying one per quad was paid by every quad of
     * every frame for a value nearly all of them share. Read
     * dd.clips[q.clip]; entry 0 clips nothing, so there is no null case. */
    uint32_t clip;
    uint32_t uv[4];          /* atlas texels: x, y, w, h; KUI_QUAD_SEGMENT: the endpoints as float bits */
} KuiQuad;

/* [lib] One clip the frame's quads name, read through KuiDrawData.clips.
 * You stride the array with your own sizeof, as with KuiQuad. */
typedef struct KuiClip {
    float rect[4];           /* physical px: pixels outside are transparent */
    /* Corner radii (physical px), clockwise from the top-left: pixels
     * outside the ROUNDED clip are transparent too. A clipping node with a
     * radius rounds what it clips, the way CSS rounds `overflow: hidden`
     * under a `border-radius`. All zero - every clip of a frame with no
     * rounded clipper - is the plain rect clip, so a renderer that ignores
     * this field is correct until an app rounds one. */
    float radius[4];
} KuiClip;

/* Where a KuiFragmentDraw's `image` is (backlog V1): none, the atlas, or a
 * texture of its own named by `image_texture`. */
enum { KUI_FRAGMENT_IMAGE_NONE = 0, KUI_FRAGMENT_IMAGE_ATLAS = 1,
       KUI_FRAGMENT_IMAGE_TEXTURE = 2 };

/* [out-array] One KUI_QUAD_FRAGMENT's draw, addressed by that quad's uv[0].
 * `params` is what the node declared, zero-padded to sixteen; the shader
 * reads them as four vec4<f32>. `fragment` is the handle, for
 * kui_fragment_source and for keying a pipeline cache. `image_source` says
 * where the draw's image (kui_fragment_with) is: KUI_FRAGMENT_IMAGE_NONE,
 * _ATLAS - bind the atlas at group 0, as for any fragment - or _TEXTURE -
 * bind the texture `textures[image_texture]` names in the atlas's place,
 * exactly as a KUI_QUAD_TEXTURE quad asks. `image_uv` is the texel rect
 * the shader is given as FragmentIn::image, in whichever is bound; zero
 * with none. The three were appended in ABI 15. */
typedef struct KuiFragmentDraw {
    uint64_t fragment;
    float params[16];
    uint32_t image_source;
    uint32_t image_texture;
    uint32_t image_uv[4];
} KuiFragmentDraw;

/* [out-array] One KUI_QUAD_TEXTURE's draw, addressed by that quad's uv[0].
 * `image` is the handle, for kui_image_pixels and for keying a texture
 * cache; `rev` moves with every kui_image_update, so a renderer that
 * uploaded this revision has nothing to do; `width`/`height` are the
 * pixels' size; `uv` is the texel rect to show - the whole image, or the
 * crop a KUI_FIT_COVER made. Added in ABI 14. */
typedef struct KuiTextureDraw {
    uint64_t image;
    uint32_t rev;
    uint32_t width, height;
    uint32_t uv[4];
} KuiTextureDraw;

/* [out] Everything a renderer needs for the finished frame. */
typedef struct KuiDrawData {
    uint32_t size;                /* = sizeof(KuiDrawData) in, bytes filled out */
    const KuiQuad *quads;
    size_t quad_count;
    float viewport_w, viewport_h; /* physical pixels */
    float scale;
    const uint8_t *atlas_pixels;  /* RGBA, atlas_size^2 * 4 bytes */
    uint32_t atlas_size;
    bool atlas_dirty;             /* re-upload when set or epoch changed */
    uint64_t atlas_epoch;
    /* One per KUI_QUAD_FRAGMENT quad, indexed by its uv[0]; NULL and 0 on a
     * frame that draws none. Added in ABI 9. */
    const KuiFragmentDraw *fragments;
    size_t fragment_count;
    /* The frame clock in seconds, for a fragment's `time`. */
    float time;
    /* The clips quads index through KuiQuad.clip. Never empty on a frame
     * that drew anything: entry 0 clips nothing. Added in ABI 11. */
    const KuiClip *clips;
    size_t clip_count;
    /* One per KUI_QUAD_TEXTURE quad, indexed by its uv[0]; NULL and 0 on a
     * frame that draws none. Added in ABI 14. */
    const KuiTextureDraw *textures;
    size_t texture_count;
} KuiDrawData;
#define KUI_DRAW_DATA_INIT ((KuiDrawData){ .size = sizeof(KuiDrawData) })

/* -- Context ------------------------------------------------------------- */
KuiCtx *kui_ctx_new(void);
void kui_ctx_free(KuiCtx *ctx);

/* -- Input (logical coordinates) + events -------------------------------- */
void kui_input_cursor(KuiCtx *ctx, float x, float y);
void kui_input_cursor_left(KuiCtx *ctx);
/* clicks: host-counted multi-click for presses (1 single, 2 double = word
 * select, 3 triple = line select); ignored on release. The count picks the
 * grain everywhere text can be selected — an editor, a `selectable` scope,
 * and a `cells` grid, where it counts in cells. This is the primary
 * button; kui_input_mouse_button carries the others. */
void kui_input_mouse(KuiCtx *ctx, bool down, uint32_t clicks);
/* Buttons (kui_input_mouse_button). Only the primary one presses, drags,
 * places the caret and clicks; the secondary one asks the node under it
 * for a context menu (KuiSpec.on_context_menu) and moves nothing else.
 * Nothing routes the rest yet; pass 3 + n for a further button n so a
 * driver need not drop it. */
enum {
    KUI_MOUSE_PRIMARY = 0,
    KUI_MOUSE_SECONDARY = 1,
    KUI_MOUSE_MIDDLE = 2,
    KUI_MOUSE_OTHER = 3,
};
/* kui_input_mouse for a named button (KUI_MOUSE_*). */
void kui_input_mouse_button(KuiCtx *ctx, bool down, uint32_t button,
                            uint32_t clicks);
void kui_input_scroll(KuiCtx *ctx, float dx, float dy); /* +y = scroll up */
void kui_input_text(KuiCtx *ctx, KuiStr text);   /* typing/paste -> focused editor */
/* Text an IME committed at the end of a composition: a focused editor takes
 * it as kui_input_text would; otherwise the focused on_key sink hears
 * {kind:"text", text, tag} - the one committed text a key event never
 * carries. Typing stays on kui_input_text (a sink already hears it as the
 * key event's text). */
void kui_input_commit(KuiCtx *ctx, KuiStr text);
/* In-progress IME composition shown at the focused editor's caret, or with
 * no editor focused delivered to the focused on_key sink as
 * {kind:"preedit", text, cursor, tag}; empty text clears it, the commit
 * arrives via kui_input_commit. cursor_* are byte offsets into text
 * (UINT32_MAX = none). The candidate window's place is kui_ime_rect either
 * way: a custom editor's `line` carrying `caret` says where. */
void kui_input_preedit(KuiCtx *ctx, KuiStr text, uint32_t cursor_start,
                       uint32_t cursor_end);
void kui_input_key(KuiCtx *ctx, uint32_t key, uint32_t mods); /* KUI_KEY_* + KUI_MOD_* */
/* Raw keys for on_key sinks (the editing keys go through kui_input_key
 * above). `code` is a single character as the layout produced it ("W", "$")
 * or a name ("left", "enter", "escape", "f5", ...); `physical` is the
 * US-QWERTY key at that *position*, spelled the same way, or {NULL, 0} when
 * the host does not track positions (then it is the position's US key: the
 * lower-case letter for a letter, `code` for everything else - the pair a
 * window reports for shift-Z is code "Z", physical "z"; backlog F65);
 * `kmods` is
 * KUI_KMOD_* bits; `text` is what the press inserts, or {NULL, 0} to derive
 * it from `code`; `repeat` marks an auto-repeat. The focused sink polls
 * {kind="key", phase="down", code, physical, ctrl, alt, shift, super, text,
 * repeat, tag} for each press; a sink whose KuiSpec set key_up hears the
 * release too, as the same payload with phase="up" and a null `text`. A
 * release whose press the sink never got resolves nothing, and moving focus
 * while a key is held delivers the "up" first, so a held-key binding (WASD,
 * press-and-hold) cannot be left stuck down. An unknown `code` or `physical`
 * is ignored.
 *
 * Passing both is what makes a keymap portable. A layout that produces
 * something outside ASCII (Cyrillic, Greek, Hebrew, Arabic) would leave a
 * Latin keymap matching nothing at all, so kui reports the position's US
 * letter as `code` instead; `physical` is there either way for a keymap that
 * would rather bind the finger than the label (WASD). A host passing
 * {NULL, 0} gets the default above. */
void kui_input_key_down(KuiCtx *ctx, KuiStr code, KuiStr physical,
                        uint32_t kmods, KuiStr text, bool repeat);
void kui_input_key_up(KuiCtx *ctx, KuiStr code, KuiStr physical,
                      uint32_t kmods);
/* Files dragged in from the OS (docs/adr/0031-a-drop-zone-is-a-row-and-
 * the-files-are-an-event.md): `paths` are `count` OS paths, x/y the
 * pointer in logical viewport coordinates. kui_input_drag_files is entering
 * and moving alike - the zone under the point hears {kind:"drop",
 * phase:"enter"|"move"}, a zone it left hears "leave", a repeat at the
 * same point is nothing. kui_input_drop_files is the release: the zone
 * there hears phase "drop" and no leave after it; with no zone there,
 * nothing but the lit zone's leave. kui_input_drag_cancel is the files
 * leaving the window or the OS ending the drag elsewhere. The host answers
 * the OS from kui_drop_target after each report: a copy operation over a
 * zone, not-allowed elsewhere, a release off every zone refused. winit's
 * own three file events carry no position; the Rust runner reads it from
 * the platform (see the ADR, decision 5). */
void kui_input_drag_files(KuiCtx *ctx, const KuiStr *paths, size_t count,
                          float x, float y);
void kui_input_drop_files(KuiCtx *ctx, const KuiStr *paths, size_t count,
                          float x, float y);
void kui_input_drag_cancel(KuiCtx *ctx);
/* A whole key going down, the way a window sends it - the call a host
 * driving kui from its own event loop wants, and the one a headless test
 * wants. Spelled exactly as kui_input_key_down, and it sends that press
 * first; then it asks the core what that key *means*, which is what
 * kui_input_key carries on its own: Escape dismisses a modal, Tab walks the
 * focus ring, an arrow nudges a focused slider, Space presses a focused
 * control, a printable character reaches the focused editor.
 *
 * The two calls above stay as the halves, for a host that means to drive one
 * channel and not the other. A host that means "the user pressed this key"
 * wants this one: kui_input_key_down(ctx, KUI_STR("escape"), ...) alone
 * leaves a modal open, because it is only half of what a keyboard does. An
 * unknown `code` or `physical` is ignored, as there. */
void kui_input_press(KuiCtx *ctx, KuiStr code, KuiStr physical, uint32_t kmods,
                     KuiStr text, bool repeat);
/* The same key coming up, spelled the way kui_input_press spells it
 * (`physical` included, {NULL, 0} for "same as `code`"). One channel,
 * because only one has a second half: the editing keys act on the way down,
 * so this is kui_input_key_up under the name that pairs with the press. */
void kui_input_release(KuiCtx *ctx, KuiStr code, KuiStr physical,
                       uint32_t kmods);
/* Lets go of every key the focused sink is holding, as if the user had
 * released them. Focus moves do this by themselves, and so does
 * kui_env_set(ctx, hz, false) - the OS stops delivering key events to a
 * window that lost the keyboard, so the release of anything held over an
 * app switch would never arrive, and reporting the loss is what lets go.
 * This is the same release for a host with a reason of its own. */
void kui_release_held_keys(KuiCtx *ctx);
/* Physical modifier state changed (KUI_KMOD_* bits); the host polls a
 * {kind="modifiers", shift, ctrl, alt, super} event when it differs. */
void kui_input_modifiers(KuiCtx *ctx, uint32_t mods);
bool kui_poll_event(KuiCtx *ctx, KuiEvent *out);

/* -- Host environment ---------------------------------------------------- */
/* These setters are the whole of C's `env`: the fields of kui_core's Env,
 * SystemEnv, WindowEnv and AudioEnv, one argument each, in the order
 * docs/props.md's Env table lists them (schema::ENV_FIELDS, the one
 * statement of the shape every binding's reading is pinned to). A C host is
 * the frame driver, so it writes the facts and has no reading of them back
 * - Rust's ui.env(), Lua's view(env) and Node's ctx.env() are the readers -
 * except the window id, which kui_ctx_window answers. The two facts Lua and
 * Node derive or carry beside these (the frame budget, the viewport) are
 * the host's own numbers here. kui-ffi's tests hold these prototypes to the
 * table:
 *   kui_env_set         refresh_hz, focused          (Env)
 *   kui_env_set_system  appearance, accent, motion, locale  (SystemEnv:
 *                       what the user set in the OS, all four with an
 *                       "I cannot tell" reading that is the default)
 *   kui_env_set_window  window, custom_chrome, maximized, fullscreen,
 *                       controls_w, controls_h       (WindowEnv; the
 *                       controls rect flattened to its extent at the
 *                       window origin, the shape Lua also reads)
 *   kui_env_set_assistive  assistive                (SystemEnv's fifth:
 *                       whether an accessibility client is listening,
 *                       which is not a setting and has its own door)
 *   kui_env_set_always_on_top  always_on_top       (WindowEnv's level:
 *                       what the host did about kui_always_on_top_get,
 *                       its own door so an older host has nothing to
 *                       recompile - declared beside the getter, below)
 *   kui_env_set_audio   device, live                (AudioEnv: what the
 *                       host's output device is doing)
 */
/* Host facts for views to read (refresh_hz <= 0 = unknown). Survives across
 * frames; set on change or every frame, either works. `focused` going
 * false lets go of every key the focused sink is holding (see
 * kui_release_held_keys): the synthetic releases are polled like any
 * event. */
void kui_env_set(KuiCtx *ctx, float refresh_hz, bool focused);
/* The OS light/dark setting, as kui_env_set_system takes it and Node and
 * Lua read back as "unknown"/"light"/"dark". Zero is unknown - a host that
 * never calls the setter reports that it cannot tell, rather than a guess a
 * view would paint. */
enum {
    KUI_APPEARANCE_UNKNOWN = 0,
    KUI_APPEARANCE_LIGHT = 1,
    KUI_APPEARANCE_DARK = 2,
};
/* The OS reduce-motion setting, zero unknown for the same reason.
 * KUI_MOTION_REDUCED is "the user asked for less animation"; nothing in the
 * core acts on it, since only a view knows which of its animations carries
 * meaning and which is decoration. */
enum {
    KUI_MOTION_UNKNOWN = 0,
    KUI_MOTION_FULL = 1,
    KUI_MOTION_REDUCED = 2,
};
/* What the user set in the OS, for views to read: a KUI_APPEARANCE_*, the
 * accent colour as 0xRRGGBBAA (0 = cannot tell, since a fully transparent
 * accent is not a colour anyone was given), a KUI_MOTION_*, and the UI
 * language as a BCP-47 tag ("en-US"; empty = cannot tell). The tag is
 * copied, so the KuiStr need not outlive the call; one longer than 31 bytes
 * or not ASCII is not a tag and reads back as unknown rather than as a
 * truncated one. kui does not parse it.
 *
 * Separate from kui_env_set because these change when the user opens a
 * settings app, not when a window moves: push them at startup and from the
 * OS's change notification. On a context handed to kui_run_with it is the
 * window's pin instead - see there. Adding a setter rather than arguments is also
 * what keeps this off KUI_ABI_VERSION - a host that never calls it is
 * unaffected, and one that does fails to link against an older library,
 * which is loud. */
void kui_env_set_system(KuiCtx *ctx, uint32_t appearance, uint32_t accent,
                        uint32_t motion, KuiStr locale);
/* Whether assistive technology is listening, as kui_env_set_assistive takes
 * it and Node and Lua read back as "unknown"/"none"/"listening". Zero is
 * unknown: a host with no accessibility bridge reports that it cannot
 * tell, which is the honest default and what a headless core says. */
enum {
    KUI_ASSISTIVE_UNKNOWN = 0,
    KUI_ASSISTIVE_NONE = 1,
    KUI_ASSISTIVE_LISTENING = 2,
};
/* Whether an accessibility client has asked for the tree, for views to
 * read as env.system.assistive - the reading that changes what a view
 * *says* rather than what it draws: an alert announces when something is
 * listening and blinks when nothing is. A host that bridges the platform's
 * accessibility API pushes KUI_ASSISTIVE_LISTENING when a client first asks
 * it for the tree (any client: a probe, an inspector, a screen reader) and
 * KUI_ASSISTIVE_NONE if the platform ever says the client left - which, of
 * the AccessKit adapters, only AT-SPI does; on macOS and Windows the
 * reading rises once and stays. kui_env_set_system leaves it alone, so a
 * host re-pushing the settings on an OS notification keeps it. Its own
 * setter rather than a fifth argument on kui_env_set_system because it is
 * not a setting and does not arrive with them, and because a setter is
 * additive where an argument is an ABI break: off KUI_ABI_VERSION, like
 * kui_env_set_audio. */
void kui_env_set_assistive(KuiCtx *ctx, uint32_t assistive);
/* The host's audio output device, as kui_env_set_audio takes it and Node
 * and Lua read back as "closed"/"opening"/"open"/"failed". Zero is closed,
 * which is what a host that never calls the setter - or has no device -
 * reports. */
enum {
    KUI_AUDIO_DEVICE_CLOSED = 0,
    KUI_AUDIO_DEVICE_OPENING = 1,
    KUI_AUDIO_DEVICE_OPEN = 2,
    KUI_AUDIO_DEVICE_FAILED = 3,
};
/* What the host's output device is doing, for views to read: a
 * KUI_AUDIO_DEVICE_*, and how many playbacks are started or waiting on the
 * open - a play that waits counts from the frame it was asked until the
 * open answers, and one the device then refuses leaves the count on the
 * apply that refuses it (backlog F63). A fact, not a verb - nothing here
 * closes the device; the host that
 * opened it does that once it has been idle a while. Worth pushing because
 * an open stream is a real-time thread whether or not anything plays,
 * which is the whole of an idle app's CPU once it has held a sound: a
 * view that shows the device still open long after its last sound is
 * showing a bug that otherwise only `top` can see. Push it every frame,
 * or on change; either works. Additive, like kui_env_set_system, and off
 * KUI_ABI_VERSION for the same reason. */
void kui_env_set_audio(KuiCtx *ctx, uint32_t device, uint32_t live);
/* -- Theme --------------------------------------------------------------- */
/* The colours a view paints with, as roles rather than values, derived from
 * the two facts above: the appearance picks the base, the accent recolours
 * it. See docs/adr/0019-a-theme-derived-from-appearance-and-accent.md and
 * the Theme table in docs/props.md, which lists every role with the value
 * it takes on each base.
 *
 * The stock widgets read it already - kui_button_with, the context menu,
 * the tooltip, the field, the scrollbars, the focus ring, and any text
 * whose `color` is zero - so a host that pushes the OS appearance through
 * kui_env_set_system and does nothing else already follows the OS. Read it
 * for paint of your own:
 *
 *   KuiTheme t = KUI_THEME_INIT;
 *   kui_theme(ctx, &t);
 *   spec.bg = t.surface;
 *
 * The roles are kui_core::schema::THEME_ROLES field for field and in that
 * order; kui-ffi's tests hold this struct to that table. Fields are
 * 0xRRGGBBAA. */
typedef struct KuiTheme {
    uint32_t size; /* = sizeof(KuiTheme) in, bytes filled out */
    /* KUI_APPEARANCE_* - which base this came from. UNKNOWN is the dark
     * base, without claiming the user chose it: what kui painted before
     * there were themes, which is why following the OS is the default. */
    uint32_t appearance;
    /* What a disabled control's opacity is multiplied by. */
    float disabled_opacity;
    uint32_t bg;
    uint32_t surface;
    uint32_t raised;
    uint32_t sunken;
    uint32_t border;
    uint32_t border_strong;
    uint32_t fg;
    uint32_t muted;
    uint32_t faint;
    uint32_t accent;
    uint32_t accent_hover;
    uint32_t accent_pressed;
    uint32_t on_accent;
    uint32_t accent_soft;
    uint32_t selection;
    uint32_t focus_ring;
    uint32_t hover;
    uint32_t pressed;
    uint32_t success;
    uint32_t warning;
    uint32_t danger;
    uint32_t scrollbar;
    uint32_t scrollbar_active;
} KuiTheme;
#define KUI_THEME_INIT ((KuiTheme){ .size = sizeof(KuiTheme) })

/* This window's palette as of the current or last frame. False for a bad
 * context, a NULL out, or a reservation below the ABI-1 layout. */
bool kui_theme(KuiCtx *ctx, KuiTheme *out);
/* Keep following the OS's light/dark, but paint this accent instead of the
 * OS's - a host with a brand colour. 0xRRGGBBAA; zero goes back to
 * following the OS for the accent too, which is the default. Everything
 * that comes off the accent moves with it: the button's hover and pressed
 * shades, the label on it (black or white by luminance), the selection
 * tint and the focus ring. */
void kui_theme_set_accent(KuiCtx *ctx, uint32_t accent);
/* Pin the whole palette: exactly these colours, following neither the OS's
 * appearance nor its accent. NULL goes back to deriving both. Read one with
 * kui_theme and change the roles you mean to change rather than zeroing a
 * fresh struct - a zeroed role is transparent, not "leave it alone". */
void kui_theme_set(KuiCtx *ctx, const KuiTheme *theme);

/* The sizes the stock widgets are built from - the palette's other axis
 * (kui_core::metrics, backlog T2): kui_core::schema::METRIC_ROLES field for
 * field and in that order; kui-ffi's tests hold this struct to that table.
 * Logical px, before the scale factor, which the renderer applies after.
 * Read it so a control of your own agrees with the stock ones:
 *
 *   KuiMetrics m = KUI_METRICS_INIT;
 *   kui_metrics(ctx, &m);
 *   spec.radius = m.radius; spec.pad_l = m.control_pad_x;
 */
typedef struct KuiMetrics {
    uint32_t size; /* = sizeof(KuiMetrics) in, bytes filled out */
    float control_text;   /* a button's label */
    float chrome_text;    /* a menu row, a menu-bar title, the titlebar's title */
    float hint_text;      /* a tooltip */
    float radius;         /* every stock surface's corner */
    float radius_inner;   /* a row inside one: a menu row, a menu-bar title */
    float control_pad_x;  /* a button's padding */
    float control_pad_y;
    float field_pad_x;    /* a text field's */
    float field_pad_y;
    float hint_pad_x;     /* a tooltip's */
    float hint_pad_y;
    float menu_pad_x;     /* a menu row's; a menu-bar title's is 2 px shorter */
    float menu_pad_y;
    float menu_width;     /* a menu panel's width */
    float menu_bar_h;     /* the drawn menu bar's height */
    float titlebar_h;     /* the strip where it is the app's alone: 32 on Windows, 34 elsewhere;
                             under macOS custom chrome the strip is window.controls_h tall */
} KuiMetrics;
#define KUI_METRICS_INIT ((KuiMetrics){ .size = sizeof(KuiMetrics) })

/* The metrics in effect. False for a bad context, a NULL out, or a
 * reservation below the ABI-1 layout. */
bool kui_metrics(KuiCtx *ctx, KuiMetrics *out);
/* Make these the frame's: every stock widget from the next node on is
 * built from them. NULL restores the stock set. Read one with kui_metrics
 * and change the fields you mean to change rather than zeroing a fresh
 * struct - a zeroed metric is zero, not "leave it alone". Density is the
 * host's to choose; nothing in the OS is followed. */
void kui_metrics_set(KuiCtx *ctx, const KuiMetrics *metrics);

/* -- Tokens (docs/adr/0027-tokens-beside-the-theme.md) ----------------------
 *
 * The app's own named colours and lengths, beside the theme's roles and
 * the metrics'. A colour token has a value per base - the same one twice
 * for a colour that does not follow the appearance - and a length token
 * is logical px. Declared whole with kui_tokens_set into the table of
 * whoever is drawing: the host's outside a plugin's kui_ext_view, the
 * plugin's own inside it, so a guest's declaration never replaces the
 * host's palette. A plugin reads the host's names through the same two
 * readers - its own table first, then the host's.
 *
 * A C prop carries no reference (KuiSpec.bg is a bare uint32_t), so read
 * the value back and write it:
 *
 *   KuiColorToken colors[] = {
 *     { KUI_STR("peach"), 0xffcc99ffu, 0xffcc99ffu },
 *     { KUI_STR("ink"),   0x111111ffu, 0xeeeeeeffu },
 *   };
 *   KuiLengthToken lengths[] = { { KUI_STR("side_w"), 132.0f } };
 *   kui_tokens_set(ctx, colors, 2, lengths, 1);
 *   uint32_t peach; kui_token_color(ctx, KUI_STR("peach"), &peach);
 *
 * A name a theme or metrics role owns ("surface", "radius") is dropped
 * with a `reserved-token` warning; asking for one answers with the role.
 * Both structs are [in] arrays: an append moves the stride and is a bump. */
typedef struct KuiColorToken {
    KuiStr name;
    uint32_t light; /* 0xRRGGBBAA on the light base */
    uint32_t dark;  /* 0xRRGGBBAA on the dark base, and on an unknown appearance */
} KuiColorToken;
typedef struct KuiLengthToken {
    KuiStr name;
    float value; /* logical px, before the scale factor */
} KuiLengthToken;
/* Replace the drawing origin's table with these. Either array may be NULL
 * with a zero count. */
void kui_tokens_set(KuiCtx *ctx, const KuiColorToken *colors, size_t color_count,
                    const KuiLengthToken *lengths, size_t length_count);
/* This frame's value for a colour token or a theme role, by name. False
 * for a name nothing declared or one that is a length (both also raise
 * `unknown-token`, once per name), a bad context or a NULL out. */
bool kui_token_color(KuiCtx *ctx, KuiStr name, uint32_t *out);
/* The same for a length token or a metrics role, in logical px. */
bool kui_token_length(KuiCtx *ctx, KuiStr name, float *out);

/* -- Derived tokens (docs/adr/0028-derived-tokens.md) ----------------------
 *
 * A colour computed from another: a name, the colour token or theme role
 * it derives from, and a chain of ops folded over it in order, each a
 * verb and its operands. The core resolves it on read, so a recipe over a
 * themed source runs on the half in effect, and the devtools list it
 * with its recipe. Added to the drawing origin's table after
 * kui_tokens_set, and read back like any other token:
 *
 *   KuiColorOp lit[]  = { { KUI_OP_LIFT, 0.3f, {0} } };
 *   KuiColorOp wash[] = { { KUI_OP_LIFT, 0.3f, {0} }, { KUI_OP_ALPHA, 0.5f, {0} } };
 *   KuiColorOp ink[]  = { { KUI_OP_READABLE, 4.5f, KUI_STR("black") } };
 *   KuiDerivedToken derived[] = {
 *     { KUI_STR("peach_lit"),  KUI_STR("peach"), lit,  1 },
 *     { KUI_STR("peach_wash"), KUI_STR("peach"), wash, 2 },
 *     { KUI_STR("peach_ink"),  KUI_STR("peach"), ink,  1 },
 *     { KUI_STR("accent2"),    KUI_STR("accent"), NULL, 0 },   // an alias of the role
 *   };
 *   kui_tokens_derive(ctx, derived, 4);
 *   uint32_t lit; kui_token_color(ctx, KUI_STR("peach_lit"), &lit);
 *
 * The verbs: KUI_OP_LIFT / KUI_OP_DARKEN move toward white / black by
 * `t`; KUI_OP_RAISE toward the front of whichever base is in effect;
 * KUI_OP_ALPHA sets the alpha to `t`; KUI_OP_MIX moves toward the token
 * `other` names by `t`; KUI_OP_READABLE moves toward black or white -
 * whichever reads on `other` - until it clears the ratio `t` on it.
 * `other` is empty for the first four and a name for the last two.
 *
 * A source that is no colour token declared before it and no theme role
 * drops that token with `unknown-token` (naming both), so the rest of the
 * call still lands. An op that is malformed - `op` past KUI_OP_READABLE,
 * `other` given to a verb that takes none or missing from one that does -
 * makes the call return false and add nothing. Both structs are [in]
 * arrays: an append moves the stride and is a bump. */
enum {
    KUI_OP_LIFT = 0, KUI_OP_DARKEN = 1, KUI_OP_RAISE = 2,
    KUI_OP_ALPHA = 3, KUI_OP_MIX = 4, KUI_OP_READABLE = 5
};
typedef struct KuiColorOp {
    uint8_t op;    /* KUI_OP_* */
    float t;       /* the amount, alpha, or contrast ratio */
    KuiStr other;  /* a colour token or role name for MIX and READABLE; empty otherwise */
} KuiColorOp;
typedef struct KuiDerivedToken {
    KuiStr name;
    KuiStr from;            /* a colour token declared before this one, or a theme role */
    const KuiColorOp *ops;  /* NULL with a zero count is an alias */
    size_t op_count;
} KuiDerivedToken;
bool kui_tokens_derive(KuiCtx *ctx, const KuiDerivedToken *derived, size_t count);

/* The frame clock for transitions (monotonic seconds, any origin). Set before
 * each kui_frame_begin; never setting it makes transitions snap. */
void kui_set_time(KuiCtx *ctx, double now_secs);
/* True when the last frame left a transition mid-flight: draw another frame
 * without waiting for input. */
bool kui_animating(KuiCtx *ctx);
/* The same by kind (backlog F64): the KUI_OWED_* bits of what the last
 * frame left owed. A host draws another frame for any of them, so
 * kui_animating is `kui_owed(ctx) != 0`; a test masks KUI_OWED_CYCLE off to
 * wait for the transitions to run out under a keyframe `repeat` cycle,
 * which never ends and so never lets kui_animating clear. */
enum {
    KUI_OWED_TRANSITION = 1,  /* a finite transition (a leg, a spring) mid-flight */
    KUI_OWED_CYCLE = 2,       /* a keyframe cycle running: always, while its node is drawn */
    KUI_OWED_DEPART = 4,      /* an exit animation still departing */
    KUI_OWED_REQUESTED = 8,   /* an `animate` node: a frame the view asked for */
    KUI_OWED_AUTOSCROLL = 16, /* a held drag scrolling its container */
};
uint32_t kui_owed(KuiCtx *ctx);
/* Rasterize outline glyphs as LCD subpixel coverage (KUI_QUAD_GLYPH_SUBPIXEL)
 * instead of alpha masks. Only turn it on if your renderer blends per
 * channel. Flipping it re-rasterizes every glyph. */
void kui_set_subpixel_text(KuiCtx *ctx, bool on);
/* Byte budget for the shaped-text cache: every text a frame draws is shaped
 * once and kept; past this many (estimated) bytes the least recently drawn
 * entries go at the start of the next frame, never what the last frame
 * drew. Default 64 MB. A terminal streaming new lines lowers it, a viewer
 * that wants every page it showed kept warm raises it. */
void kui_set_text_cache_budget(KuiCtx *ctx, size_t bytes);
/* What that cache holds, in the estimated bytes the budget is charged
 * against. */
size_t kui_text_cache_bytes(KuiCtx *ctx);
/* Window facts for views (widgets adapt to them). `window` is which window
 * this context draws - KUI_WINDOW_MAIN, or the id a KUI_CMD_OPEN carried -
 * and every KuiEvent it hands out says so. controls_w/h > 0 describe the
 * top-left keep-out rect of OS-drawn controls (macOS traffic lights under
 * custom chrome). */
void kui_env_set_window(KuiCtx *ctx, uint32_t window, bool custom_chrome,
                        bool maximized, bool fullscreen, float controls_w,
                        float controls_h);
/* Pops the next window command into out: what chrome nodes asked for since
 * the last drain, and the KUI_CMD_OPEN / KUI_CMD_CLOSE the declared window
 * set decided at the last kui_frame_finish. Returns false, writing nothing,
 * when there is none - or when out->size is below the layout this library
 * knows (start from KUI_WINDOW_COMMAND_INIT), leaving the command queued.
 * Call after each input dispatch and each frame until it returns false:
 *
 *     KuiWindowCommand cmd = KUI_WINDOW_COMMAND_INIT;
 *     while (kui_take_window_command(ctx, &cmd)) {
 *         switch (cmd.kind) {
 *         case KUI_CMD_OPEN:  open a window for cmd.window with cmd.config; break;
 *         case KUI_CMD_CLOSE: close window cmd.window (exit if KUI_WINDOW_MAIN); break;
 *         case KUI_CMD_SET_SIZE: resize cmd.window to cmd.width x cmd.height; break;
 *         ...
 *         }
 *     }
 */
bool kui_take_window_command(KuiCtx *ctx, KuiWindowCommand *out);
/* Declares that a window named `name` exists this frame
 * (docs/adr/0004-multi-window.md). It opens on the first frame any window's
 * frame declares it - cfg is read then and never again (NULL means
 * KUI_WINDOW_CONFIG_INIT), because the user owns a window's geometry once
 * it exists - and closes on the first frame none does. The KUI_CMD_OPEN /
 * KUI_CMD_CLOSE arrive through kui_take_window_command, and the app sees
 * {kind:"window", phase:"opened"|"closed", name, id} through
 * kui_poll_event. A window the user closed (kui_window_closed) stays closed
 * while it is still declared - stop declaring it, then declare it again -
 * and says so with the "window-declared-while-closed" warning. Two
 * declarations of one name that disagree on the frame it opens warn
 * "duplicate-window-config"; the lowest declaring window's first one
 * wins. Call between kui_frame_begin and kui_frame_finish.
 *
 * A cfg with kind = KUI_WINDOW_KIND_POPUP (start from
 * KUI_WINDOW_POPUP_INIT) declares a menu surface instead: open it
 * borderless, off the taskbar, without activating it, placed against
 * cfg->anchor_* resolved into screen coordinates against cmd.owner's
 * position - and while it is up, route cmd.owner's key input to it and
 * leave the owner's env `focused` true, so the field that opened it keeps
 * its ring while the arrows walk the list. Report a press outside it or an
 * Escape with kui_window_dismissed; you close it when the app stops
 * declaring it, and the core closes it for you when its owner closes. */
void kui_window_declare(KuiCtx *ctx, KuiStr name, const KuiWindowConfig *cfg);
/* Asks the driver to resize a window to w x h logical px, or to give it
 * keyboard focus. Requests, not declarations: kui_window_declare's config is
 * read on the opening edge only, because the user owns a window's size once
 * it exists, so these are the only way an app moves a live window. They are
 * queued the way kui_reveal queues a scroll and come back out of your own
 * kui_take_window_command - KUI_CMD_SET_SIZE, carrying the size in
 * cmd.width/cmd.height, and KUI_CMD_FOCUS - for you to apply; a headless
 * host that never drains ignores them. window is the id events carry
 * (KuiEvent.window), KUI_WINDOW_MAIN for the launcher's. The size the window
 * actually becomes arrives as the ordinary resize event, and whether focus
 * was granted through kui_env_set's focused - neither is a reply here. */
void kui_set_window_size(KuiCtx *ctx, uint32_t window, float w, float h);
void kui_focus_window(KuiCtx *ctx, uint32_t window);
/* Reports that window `id` was asked to go away: a press landed outside it
 * (KUI_DISMISS_OUTSIDE) or Escape reached it (KUI_DISMISS_ESCAPE). The app
 * sees {kind:"dismiss", reason, name, id} through kui_poll_event, and
 * nothing closes - the same contract a modal node's dismissal has, one
 * level up: only the app can stop declaring the window, on the frame it
 * decides to. You report it because neither fact is the frame's: a press
 * outside a window lands in another surface, and a non-activating popup is
 * never the window the OS hands keys to. Nothing happens for a window that
 * is not open. */
void kui_window_dismissed(KuiCtx *ctx, uint32_t id, uint32_t reason);
/* Which window this context draws (what kui_env_set_window set;
 * KUI_WINDOW_MAIN until then). In a kui_run view callback: the window
 * being drawn. */
uint32_t kui_ctx_window(KuiCtx *ctx);
/* Its name: "main" for the launcher's, else the name it was declared
 * under. Borrowed until the next call on the same context. */
bool kui_ctx_window_name(KuiCtx *ctx, KuiStr *out);
/* You report that the OS closed window `id` (its close button, the window
 * manager). It stays closed while still declared, whatever only it declared
 * closes with it, and the app gets {kind:"window", phase:"closed"}. Nothing
 * happens for KUI_WINDOW_MAIN or for a window already closed by the diff. */
void kui_window_closed(KuiCtx *ctx, uint32_t id);
/* The pointer shape for where the pointer is now (KUI_CURSOR_*, 0 only on a
 * bad context): the `cursor` the topmost node under it declared, the
 * I-beam over text, the arrow otherwise; a captured drag holds the
 * dragged node's. A state, not a queue — read after each input dispatch
 * and each frame, and set the real cursor when the answer changes. */
uint32_t kui_cursor_shape(KuiCtx *ctx);
/* Declares this frame's window title (cleared each kui_frame_begin). */
void kui_window_title(KuiCtx *ctx, KuiStr title);
/* The title declared this frame, if any — diff and apply after
 * kui_frame_finish. The view is valid until the next kui_frame_begin. */
bool kui_window_title_get(KuiCtx *ctx, KuiStr *out);
/* Declares that this frame wants the window kept above every other app's
 * - a floating palette, a picture-in-picture player, a timer. Cleared each
 * kui_frame_begin like the title, but its default is false rather than
 * "leave as-is": a frame that stops calling this is what lowers the window
 * again, so a pin button is a toggle on the app's own state. A popup keeps
 * its own level whatever its owner declares. */
void kui_set_always_on_top(KuiCtx *ctx, bool on_top);
/* Whether the frame that just finished asked for the window on top - diff
 * against the level you applied and set it on change only:
 *
 *     bool want = kui_always_on_top_get(ctx);
 *     if (want != applied) { set_window_level(hwnd, want); applied = want; }
 *     kui_env_set_always_on_top(ctx, applied);
 *
 * False for a frame that never asked. */
bool kui_always_on_top_get(KuiCtx *ctx);
/* What you did about it, for views to read as env.window.always_on_top:
 * the level the window actually has, so a pin button draws the platform's
 * answer and not the app's guess (a window manager can refuse or drop it).
 * A host that never applies a level never calls this and reports false.
 * Its own setter rather than a seventh argument on kui_env_set_window for
 * the reason kui_env_set_assistive has one: additive, and off
 * KUI_ABI_VERSION. */
void kui_env_set_always_on_top(KuiCtx *ctx, bool always_on_top);

/* -- Spec helpers -------------------------------------------------------- */
/* Fills spec->float_* from a preset name — the same four the JSX and Lua
 * `float` props take ("parent", "viewport", "below", "above"), resolved by
 * the same function in kui-core, so "below" cannot mean one thing here and
 * another there. Returns false and leaves the spec alone for an unknown
 * name. The fields stay writable, so a preset is a starting point:
 *
 *     KuiSpec s = {0};
 *     kui_spec_float_preset(&s, KUI_STR("below"));
 *     s.float_dy = 12.0f;  // same attachment, a wider gap
 */
bool kui_spec_float_preset(KuiSpec *spec, KuiStr name);

/* -- Frame building ------------------------------------------------------ */
/* w/h are logical pixels. A frame begun at a different size or scale than
 * the last one posts a {kind="resize", width, height, scale} event on the
 * root, polled after kui_frame_finish like any other. */
void kui_frame_begin(KuiCtx *ctx, float w, float h, float scale);
void kui_root(KuiCtx *ctx, const KuiSpec *spec);
/* on_click may be NULL; consumed when given. Returns the node key. */
uint64_t kui_open(KuiCtx *ctx, const KuiSpec *spec, KuiValue *on_click);
uint64_t kui_open_keyed(KuiCtx *ctx, KuiStr label, const KuiSpec *spec, KuiValue *on_click);
/* The same node under a data *index* rather than a name: the key auto-keying
 * would have given the i-th child, given to this one wherever it sits. What
 * a virtualised list is for - a view that builds rows 900..930 of ten
 * thousand opens each with its own row number, so a row keeps its hover,
 * focus, edit buffer and tweens as the built range slides over it, and a
 * list that builds every row agrees with one that builds a screenful.
 * Indices and names are separate namespaces, so a spacer keyed "lead" cannot
 * collide with row 0. Added after ABI 11 as a new symbol: no struct moved,
 * so a binary built against ABI 11 keeps working unrecompiled. */
uint64_t kui_open_indexed(KuiCtx *ctx, uint64_t index, const KuiSpec *spec, KuiValue *on_click);
/* How many indexed rows the open node's virtual list has, built or not
 * (`rowCount`): what Select All inside a `selectable` virtual list spans,
 * since the rows the frame built are all the core can see (ADR 0017, tier
 * 3). Called inside the list's container, after its kui_open_*. The copy
 * that follows is a selectionrange ask whose `to` byte is past the last
 * row's length when that row was not built - cut it to the row. A new
 * symbol, so KUI_ABI_VERSION stays 15. */
void kui_row_count(KuiCtx *ctx, uint64_t rows);
/* A slot: a position among the current node's children that an extension
 * fills, in place (docs/adr/0014-slots-an-extension-fills-in-place.md).
 * `name` is the full name, `namespace/slot`: the namespace the host gave
 * the extension when it loaded it, and the slot in the extension's own
 * vocabulary ("fs/panel"). `params` may be NULL and is borrowed for the
 * call - you keep it; it is what the extension reads back with
 * kui_slot_params, declared every frame and retained by nothing. "ns/root"
 * is the fill after the view for an extension listing no slots, and
 * declaring it moves that fill here. Returns false when the name was
 * already declared this frame (a `duplicate-slot` warning).
 *
 * Whatever fills it is loaded with kui_ctx_add_extension, into a context
 * you build frames on yourself or hand to kui_run_with. With nothing loaded this
 * still records the position and kui_key_of still answers the slot's key,
 * so a host can declare its layout before it has a plugin to put in it.
 *
 * An extension may call this from its own kui_ext_view too: the slot is
 * declared inside its fill and keyed there, which is how a guest puts a
 * plugin in the middle of its own tree. What a plugin cannot do is load
 * the thing that fills it - its context has no list of its own - so the
 * name has to be one the host above it already loaded. Its own slot is
 * the one name that finds nobody, since a plugin is out of the list while
 * it draws: that warns (`recursive-slot`) and draws nothing. */
bool kui_slot(KuiCtx *ctx, KuiStr name, const KuiValue *params);

/* -- Extensions: loading one from C (ADR 0014) --------------------------- */

/* Load the shared library at `path` as an extension of this context, under
 * `namespace` - the word that fronts every slot name it fills
 * ("namespace/panel"). An empty `namespace` takes the plugin's own
 * kui_ext_name.
 *
 * False on any refusal, with the reason readable until the next call
 * through kui_ctx_extension_error: the library will not load, it declares
 * no kui_ext_abi or one this build does not implement, it has no
 * kui_ext_view, or the namespace is empty or already another extension's.
 *
 * The context owns it from here and unloads it in kui_ctx_free, after the
 * plugin's own kui_ext_free. Load before the first frame: origins are
 * positions in the list, so one added later renumbers what follows it.
 *
 * It must be a context of your own. One that borrows a frame - the ctx a
 * kui_run_with view callback is handed, or a plugin's kui_ext_view - fills
 * its slots from the list one level up and is gone at the end of the call,
 * so this refuses it and says so rather than loading a plugin that would
 * never be asked to draw.
 *
 * The plugin's code runs in your process, on your thread, on your frame.
 * Loading it is trusting it exactly as much as linking it would be. */
bool kui_ctx_add_extension(KuiCtx *ctx, KuiStr namespace_, KuiStr path);
/* Why the last kui_ctx_add_extension said false; false (out untouched)
 * when it succeeded or none has run. Borrowed until the next call. */
bool kui_ctx_extension_error(KuiCtx *ctx, KuiStr *out);
/* How many are loaded. Their origins are 1..=n in the order added; an
 * event's origin of 0 is your own nodes. */
uint32_t kui_ctx_extension_count(KuiCtx *ctx);
/* The namespace the extension at `origin` was loaded under - what turns an
 * event's origin back into the name you chose. False (out untouched) for 0
 * or an origin nothing is loaded at. Borrowed until the next call. */
bool kui_ctx_extension_namespace(KuiCtx *ctx, uint16_t origin, KuiStr *out);
/* Draggable container: press-drag emits {kind="drag", phase="start"|"move"|
 * "end", x, y, dx, dy, parent, tag} events. dx/dy are the displacement from
 * the press point in every phase - start is zero, a move is how far the
 * pointer is from where it pressed, end is the whole distance - so a handler
 * sets value = start + dx rather than summing, and can commit from end
 * alone; nothing is lost under the click slop. A drag past the slop (3 px
 * from the press) suppresses on_click. on_click/on_drag are nullable and
 * consumed. */
uint64_t kui_open_draggable(KuiCtx *ctx, KuiStr label, const KuiSpec *spec,
                            KuiValue *on_click, KuiValue *on_drag);
/* The general container: every message prop at once, each nullable and
 * consumed. NULL means absent (a NULL on_drag here does NOT make the node
 * draggable, unlike kui_open_draggable). A non-NULL on_key makes the node a
 * key sink: focus it with kui_set_key_focus and every press arrives as
 * {kind="key", phase="down", code, ctrl, alt, shift, super, text, repeat,
 * tag} - releases too, with phase="up", when KuiSpec.key_up is set. A non-NULL
 * on_hover makes the pointer entering/leaving emit
 * {kind="hover", phase="enter"|"leave", tag} — for hover-dependent layout;
 * plain hover colors belong in KuiSpec.hover_bg / pressed_bg. A tag of
 * kui_value_null() keeps the behaviour and leaves `tag` off the events.
 * Layout events come from KuiSpec.on_layout, not an argument. */
uint64_t kui_open_with(KuiCtx *ctx, KuiStr label, const KuiSpec *spec,
                       KuiValue *on_click, KuiValue *on_drag, KuiValue *on_key,
                       KuiValue *on_hover);
/* -- Keyboard focus (docs/adr/0002-keyboard-focus-as-data.md) ------------ */
/* One focus for every node: editors, on_key sinks, on_click nodes, the
 * control roles and `focusable` boxes are Tab stops in tree order; Enter
 * and Space press the focused control, the arrows nudge a focused slider
 * (increment / decrement access events), a key sink keeps every key (Tab
 * included) and hands focus on with kui_focus_next. Keyboard focus draws a
 * ring (or the node's focus_bg); a click's does not. */
/* Declares key focused this frame (0 blurs at once). Edge-triggered: the
 * node takes focus on the first frame it is declared, and a declaration
 * repeated every frame does not clobber a Tab press or a click. */
void kui_set_key_focus(KuiCtx *ctx, uint64_t key);
/* Moves focus to key now (0 blurs). */
void kui_focus(KuiCtx *ctx, uint64_t key);
/* The key of the node opened under label in the last finished frame (from
 * inside a view callback: this frame so far, then the last one); 0 for a
 * label no node declared. Keys hash the path from the root, through the
 * auto-keyed ancestors a host cannot spell, so a node no event has come
 * from is named this way: kui_focus(ctx, kui_key_of(ctx, KUI_STR("note"))).
 * Labels are unique among siblings, not across the tree: two nodes on one
 * label under different parents resolve to the first in tree order, with an
 * "ambiguous-key" warning (kui_take_warnings). A plugin asking from inside
 * its fill is answered from the nodes it opened and no one else's; the host
 * from its own first, and everyone's when it opened none. */
uint64_t kui_key_of(KuiCtx *ctx, KuiStr label);
/* What Tab (forward) / Shift-Tab does: the next / previous focusable node,
 * wrapping. */
void kui_focus_next(KuiCtx *ctx, bool forward);
/* The focused node's key, 0 for none; and whether the focus shows (it got
 * there by keyboard or assistive technology, not a click). */
uint64_t kui_focused(KuiCtx *ctx);
bool kui_focus_visible(KuiCtx *ctx);
/* The caret's blink (backlog C35). kui_caret_visible is the phase - true
 * draws it - which a custom editor reads in its view to skip its caret
 * node on the off phase, keeping the `caret` row on its KUI_ROLE_LINE
 * either way. Under kui_run the runner's clock sets it, while a focused
 * editor or such a line has a caret, and parks it hidden while the window
 * has no keyboard. A host driving its own window runs the clock itself:
 * armed while kui_has_caret, toggling kui_set_caret_visible each half
 * period, re-armed solid whenever kui_caret_stamp changes (the caret
 * moved, or focus did). Headless the phase stays true. */
bool kui_caret_visible(KuiCtx *ctx);
void kui_set_caret_visible(KuiCtx *ctx, bool visible);
bool kui_has_caret(KuiCtx *ctx);
uint64_t kui_caret_stamp(KuiCtx *ctx);
/* Enters the focus region key names - a node declared with focus_region -
 * or the main ring for 0 (docs/adr/0022-focus-regions.md): focus lands on
 * what that ring last held if the node is still there, else its
 * initial_focus, else its first stop, and shows. Deferred to the end of
 * the frame being built, like kui_focus_next from a view: call it from the
 * view callback after kui_open on the region (the frame that toggles a dock
 * on may enter it), or between frames to land on the next one. A key the
 * frame does not declare as a region raises "focus-region-without-node"
 * (kui_take_warnings) and moves nothing. */
void kui_focus_region(KuiCtx *ctx, uint64_t key);
/* The focus region in effect - the node whose ring Tab walks - or 0 for
 * the main ring. What a chord that toggles between a dock and the app
 * reads to know which way it is going. */
uint64_t kui_region(KuiCtx *ctx);
/* -- Scrolling ------------------------------------------------------------ */
/* Scroll offsets are retained per node key and clamped by each layout to
 * that frame's overflow. The wheel, the scrollbars, Tab and the caret move
 * them from inside; these move them from outside. */
/* Scrolls whatever contains key so the node shows — what Tab does to the
 * control it lands on, asked for by name. Already-visible nodes stay put.
 * The request resolves at the next kui_frame_finish, against the frame it
 * lays out: the one being built when called from inside a view callback,
 * the one after it otherwise (a frame is requested, so one comes). That is
 * what lets a view reveal a row it is declaring for the first time. If that
 * frame does not declare key, or nothing above it scrolls, it is a no-op —
 * the request is spent, not kept for a later frame. Last reveal before a
 * frame wins. */
void kui_reveal(KuiCtx *ctx, uint64_t key);
/* Sets a scroll container's offset the way the wheel would (positive =
 * content moved up / left); the next frame's layout clamps it, so 0,0 is
 * "jump to the top" and a huge y is "jump to the end" without knowing the
 * content height. Harmless for a key that never scrolls. */
void kui_set_scroll(KuiCtx *ctx, uint64_t key, float x, float y);
/* Reads it back as the last layout clamped it — the number to persist and
 * restore. 0,0 for a node that never scrolled; either pointer may be NULL. */
void kui_scroll_offset(KuiCtx *ctx, uint64_t key, float *x, float *y);
/* [out] The rect a node was laid out at (kui_layout_of): logical px in
 * viewport coordinates - the `layout` event's numbers without the event. */
typedef struct KuiLayoutRect {
    uint32_t size; /* = sizeof(KuiLayoutRect) in, bytes filled out */
    float x, y, w, h;
} KuiLayoutRect;
#define KUI_LAYOUT_RECT_INIT ((KuiLayoutRect){ .size = sizeof(KuiLayoutRect) })
/* The rect the last frame laid `key` out at, for a node that declared
 * on_layout - the same numbers its `layout` event carries, read back during
 * the next build with no event. False for any other key, a bad context, a
 * NULL out or a short reservation. Read during a build it describes the
 * previous frame, like kui_scroll_geometry. */
bool kui_layout_of(KuiCtx *ctx, uint64_t key, KuiLayoutRect *out);
/* Everything the last layout resolved for a container: its box, its content
 * size and that offset. False (leaving out untouched) for a key no layout
 * has resolved as a scroll container.
 *
 * This is what makes a long list affordable. The core builds every child a
 * view declares, so ten thousand rows cost ten thousand rows; a view that
 * knows h and offset_y declares the rows that fit plus two spacers holding
 * the space of the rest, and pays for a screenful. Read while building, it
 * describes the previous frame - so a resize slices one frame late, and a
 * row or two of overscan at each end covers it. */
bool kui_scroll_geometry(KuiCtx *ctx, uint64_t key, KuiScrollGeometry *out);
/* -- Fonts ---------------------------------------------------------------- */
/* Registers a font from file bytes (TTF/OTF/TTC, copied); returns a handle
 * for KuiTextStyle.font, 0 when the data holds no usable face. */
uint64_t kui_font_add(KuiCtx *ctx, const uint8_t *data, size_t len);
/* The handle for a font family by name ("Menlo") — installed, or loaded with
 * the two calls below; 0 when none matches. Idempotent per family. */
uint64_t kui_font_add_system(KuiCtx *ctx, KuiStr name);
/* The family names kui_font_add_system can take - every face this context
 * knows, installed or loaded, sorted - written into `out` up to `cap` and
 * the total returned, so a short array can be resized and the call
 * repeated (NULL `out` asks for the count alone). Strings are borrowed
 * until the next kui_font_families on the context. */
size_t kui_font_families(KuiCtx *ctx, KuiStr *out, size_t cap);
/* Registers a font file by path (memory-mapped); 0 on failure. */
uint64_t kui_font_load_file(KuiCtx *ctx, KuiStr path);
/* Loads every font file under a folder (recursively) for kui_font_add_system;
 * returns the number of faces added. */
size_t kui_font_load_dir(KuiCtx *ctx, KuiStr dir);
/* Forgets a font; styles still naming it shape as sans. */
void kui_font_remove(KuiCtx *ctx, uint64_t id);
/* -- Images --------------------------------------------------------------- */
/* Registers a w*h RGBA image (pixels copied); returns a handle, 0 on
 * failure. Handles are stable until kui_image_remove. */
uint64_t kui_image_add(KuiCtx *ctx, uint32_t w, uint32_t h, const uint8_t *rgba);
void kui_image_remove(KuiCtx *ctx, uint64_t id);
/* Replaces an image's pixels in place (copied): the handle is unchanged, so
 * every node showing it draws the new pixels next frame; w/h may differ
 * from the registration. From the first update on the image is drawn from
 * a texture of its own, as a KUI_QUAD_TEXTURE quad - a video frame, a
 * camera, a plot the host rasterised itself. A dead or foreign handle warns
 * `foreign-resource` and changes nothing.
 * (docs/adr/0025-the-image-is-the-canvas.md) */
void kui_image_update(KuiCtx *ctx, uint64_t id, uint32_t w, uint32_t h,
                      const uint8_t *rgba);
/* The pixels behind an image handle, for a host that renders the draw list
 * itself and meets a KUI_QUAD_TEXTURE quad: writes w, h and rgba (w*h*4
 * bytes) and returns true when the handle is live here. The bytes are
 * borrowed and valid until the next call of this function. */
bool kui_image_pixels(KuiCtx *ctx, uint64_t id, uint32_t *w, uint32_t *h,
                      const uint8_t **rgba);
/* -- Fragments ------------------------------------------------------------ */
/* Registers a WGSL fragment function for kui_fragment; returns a handle, 0
 * when the source does not compile (with a "fragment-rejected" warning
 * carrying the compiler's message in the app's own line numbers). The app
 * writes one function:
 *
 *   fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32>
 *
 * and the core wraps it in the prelude and epilogue that give it the node's
 * rounded box, the inherited clip, the group opacity and the blend.
 * Idempotent by source: the same text gets the same handle without being
 * validated twice. (docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md) */
uint64_t kui_fragment_add(KuiCtx *ctx, KuiStr wgsl);
void kui_fragment_remove(KuiCtx *ctx, uint64_t id);
/* The whole WGSL module behind a handle - the app's source between the
 * core's prelude and epilogue - which is what a renderer compiles. Writes
 * a borrowed pointer valid until the next call; false when the handle is
 * not live here. */
bool kui_fragment_source(KuiCtx *ctx, uint64_t id, KuiStr *out);
/* -- Sounds --------------------------------------------------------------- */
/* Registers a sound from its encoded file bytes (wav/ogg/mp3/flac, copied);
 * returns a handle for KuiSpec.click_sound / hover_sound, kui_audio and
 * kui_play; 0 when empty. */
uint64_t kui_sound_add(KuiCtx *ctx, const uint8_t *data, size_t len);
void kui_sound_remove(KuiCtx *ctx, uint64_t id);
/* Starts a playback; returns its id. opts may be NULL (defaults). A non-NULL
 * tag (consumed) asks for a {kind="sound", phase="ended", playback, tag}
 * event when the playback finishes on its own (never when stopped). */
uint64_t kui_play(KuiCtx *ctx, uint64_t sound, const KuiPlay *opts, KuiValue *tag);
void kui_stop(KuiCtx *ctx, uint64_t playback, float fade_ms);
void kui_set_volume(KuiCtx *ctx, uint64_t playback, float volume, float tween_ms);
void kui_pause(KuiCtx *ctx, uint64_t playback, float fade_ms);
void kui_resume(KuiCtx *ctx, uint64_t playback, float fade_ms);
void kui_set_master_volume(KuiCtx *ctx, float volume, float tween_ms);
/* An audio node: a playback retained by key while the frame declares it
 * (present = playing, once or looped; gone = stopped; volume/paused apply
 * live; a changed src restarts). Draws nothing. Empty label = a key from the
 * tree position. tag (nullable, consumed) rides the ended event. Returns the
 * node key the event carries. */
uint64_t kui_audio(KuiCtx *ctx, KuiStr label, const KuiAudio *spec, KuiValue *tag);
/* Only for hosts with their own audio device (kui_run needs neither): drains
 * queued commands into out (up to cap; the rest are dropped), returns the
 * count; report a playback that finished on its own with kui_audio_ended so
 * a tagged one becomes a sound event. */
size_t kui_take_audio_commands(KuiCtx *ctx, KuiAudioCommand *out, size_t cap);
void kui_audio_ended(KuiCtx *ctx, uint64_t playback);
/* The other two answers such a host owes (backlog F36), so its warnings
 * match the runner's: a KUI_AUDIO_STOP it drained that landed on a
 * playback still running, `at` seconds in - a one-shot audio node that
 * went away without `finish` is named in a "truncated-playback" warning,
 * any other stop reports nothing; and a KUI_AUDIO_PLAY its device refused
 * (voices all held, a sound that failed to decode) - the node that asked
 * is named in a "playback-refused" warning and a tagged playback, which
 * can now never end, gets a sound event with phase "refused" instead. */
void kui_audio_truncated(KuiCtx *ctx, uint64_t playback, double at);
void kui_audio_refused(KuiCtx *ctx, uint64_t playback);
/* An image node. Fit sizing = the image's pixel size as logical px; a Fit
 * height against a resolved width keeps the aspect; radius rounds corners. */
void kui_image(KuiCtx *ctx, uint64_t id, const KuiSpec *spec);
/* kui_image with its two rows (docs/adr/0025-the-image-is-the-canvas.md):
 * `sampling` is KUI_SAMPLING_* and `fit` KUI_FIT_*; 0 for either is the
 * default kui_image gives. The box - its layout, hit region and access
 * rect - is the same in every mode; only what is painted inside it moves. */
void kui_image_with(KuiCtx *ctx, uint64_t id, uint32_t sampling, uint32_t fit,
                    const KuiSpec *spec);
/* A box the registered WGSL `id` paints (kui_fragment_add). An ordinary node
 * otherwise: it lays out, rounds, clips, fades and takes input like a box.
 * It has NO intrinsic size, so spec must give it one. `params` is up to
 * sixteen floats, zero-padded; more are dropped with a
 * "fragment-params-truncated" warning. `params` may be NULL when count is 0,
 * and spec may be NULL.
 * (docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md) */
void kui_fragment(KuiCtx *ctx, uint64_t id, const float *params, size_t count,
                  const KuiSpec *spec);
/* kui_fragment as a parent: its children paint over it. Balance with
 * kui_close. An empty label is the unkeyed form. */
void kui_fragment_open(KuiCtx *ctx, KuiStr label, uint64_t id,
                       const float *params, size_t count, const KuiSpec *spec);
/* kui_fragment reading `image` - a handle from kui_image_add, 0 for none -
 * through the prelude's kui_sample(uv) / kui_sample_nearest(uv), with the
 * texel rect in FragmentIn::image (backlog V1, ADR 0025 decision 7). A
 * waveform, a heatmap, an image effect: the image is data the function
 * reads. An image that is not live draws nothing, as a dead `id` does.
 * label keys the node (empty = a key from the tree position). */
void kui_fragment_with(KuiCtx *ctx, KuiStr label, uint64_t id, uint64_t image,
                       const float *params, size_t count, const KuiSpec *spec);
/* kui_fragment_with as a parent: its children paint over it. Balance with
 * kui_close. */
void kui_fragment_open_with(KuiCtx *ctx, KuiStr label, uint64_t id,
                            uint64_t image, const float *params, size_t count,
                            const KuiSpec *spec);
/* A round-capped stroke from (x0, y0) to (x1, y1), in the parent's box space
 * (docs/adr/0010-a-segment-primitive.md). Never in layout: the node is a float
 * sized to the stroke's bounding box, so spec's sizing, padding and alignment
 * are ignored; what it keeps is transition/enter/exit (the colour eases),
 * opacity, on_layout (the bounding box), a label/role, and a declared float
 * anchor (KUI_FLOAT_VIEWPORT reads the points in viewport space). width is the
 * stroke width in logical px (<= 0: 1); color 0xRRGGBBAA (0: the default
 * foreground). spec may be NULL. For a stroke that takes input use
 * kui_polyline, which takes the payloads. */
void kui_line(KuiCtx *ctx, float x0, float y0, float x1, float y1, float width,
              uint32_t color, const KuiSpec *spec);
/* The same through `count` points (xy: x0, y0, x1, y1, ...; fewer than two draw
 * nothing): a polyline, or with `curve` a smooth curve through the points,
 * flattened in the core. Consecutive pieces overlap at their round caps.
 * label keys the node (empty = a key from the tree position) so a stroke can
 * transition or exit; kui_line is auto-keyed and takes no payloads. The
 * three payloads are taken as kui_open_with takes them: a stroke with one
 * is hit by its SHAPE (docs/adr/0026-hit-testing-by-shape.md) - a press
 * within half its width of any piece (at least 4 px of grab), and a press
 * elsewhere in its box falls through to what is under. NULL for none. */
void kui_polyline(KuiCtx *ctx, KuiStr label, const float *xy, size_t count,
                  float width, uint32_t color, bool curve, const KuiSpec *spec,
                  KuiValue *on_click, KuiValue *on_drag, KuiValue *on_hover);
/* A filled polygon through `count` points at xy (x0, y0, x1, y1, ...): at
 * most eight - more are dropped with `polygon-points-truncated`, fewer than
 * three draw nothing - the fill in spec->bg (no bg, no fill). Placed like a
 * stroke: a float sized to its own bounding box, in the parent's box space.
 * The three payloads are taken as kui_open_with takes them: a fill with one
 * is hit by its OUTLINE (docs/adr/0026-hit-testing-by-shape.md) - a press
 * inside it hits, a press in its box past the outline falls through - and
 * a clickable fill is a button to assistive technology, so name it. The
 * outline may be concave. On the wire it is one KUI_QUAD_FRAGMENT painted
 * by a WGSL function the core registers itself, reachable through
 * kui_fragment_source like any other. label keys the node (empty = a key
 * from the tree position); spec may be NULL; NULL for a payload is none.
 * (docs/adr/0025-the-image-is-the-canvas.md, decision 6) */
void kui_polygon(KuiCtx *ctx, KuiStr label, const float *xy, size_t count,
                 const KuiSpec *spec, KuiValue *on_click, KuiValue *on_drag,
                 KuiValue *on_hover);
void kui_close(KuiCtx *ctx);
void kui_text(KuiCtx *ctx, KuiStr text, const KuiTextStyle *style);
void kui_rich_text(KuiCtx *ctx, const KuiSpan *spans, size_t span_count,
                   const KuiTextStyle *base);
/* A terminal's screen as one node: rows x cols cells from `cells` (fewer draw
 * as blank), each shaped once per character and placed at col x cell_w ever
 * after, so a screen new every frame costs what a still one costs (~60 us
 * for 200 x 50). `style` sizes the cells (size, family/font, line_height as
 * the cell height); `spec` is the node's own - an on_key makes it the
 * terminal's sink, an on_click / on_drag carry cell: {row, col} on their
 * events - the payloads taken as kui_open_with takes them; label keys it
 * (empty = auto). cursor_shape is KUI_CELL_CURSOR_* or
 * 0 for none, at (cursor_row, cursor_col) in cursor_color. Its access role is
 * KUI_ROLE_TERMINAL, the rows joined as the value. */
void kui_cells(KuiCtx *ctx, KuiStr label, uint32_t rows, uint32_t cols,
               const KuiCell *cells, size_t count, const KuiTextStyle *style,
               const KuiSpec *spec, KuiValue *on_click, KuiValue *on_drag,
               KuiValue *on_key, uint32_t cursor_row, uint32_t cursor_col,
               uint32_t cursor_shape, uint32_t cursor_color,
               uint64_t origin_line);
uint64_t kui_child_key(KuiCtx *ctx, KuiStr label);
bool kui_is_hovered(KuiCtx *ctx, uint64_t key);
bool kui_is_pressed(KuiCtx *ctx, uint64_t key);
/* Whether dragged files are over `key` (ADR 0031), for drop-dependent
 * layout; the colour is KuiSpec.drop_bg. */
bool kui_is_drop_target(KuiCtx *ctx, uint64_t key);
/* The drop zone the dragged files are over, or 0 - what the host answers
 * the OS with after each kui_input_drag_files. */
uint64_t kui_drop_target(KuiCtx *ctx);
/* -- Measurement ---------------------------------------------------------- */
/* Measures text the way layout would, without adding a node: unwrapped with
 * max_w <= 0, else wrapped to max_w logical px; the style's wrap /
 * max_lines / ellipsis apply. Works before the first frame (at scale 1).
 * Size a column to its widest label, or pick the tier that fits, from these
 * numbers instead of constants. */
bool kui_measure_text(KuiCtx *ctx, KuiStr text, const KuiTextStyle *style,
                      float max_w, KuiTextMetrics *out);
bool kui_measure_rich_text(KuiCtx *ctx, const KuiSpan *spans, size_t span_count,
                           const KuiTextStyle *base, float max_w, KuiTextMetrics *out);
/* Where a point (logical viewport px, as a click or drag event carries it)
 * lands in the text the node `key` drew: a byte offset into that text and
 * the visual line, so a custom editor turns the event into a caret position
 * with one call instead of measuring prefixes or assuming a cell width. A
 * node holding several text runs (a `line` row of token runs) answers across
 * them in order. Answered from the frame that finished - the layout the
 * pointer was over. False for a key that drew no text. */
bool kui_text_hit(KuiCtx *ctx, uint64_t key, float x, float y, KuiTextHit *out);
/* Opens a context menu at (x, y) over `key`, with `count` items read from
 * `items`; the next frame draws it. Choosing a row posts {kind:"menu",
 * role, item} on `key` and closes the menu, and a press outside it or
 * Escape closes it with nothing posted. False - and nothing opens - for a
 * key of 0, no items, or an item whose role this build does not know. */
bool kui_open_menu(KuiCtx *ctx, uint64_t key, float x, float y,
                   const KuiMenuItem *items, size_t count);
/* Closes whatever menu is open; true when there was one. */
bool kui_close_menu(KuiCtx *ctx);

/* The application menu for this frame: `count` menus in bar order,
 * declared and - where the platform has no menu bar of its own - drawn
 * right here as a row of titles that drop their menus
 * (docs/adr/0018-a-menu-bar-the-app-declares.md). One call and not two,
 * because what the menu is and where its strip goes are one decision.
 * Where the platform owns the bar (kui_set_native_menu_bar) nothing is
 * drawn and the declaration still stands, so calling this unconditionally
 * is what a portable view does. Sticky and diffed, like kui_window_title:
 * a frame that does not call it leaves the last declaration in force, the
 * same one again changes nothing, and `count` of 0 takes the menu away.
 * False - declaring and drawing nothing - for a row whose role this build
 * does not know. Between kui_frame_begin and kui_frame_finish. */
bool kui_menu_bar(KuiCtx *ctx, const KuiMenu *menus, size_t count);
/* Tells the core the platform owns the menu bar, so kui_menu_bar draws
 * nothing and you are the one handing the declaration over (read it back
 * with the two calls below) and reporting what was chosen. Off by
 * default. */
void kui_set_native_menu_bar(KuiCtx *ctx, bool on);
/* How many menus the declaration in force has, writing its revision into
 * *revision when that is not NULL. The revision changes only when the
 * declaration does, so a host with a native bar rebuilds nothing until it
 * moves. */
size_t kui_menu_bar_menu_count(KuiCtx *ctx, uint64_t *revision);
/* Reads one menu back: its title into *label and its enabled state into
 * *enabled (either may be NULL), returning how many rows it has. Zero for
 * a menu past the end. *label is borrowed until the next call on this
 * context. */
size_t kui_menu_bar_menu(KuiCtx *ctx, size_t menu, KuiStr *label, bool *enabled);
/* Reads one row: its text into *label, its accelerator into *accel (empty
 * when it has none), its KUI_MENU_* role into *role and its
 * KUI_MENU_ITEM_* flags into *flags. Any out pointer may be NULL; both
 * strings are borrowed until the next call on this context. False for a
 * row that is not there. */
bool kui_menu_bar_item(KuiCtx *ctx, size_t menu, size_t item, KuiStr *label,
                       KuiStr *accel, uint32_t *role, uint32_t *flags);
/* Reports that the platform's menu bar chose row `item` of menu `menu`:
 * the same path a press on the drawn bar's row takes. Out of range does
 * nothing and returns false. */
bool kui_activate_menu_bar_item(KuiCtx *ctx, size_t menu, size_t item);

/* The window's selected text - a `selectable` scope's, a `cells` grid's,
 * or the focused editor's, whichever it holds. False when nothing is
 * selected; *out is borrowed until the next selection call. */
bool kui_selection_text(KuiCtx *ctx, KuiStr *out);
/* The same selection with the formatting the text declared - bold, italic,
 * a span's own colour - for a host offering a second clipboard flavour.
 * Never a replacement for kui_selection_text
 * (docs/adr/0017-selection-as-a-scope.md). */
bool kui_selection_html(KuiCtx *ctx, KuiStr *out);
/* The text selection's two ends as the drag made them - the anchor where
 * the press landed, the focus where the pointer is - each as the data
 * index of the virtualised row it is in (-1 outside every virtualised
 * row) and the byte inside that row's own text. Directed, so a
 * Shift-press that kept the anchor reads as one
 * (docs/adr/0029-a-selection-follows-the-pointer-past-the-edge.md).
 * False with no text selection; a grid's is kui_cell_selection. Any out
 * pointer may be NULL. */
bool kui_selection_ends(KuiCtx *ctx, int64_t *anchor_index, size_t *anchor_byte,
                        int64_t *focus_index, size_t *focus_byte);
/* A cells grid's selection, the window's when it lives in one: the grid's
 * key, the anchor and the focus as the drag made them - each an absolute
 * line (origin_line plus the row, so a scroll does not move it) and a
 * column - and whether it is a block rather than linewise
 * (docs/adr/0017-selection-as-a-scope.md, decision 4). False when the
 * window's selection is not a grid's; a text selection's ends are
 * kui_selection_ends. Any out pointer may be NULL. */
bool kui_cell_selection(KuiCtx *ctx, uint64_t *node, uint64_t *anchor_line,
                        size_t *anchor_col, uint64_t *focus_line, size_t *focus_col,
                        bool *block);
/* Selects everything in the scope `key` declared: every run of a
 * `selectable` container, or the whole screen of a `cells` grid. */
bool kui_select_all_in(KuiCtx *ctx, uint64_t key);
/* Drops the window's selection, whichever kind; true when there was one. */
bool kui_clear_selection(KuiCtx *ctx);

/* kui_request_copy's answer. */
enum { KUI_COPY_READY = 0, KUI_COPY_ASKED = 1, KUI_COPY_NOTHING = 2 };
/* Asks for the selection as text
 * (docs/adr/0017-selection-as-a-scope.md). KUI_COPY_READY writes it into
 * *out, borrowed until the next call on this context. KUI_COPY_ASKED means
 * the selection reaches rows a virtual list never built: a
 * {kind:"selectionrange", from:{index, byte}, to:{index, byte}} event is
 * in the queue, the rows behind that gap are yours, and answering with
 * kui_answer_selection_range hands the text back as a
 * KUI_MENU_ACTION_SET_CLIPBOARD. */
uint32_t kui_request_copy(KuiCtx *ctx, KuiStr *out);
/* Answers that ask with the text for the range it named, whole. False when
 * nothing asked - a late answer cannot overwrite what has been copied
 * since. */
bool kui_answer_selection_range(KuiCtx *ctx, KuiStr text);
/* The clipboard for an app that owns its text (backlog C33): an on_key sink
 * hears the raw Ctrl-c / Ctrl-v and binds them here. kui_set_clipboard
 * queues a KUI_MENU_ACTION_SET_CLIPBOARD with `text` (and `html` as a
 * second flavour beside it - empty for none, never instead of it);
 * kui_request_paste queues a KUI_MENU_ACTION_PASTE, which the host answers
 * with kui_input_commit so the text reaches the focused editor as typing
 * or the focused sink as {kind:"text"}. Under kui_run the runner drains
 * both after every input and every frame; a host driving its own window
 * drains them with kui_take_menu_action as it does a menu's. */
void kui_set_clipboard(KuiCtx *ctx, KuiStr text, KuiStr html);
void kui_request_paste(KuiCtx *ctx);
/* One paste ask at a time: while one is unanswered a second
 * kui_request_paste is dropped, and the kui_input_commit that answers it -
 * send an empty one when the clipboard held nothing - lets the next
 * through. This reads whether one is out (backlog AR34). */
bool kui_awaiting_paste(KuiCtx *ctx);
/* Tells the core this host shows menus itself, however the platform draws
 * them: the core then keeps the open menu as state and draws none of it.
 * Read what is open, show it, and report back with kui_activate_menu_item
 * or kui_close_menu. Off by default, which is the menu this library
 * draws. */
void kui_set_native_menus(KuiCtx *ctx, bool on);
/* What is open: how many rows the open menu has, writing the node it is
 * about into *target and where it opened (logical viewport px) into *x /
 * *y - any of the three may be NULL. 0 when no menu is open, which is
 * unambiguous because a menu never opens with no rows. */
size_t kui_menu_item_count(KuiCtx *ctx, uint64_t *target, float *x, float *y);
/* Reads one row of the open menu, spelled exactly as kui_menu_bar_item
 * spells a bar's: its text into *label, its accelerator into *accel (the
 * role's own where the row declared none, empty where there is neither),
 * its KUI_MENU_* role into *role and its KUI_MENU_ITEM_* flags into
 * *flags. Any out pointer may be NULL; both strings are borrowed until
 * the next call on this context. False for a row that is not there. */
bool kui_menu_item(KuiCtx *ctx, size_t item, KuiStr *label, KuiStr *accel,
                   uint32_t *role, uint32_t *flags);
/* Reports that the host's own menu chose row `index` - the same path a
 * press on the drawn menu's row takes; an index past the end closes the
 * menu and posts nothing. False when no menu was open. */
bool kui_activate_menu_item(KuiCtx *ctx, size_t index);
/* Tells the core this host can show the platform's definition panel
 * (macOS's Look Up). The standard Look Up row is then offered where it
 * means something and a force click over text asks for one; without it the
 * core neither offers nor asks. */
void kui_set_lookup_available(KuiCtx *ctx, bool on);
/* Drains one queued menu action (see KuiMenuAction); false when there are
 * none. Drain to empty after handling input, the way window commands are. */
bool kui_take_menu_action(KuiCtx *ctx, KuiMenuAction *out);
/* The caret rect for byte offset `byte` in that text: where a caret, a
 * selection edge or an IME candidate window goes. A byte past the text is
 * the end. False for a key that drew no text. */
bool kui_caret_rect(KuiCtx *ctx, uint64_t key, size_t byte, KuiCaretRect *out);
/* Where the OS candidate window goes while a composition is under way: the
 * focused editor's caret, or a custom editor's `line` carrying `caret`.
 * False when nothing with a caret is focused. A host driving its own window
 * reads it after each frame and hands it to the platform. */
bool kui_ime_rect(KuiCtx *ctx, KuiCaretRect *out);
/* -- Diagnostics ---------------------------------------------------------- */
/* Drains the warnings the core raised since the last call into out (up to
 * cap; the rest wait), returns the count. Each distinct (code, node) pair
 * is raised once. A host driving kui_frame_* drains them here, a test
 * asserts on them; kui_run prints them to stderr by itself in debug builds. */
size_t kui_take_warnings(KuiCtx *ctx, KuiWarning *out, size_t cap);
/* A standalone context starts with the checks OFF — a development build
 * turns them on; off costs nothing per frame. */
void kui_set_diagnostics(KuiCtx *ctx, bool on);
/* -- Devtools (docs/adr/0024-the-devtools-are-the-cores.md) --------------- */
/* The core's devtools panel: the event stream, the runtime's facts and the
 * tree, drawn by the core beside the host's tree in the main window and
 * acted on inside kui_input, so nothing of it reaches the host's events.
 * KUI_DEVTOOLS=1 in the environment is the same call made by nobody, for a
 * window kui_run opens; a headless context never reads it. */
void kui_set_devtools(KuiCtx *ctx, bool on);
/* Where it sits: "left", "right", "bottom", "window" (one of its own, named
 * kui-devtools, opened through the ordinary KUI_CMD_OPEN — the host builds
 * nothing into it, and a KUI_CMD_REDRAW names it when it should be drawn
 * again) or "off" (hidden, the chords still live). False for any other
 * word. */
bool kui_set_devtools_dock(KuiCtx *ctx, KuiStr dock);
/* Whether the panel is on. */
bool kui_devtools(KuiCtx *ctx);
/* Where it sits, as the word kui_set_devtools_dock takes ("right" for the
 * side); the string is static. False on a bad context. */
bool kui_devtools_dock(KuiCtx *ctx, KuiStr *out);
/* Seeds the panel's theme override, what its T and A chords cycle from:
 * base is "light", "dark" or empty for the app's own; accent a 0xRRGGBBAA
 * colour, or 0 for none. False for any other base word. */
bool kui_set_devtools_theme(KuiCtx *ctx, KuiStr base, uint32_t accent);
/* Respells the chord that moves the keyboard into the panel and back out
 * (and brings a hidden panel back) from its default "ctrl+shift+i":
 * "f12", "mod+shift+d" (mod is Command on macOS, Control elsewhere),
 * "⌥⌘I", any spelling a KuiMenuItem's accel takes. The panel's other
 * chords stay Ctrl+Shift+<letter>; with another chord set, Ctrl+Shift+I
 * reaches the host like any other press. False for a spelling kui cannot
 * name, which leaves the chord as it was. */
bool kui_set_devtools_key(KuiCtx *ctx, KuiStr key);
/* That chord, or the default, in its portable spelling ("ctrl+shift+i",
 * "f12", "super+alt+d"); borrowed until the next call. False on a bad
 * context. */
bool kui_devtools_key(KuiCtx *ctx, KuiStr *out);
/* A tab in the panel (docs/adr/0032-a-devtools-tab-mounts-a-slot.md), in
 * one of two forms. kui_devtools_tab declares one an extension fills: name
 * is the tab's identity, label what the strip shows, slot the full
 * namespace/slot the extension names - while the tab is on show the panel
 * declares that slot in the tab's body and the fill is drawn there, and
 * otherwise the extension is not asked (and naming the slot raises no
 * unknown-slot). kui_devtools_tab_open declares one the host draws itself
 * and opens its content ONLY while the tab is on show: true means the
 * content node is open - build inside and kui_close - and false means the
 * tab was declared and nothing was opened, so skip the body and do not
 * close:
 *
 *     if (kui_devtools_tab_open(ctx, KUI_STR("syntax"), KUI_STR("Tree-sitter"))) {
 *         ...
 *         kui_close(ctx);
 *     }
 *
 * What the host builds is its own - its keys, its events - laid out and
 * painted as a layer over the panel's tab body, clipped to it, in the
 * dock's focus region. Both forms are made every frame, panel on or off;
 * a name declared twice in a frame warns duplicate-tab and keeps the
 * first, which is what a false from either says. */
bool kui_devtools_tab(KuiCtx *ctx, KuiStr name, KuiStr label, KuiStr slot);
bool kui_devtools_tab_open(KuiCtx *ctx, KuiStr name, KuiStr label);
/* The panel's facts a tab reads (ADR 0032, decision 4): the node the tree
 * tab has selected, the tree row under the pointer, the node the picker is
 * over - each as a key, 0 for none. kui_set_devtools_selected selects (and
 * reveals) a node in the tree tab from outside it; 0 clears. */
uint64_t kui_devtools_selected(KuiCtx *ctx);
uint64_t kui_devtools_hovered(KuiCtx *ctx);
uint64_t kui_devtools_picked(KuiCtx *ctx);
void kui_set_devtools_selected(KuiCtx *ctx, uint64_t key);
/* Raises the panel's picker from outside it (an inspector in a declared tab
 * asking "which node?") or puts it away: the node under the pointer is
 * kui_devtools_picked while it is up, and the press lands it in
 * kui_devtools_selected. Raised while a declared tab is on show, the pick
 * leaves that tab up; raised otherwise it is the Ctrl+Shift+P pick and
 * shows the tree tab. kui_devtools_picking says whether it is up. */
void kui_set_devtools_pick(KuiCtx *ctx, bool on);
bool kui_devtools_picking(KuiCtx *ctx);
/* Shows the panel's tab named name from the host's side - what the strip's
 * click and Ctrl+Shift+N do, for a command that jumps to the host's own
 * tab: one of the panel's own ("facts", "events", "tree") or a declared
 * tab's. A declared name the panel does not list yet is kept and shows
 * once a frame declares it; the return says whether the panel lists it
 * now. A hidden panel comes back docked; kui_set_devtools is still the
 * host's to call. Once, not every frame: called each frame it would pin
 * the strip against the user's own clicks. kui_devtools_current_tab reads
 * the tab the panel is on, by the same names; borrowed until the next
 * call. */
bool kui_set_devtools_tab(KuiCtx *ctx, KuiStr name);
bool kui_devtools_current_tab(KuiCtx *ctx, KuiStr *out);
/* The key legend the panel's facts tab shows: count pairs, the keys in
 * keys and what each does in what, index for index. */
void kui_set_devtools_legend(KuiCtx *ctx, const KuiStr *keys, const KuiStr *what,
                             size_t count);
/* Turns the per-frame node snapshot behind kui_nodes on or off (off unless
 * a devtool asked: the copy is O(nodes) a frame). */
void kui_set_inspect(KuiCtx *ctx, bool on);
/* The last finished frame's nodes in tree order, as a list of maps - each
 * with key, parent, depth, kind, label, rect, role, text, flags, layer,
 * origin, children, the layout spec and events (the node's own payloads
 * by handler name) - what a tree view and a node inspector are built
 * from; read it with kui_value_at / kui_value_get. Rects are in your
 * viewport's logical px, like kui_layout_of's (backlog AR36). Empty until
 * kui_set_inspect(ctx, true) and a frame after it. Borrowed until the
 * next call; NULL on a bad context. */
const KuiValue *kui_nodes(KuiCtx *ctx);
/* -- Accessibility (docs/adr/0001-accessibility-as-data.md) ---------------- */
/* The access tree of the last finished frame: fills out with up to cap
 * nodes in tree order (root first) and returns the total count, so a short
 * buffer can be resized and the call repeated. A host wiring its own
 * platform accessibility layer reads it after each frame; a test asserts
 * on it. Never asking costs nothing. */
size_t kui_access_tree(KuiCtx *ctx, KuiAccessNode *out, size_t cap);
/* Says something once, with no node behind it: "Saved", "3 results".
 * `live` is KUI_LIVE_POLITE or KUI_LIVE_ASSERTIVE; KUI_LIVE_OFF and an
 * empty text are both no-ops, the first so a caller can gate politeness
 * without a branch. A region whose message is on screen is KuiSpec.live
 * instead. Call it where the event is handled: called from a frame builder
 * it fires every frame, which the core reports as "announcement-repeated".
 * See docs/adr/0008-live-regions-and-announcements.md. */
void kui_announce(KuiCtx *ctx, KuiStr text, uint32_t live);
/* Drains queued announcements into out (up to cap; the rest are dropped, so
 * size it generously) and returns the count. Drain every frame whether or
 * not assistive technology is attached and discard what you cannot deliver
 * — an announcement kept is an announcement said minutes late. kui_run
 * does this itself. */
size_t kui_take_announcements(KuiCtx *ctx, KuiAnnouncement *out, size_t cap);
/* A request from assistive technology on a node: one KUI_ACCESS_* bit the
 * node advertises, with value the new text for KUI_ACCESS_SET_VALUE (empty
 * otherwise). Resolved like its pointer/keyboard equivalent: a click emits
 * the node's payload, focus lands on an editor, a slider nudge arrives as
 * a {kind="access", action, tag} event. */
void kui_input_access(KuiCtx *ctx, uint64_t key, uint32_t action, KuiStr value);
/* The laid-out text of editor node key as runs (KuiAccessRun): fills out
 * with up to cap of them, returns the total. */
size_t kui_access_runs(KuiCtx *ctx, uint64_t key, KuiAccessRun *out, size_t cap);
/* A text request on an editor: KUI_ACCESS_SET_TEXT_SELECTION with the
 * selection as run positions (anchor the end that stays, focus the caret),
 * or KUI_ACCESS_REPLACE_SELECTED_TEXT / KUI_ACCESS_SET_VALUE with value. A
 * built-in editor applies it (a changed event follows an edit); a custom
 * editor gets it as a {kind="access", action, anchor={line, offset},
 * focus={line, offset}, text, tag} event to apply itself. */
void kui_input_access_text(KuiCtx *ctx, uint64_t key, uint32_t action,
                           uint64_t anchor_run, uint32_t anchor_char,
                           uint64_t focus_run, uint32_t focus_char, KuiStr value);
/* Styled button with hover/press states; payload consumed (may be NULL). */
void kui_button(KuiCtx *ctx, KuiStr label, KuiValue *payload);
/* kui_button with the rows the stock button admits read off spec - label
 * (the accessible name, when the text is not it), description, tooltip
 * (its description, and the hint floated while hovered) and disabled
 * (inert, and dimmed to half) - and every other field of spec ignored: the
 * button's look is its own (kui_core::widgets::button_spec), and a zeroed
 * KuiSpec is the schema default rather than "unset", so there is nothing
 * to merge; a button that needs another row is kui_open_keyed with a role.
 * Keyed by text. spec may be NULL (then this is kui_button); payload
 * consumed (may be NULL). Appended after alpha.8 - a new function, no
 * KUI_ABI_VERSION bump (see the note above KuiSpec). */
void kui_button_with(KuiCtx *ctx, KuiStr text, const KuiSpec *spec, KuiValue *payload);
/* -- Widgets (the same kui_core::widgets every frontend uses) ------------ */
/* Body callbacks build content through the same ctx (see KuiViewFn). */
typedef void (*KuiViewFn)(void *user, KuiCtx *ctx);
/* Adaptive titlebar: drag strip, title, window buttons per the env facts. */
void kui_titlebar(KuiCtx *ctx, KuiStr title);
/* Titlebar hosting custom content built by body (tabs, search, ...). */
void kui_titlebar_with(KuiCtx *ctx, KuiViewFn body, void *user);
/* Min/max/close cluster; draws nothing when the OS provides controls. */
void kui_window_buttons(KuiCtx *ctx);
/* Hint floated below the enclosing node; gate on kui_is_hovered. */
void kui_tooltip(KuiCtx *ctx, KuiStr text);
void kui_tooltip_with(KuiCtx *ctx, KuiViewFn body, void *user);
/* Per-phase frame-latency bars (populated by kui_run; empty headless). */
void kui_latency_graph(KuiCtx *ctx);
/* The graph in a corner panel; x/y are KUI_START/CENTER/END. */
void kui_latency_hud(KuiCtx *ctx, uint32_t x, uint32_t y);
/* Single-line input with chrome; returns the editor key (kui_edit_text). */
uint64_t kui_text_input(KuiCtx *ctx, KuiStr label, KuiStr initial);
/* The stock select: a field showing the `current`th of `count` options
 * (-1 for none) that, clicked, opens the core's own menu of them under
 * it - the rows are the ones kui_open_menu takes, the current one drawn
 * checked. The host holds no open state; the choice arrives as the
 * {kind:"menu", role, item} event a menu row posts, on the key this
 * returns, and drawing the field again with the new `current` is the
 * whole loop. 0 for no label, no items, or a role this build does not
 * know. */
uint64_t kui_select(KuiCtx *ctx, KuiStr label, const KuiMenuItem *items, size_t count,
                    int64_t current);
/* Editable text node (state retained by key). Returns the node key;
 * "changed"/"submit" events arrive via kui_poll_event with that key. */
uint64_t kui_text_edit(KuiCtx *ctx, KuiStr label, KuiStr initial,
                       const KuiTextStyle *style, uint32_t flags, const KuiSpec *spec);
/* Borrowed view of an editor's text; valid until the next kui_edit_text call. */
bool kui_edit_text(KuiCtx *ctx, uint64_t key, KuiStr *out);
/* Replaces an editor's text, caret left at the end. Reaches an editor that
 * does not exist yet: the text is held for the frame that declares `key`
 * and seeds it there, over the `initial` that frame passes - held for that
 * one frame, so a key nothing declares on it drops its text with an
 * "edit-text-without-editor" warning. */
void kui_edit_set_text(KuiCtx *ctx, uint64_t key, KuiStr text);
/* The same by the label kui_text_edit / kui_text_input declares, for the
 * host that has no key: a key comes from an event the node fired, and an
 * editor opening for the first time has fired none. A declared label is
 * applied at once; one no frame has declared is held for the frame that
 * declares it, seeding a new editor over `initial` and replacing a
 * retained one's draft. A new function, so no ABI bump. */
void kui_edit_set_text_label(KuiCtx *ctx, KuiStr label, KuiStr text);
bool kui_is_focused(KuiCtx *ctx, uint64_t key);
void kui_frame_finish(KuiCtx *ctx);
/* Pointers valid until the next kui_frame_begin on this context. False -
 * writing nothing - for a NULL out or one whose `size` says it predates
 * this library; it returned void before that check existed, so a host that
 * ignores the result still compiles. */
bool kui_draw_data(KuiCtx *ctx, KuiDrawData *out);

/* -- Values -------------------------------------------------------------- */
/* A payload is one of seven shapes - null, bool, integer, float, string,
 * list, map - and an event carries every one of them: a key event's
 * `shift` is a bool, a drag's `dx` a float, a preedit's `cursor` a list
 * of two integers, its `tag` whatever the view declared. Build one with
 * the constructors and read one with the `as_*` readers, each of which
 * answers false for a shape it is not, so a field's absence and a
 * field's shape are both something a host can branch on. */
KuiValue *kui_value_null(void);
KuiValue *kui_value_bool(bool v);
KuiValue *kui_value_int(int64_t v);
KuiValue *kui_value_float(double v);
KuiValue *kui_value_str(KuiStr s);
KuiValue *kui_value_map(void);
KuiValue *kui_value_list(void);
void kui_value_map_set(KuiValue *map, KuiStr key, KuiValue *val); /* consumes val */
void kui_value_list_push(KuiValue *list, KuiValue *val);          /* consumes val */
const KuiValue *kui_value_get(const KuiValue *v, KuiStr key);     /* borrowed */
/* Entry `i` of a list (borrowed; NULL past the end), and the `i`th (key,
 * value) of a map for walking one whose keys you do not know. */
const KuiValue *kui_value_at(const KuiValue *v, size_t i);
const KuiValue *kui_value_entry(const KuiValue *v, size_t i, KuiStr *key);
size_t kui_value_len(const KuiValue *v); /* a list's or map's entries; 0 otherwise */
/* True for the null value and for a NULL pointer alike, so a missing key
 * and an explicit null read the same. */
bool kui_value_is_null(const KuiValue *v);
bool kui_value_as_bool(const KuiValue *v, bool *out);
/* An integer as itself, a float truncated; the reader for a count, an
 * index, a byte offset. */
bool kui_value_as_int(const KuiValue *v, int64_t *out);
/* A float as itself, an integer widened; the reader for anything in
 * pixels - a drag's x/dx, a layout's rect, a resize's scale. */
bool kui_value_as_float(const KuiValue *v, double *out);
bool kui_value_as_str(const KuiValue *v, KuiStr *out);            /* borrowed */
void kui_value_free(KuiValue *v);

/* -- Windowed runner (winit + wgpu), blocks until the window closes ------ */
/* These two are the library's `runner` feature, on by default and the only
 * thing in it. They are also the only two entry points that need
 * the GUI runtime - winit, wgpu, kira, accesskit - and it is most of the
 * library's size: the release cdylib is 12.3 MB with them and 5.1 MB
 * without (x86_64-pc-windows-msvc). A host that already has a window and
 * draws the display list itself builds
 *
 *     cargo build -p kui-ffi --no-default-features --release
 *
 * and these two are simply absent - a link error naming them, not a stub
 * that fails at run time. Everything else in this header is always there. */
typedef void (*KuiEventFn)(void *user, const KuiEvent *ev);
bool kui_run(KuiStr title, KuiViewFn view, KuiEventFn on_event, void *user);

/* What kui_run's window opens as: the chrome, the antialiasing and the
 * diagnostics words KuiRunConfig takes. Zero is the default of each. */
enum { KUI_CHROME_NATIVE = 0, KUI_CHROME_CUSTOM = 1, KUI_CHROME_BORDERLESS = 2 };
enum { KUI_TEXT_AA_AUTO = 0, KUI_TEXT_AA_GRAYSCALE = 1, KUI_TEXT_AA_SUBPIXEL = 2 };
enum { KUI_DIAG_DEFAULT = 0, KUI_DIAG_ON = 1, KUI_DIAG_OFF = 2 };

/* [in] How kui_run_with opens its window: the options a Rust host's
 * Launcher has and Node's WindowOptions carry, as one struct. Read
 * literally, so start from KUI_RUN_CONFIG_INIT (every zero) or pass NULL
 * for exactly that: a 960x640 native window, unbounded, antialiasing
 * chosen by the GPU, diagnostics as the build has them. A word this
 * build does not have - a chrome past KUI_CHROME_BORDERLESS, a size that
 * is not a size - makes kui_run_with return false before any window
 * opens, with the reason on stderr, rather than open something else. */
typedef struct KuiRunConfig {
    float width, height;   /* initial inner size, logical px; 0,0 = 960x640;
                            * both or neither. Clamped into the bounds
                            * below; KUI_WINDOW=WxH still overrides. */
    float min_w, min_h;    /* smallest inner size the user may resize to;
                            * a 0 side is unbounded, so one side may stand */
    float max_w, max_h;    /* largest; a max below its min loses to it */
    uint32_t chrome;       /* KUI_CHROME_*: with KUI_CHROME_CUSTOM the view
                            * draws kui_titlebar and the runner synthesizes
                            * edge resizing and double-click maximize */
    uint32_t text_aa;      /* KUI_TEXT_AA_*; KUI_TEXT_AA=gray|subpixel in
                            * the environment still overrides */
    uint32_t diagnostics;  /* KUI_DIAG_*: the window's, over what
                            * kui_set_diagnostics set on the context; the
                            * default is the build's (debug on, release off) */
} KuiRunConfig;
#define KUI_RUN_CONFIG_INIT ((KuiRunConfig){0})

/* kui_run with a window of your choosing and the context's registrations
 * (backlog AR27; ABI 16). `config` is the window, NULL for every default.
 * `ctx`'s core becomes the window's: the fonts, images, sounds, tokens,
 * theme, devtools doors, kui_set_native_menus and text-cache budget you
 * registered on it before the call reach the window, and the handles you
 * minted keep drawing there - register, then run, in the order a Node
 * host does. The context is left with a fresh core and no extensions,
 * still yours to free. NULL for both is kui_run.
 *
 * The extensions come along the same way (ADR 0014): make a context,
 * kui_ctx_add_extension each plugin into it - kui_ctx_extension_error says
 * why one was refused, before any window opens - and hand it here. Your
 * view declares slots with kui_slot exactly as it would headless, and
 * what a plugin's nodes produce reaches your on_event as replies carrying
 * that plugin's origin.
 *
 * The context's env.system comes along too, as the window's pin: whatever
 * you pushed with kui_env_set_system before handing the context here is
 * laid over the OS's reading before every frame, for the life of the
 * window. kui_env_set_system(ctx, 0, 0, KUI_MOTION_REDUCED, empty) and then
 * kui_run_with(ctx, ...) opens the window as a user who asked for less
 * motion sees it, on a machine whose owner did not; the zero (unknown)
 * fields are not pinned and keep following the OS, whose changes to them
 * still arrive as the `system` event, carrying the pin. A context never
 * told anything pins nothing, so this changes nothing for a host that only
 * loads plugins into it. It is the launcher's option and not an
 * environment variable on purpose: a shipped app's motion is its own code's
 * decision. */
bool kui_run_with(KuiCtx *ctx, KuiStr title, const KuiRunConfig *config,
                  KuiViewFn view, KuiEventFn on_event, void *user);

/* macOS: whether holding a letter key opens the accent picker (the
 * platform's press-and-hold, on unless the user turned it off) or repeats
 * the key, as every other platform does. With it on, a held e offers
 * "é è ê" and a held j does nothing at all - so a host whose keys are
 * commands (a modal editor, where j held is a motion) passes false before
 * kui_run; one that is typed into leaves it, the picker being how its
 * users write accents. This process alone, never written to the user's
 * preferences; a no-op on every other platform, where a held key repeats
 * already. */
void kui_press_and_hold(bool on);

/* -- Extension ABI: C as the guest rather than the host ------------------
 *
 * The other direction from everything above. A host that already owns the
 * window - a Rust app, or anything else driving a Core - loads a shared
 * library and gives it a share of each frame: it draws into the host's tree,
 * keeps its own state, and gets back the events its own nodes emitted and
 * no others. Same deal a Lua extension gets
 * (examples/lua/features/slots/panel.lua), and the loader on the host's side is
 * kui_ffi::CExtension.
 *
 * YOU define these six; the library only calls them. Two are required -
 * kui_ext_abi and kui_ext_view - and the host refuses to load a plugin
 * without either. The other four are optional, and a missing one is not an
 * error:
 *
 *   kui_ext_abi      REQUIRED. Your KUI_ABI_VERSION. The host refuses a
 *                    mismatch, which is the check a C host makes for itself
 *                    against kui_abi_version(), and it refuses absence the
 *                    same way: a plugin built against a header from before
 *                    this symbol existed is exactly the mismatched plugin
 *                    the check is for, so "absent" cannot mean "unchecked".
 *   kui_ext_name     A name for logs; a NUL-terminated static string. Absent
 *                    = the library's file stem.
 *   kui_ext_init     Your state, handed back to every call below. Absent =
 *                    NULL, which is fine for a stateless panel.
 *   kui_ext_view     REQUIRED. Called once per frame per slot you fill,
 *                    with a context borrowing the host's frame. Call the
 *                    kui_open / kui_text / kui_close builders on it; the
 *                    nodes are tagged with the origin the host assigned you
 *                    and keyed under the slot. It is alive for that one
 *                    call only - store nothing - and the input, frame and
 *                    draw entry points do not apply to it, since the host
 *                    drives those. kui_slot_name says which slot this is
 *                    and kui_slot_params what the host passed with it.
 *   kui_ext_slots    The slots you fill: a pointer to an array of KuiStr
 *                    you keep alive for as long as you are loaded, and its
 *                    count through the out-pointer. Read once, at load.
 *                    Absent, or an empty array = you fill "root", once
 *                    after the host's view, where every extension drew
 *                    before slots existed
 *                    (docs/adr/0014-slots-an-extension-fills-in-place.md).
 *   kui_ext_on_event One event of yours, payload borrowed for the call.
 *                    Answer the host from inside it with kui_reply, as
 *                    often as the event deserves.
 *   kui_ext_free     Your state, at unload.
 *
 * On ELF and Mach-O you link against nothing: leave every kui_* symbol
 * undefined and let it resolve from the host executable at load, the way a
 * Lua C module resolves lua_*. That asks one thing of the *host*, which
 * crates/kui-ffi/build.rs does for this crate's examples: link with
 * -rdynamic / --export-dynamic, so that the kui_* symbols in its binary are
 * also in the dynamic symbol table the loader reads. The cbuild tool
 * builds your side.
 *
 * ON WINDOWS THAT IS NOT AVAILABLE, and you pick one of two shapes instead.
 * A DLL may not have an unresolved import: your plugin names the module each
 * kui_* comes from in its own import table, and takes that name from an
 * import library at link time. So either -
 *
 *   1. link against kui_ffi.dll.lib. Your plugin imports from kui_ffi.dll
 *      and loads into ANY host that ships it, which is the ordinary Windows
 *      plugin shape (a Python extension imports from python313.dll, not
 *      from python.exe). Prefer this one: it is the plugin you can compile
 *      once and hand to somebody. examples/c/features/slots/host.c loads a plugin built
 *      this way, and the cbuild tool builds it as target/<profile>/panel.dll.
 *
 *   2. link against the *host's* import library - the .lib link.exe writes
 *      beside an executable that exports something. kui-ffi's build.rs makes
 *      target/debug/examples/c_panel.lib for the example Rust host by
 *      handing link.exe a /DEF: naming all 135 kui_*; a host of your own
 *      exports them the same way and ships the .lib it gets. The plugin then
 *      loads into that host and no other, because an import library names
 *      the module it imports from. Use it when the host statically links
 *      kui-ffi and ships no DLL, which every Rust host does by default.
 *
 * Shape 1 puts two copies of this library in one process, and that is fine:
 * everything crossing the boundary does so through a pointer the host hands
 * over, and the one thing that used to live in a static - kui_reply's sink -
 * rides on the event as of ABI 10 (see KuiReplySink). It was not fine
 * before ABI 10: every reply landed in the copy nobody was reading.
 *
 * The other Windows difference is below: a DLL exports nothing unless it
 * says so, which is what KUI_EXT_EXPORT on these seven declarations is for.
 * It is on the declarations rather than left to you, so a plugin that
 * includes this header and defines them the ordinary way is already
 * exported and your source stays the source that builds everywhere.
 */
typedef void (*KuiExtViewFn)(void *user, KuiCtx *ctx);
typedef void (*KuiExtEventFn)(void *user, const KuiEvent *ev);

#if defined(_WIN32)
#define KUI_EXT_EXPORT __declspec(dllexport)
#else
/* ELF and Mach-O export every non-static definition already. */
#define KUI_EXT_EXPORT
#endif

KUI_EXT_EXPORT uint32_t kui_ext_abi(void);
KUI_EXT_EXPORT const char *kui_ext_name(void);
KUI_EXT_EXPORT void *kui_ext_init(void);
KUI_EXT_EXPORT const KuiStr *kui_ext_slots(size_t *count);
KUI_EXT_EXPORT void kui_ext_view(void *user, KuiCtx *ctx);
KUI_EXT_EXPORT void kui_ext_on_event(void *user, const KuiEvent *ev);
KUI_EXT_EXPORT void kui_ext_free(void *user);

/* The library's side of the same contract - what an extension calls.
 * Which slot kui_ext_view is filling, in your own vocabulary (false, out
 * untouched, on a context that is not an extension's); the namespace the
 * host loaded you under, which is what makes the slot's full name
 * (`namespace/name`) and tells one instance of you from another when a
 * host loads you twice; and the params the host declared the slot with
 * (NULL when it passed none). All borrowed for the duration of the call,
 * like an event's payload. */
bool kui_slot_name(KuiCtx *ctx, KuiStr *out);
bool kui_slot_namespace(KuiCtx *ctx, KuiStr *out);
const KuiValue *kui_slot_params(KuiCtx *ctx);
/* A reply to the host, from inside kui_ext_on_event: `ev` is the event you
 * were handed, `reply` is copied (you keep ownership; free it as usual)
 * and reaches the host's on_event with your origin and the event's window
 * and key. The host authored what it expects - put its template in the
 * slot's params and fill in the fields. Outside the callback, or with an
 * event that is not the one in progress, it does nothing and returns
 * false. */
bool kui_reply(const KuiEvent *ev, const KuiValue *reply);

#ifdef __cplusplus
}
#endif

#endif /* KUI_H */
