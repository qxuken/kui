use super::*;
use std::fmt::Write as _;

/// One repr(C) struct's ABI, restated as a table and emitted as C.
///
/// The table is pinned to the Rust definition at compile time: the
/// destructuring names every field, so adding one to the struct stops
/// this module compiling until it is listed here (and mirrored in
/// include/kui.h), and each binding is checked against the Rust type the
/// row declares, so `radius: f32 => "float"` cannot outlive a change to
/// `radius: u32`. Offsets, sizes and alignments come from Rust itself.
macro_rules! abi_struct {
        ($out:expr, $ty:ident { $($f:ident : $rt:ty => $c:literal),* $(,)? }) => {{
            // Rebuilding the struct field by field is the pin: a field
            // added to $ty and not to the table below is a missing field in
            // this initializer (E0063, by name), and a field whose Rust type
            // changed no longer matches the `$rt` the row declares.
            #[allow(dead_code)]
            fn pinned(v: $ty) -> $ty {
                $(let $f: $rt = v.$f;)*
                $ty { $($f),* }
            }
            writeln!(
                $out,
                "KUI_STRUCT({}, {}, {});",
                stringify!($ty),
                std::mem::size_of::<$ty>(),
                std::mem::align_of::<$ty>(),
            )
            .unwrap();
            $(writeln!(
                $out,
                "KUI_FIELD({}, {}, {}, {}, {});",
                stringify!($ty),
                stringify!($f),
                $c,
                std::mem::offset_of!($ty, $f),
                std::mem::size_of::<$rt>(),
            )
            .unwrap();)*
            writeln!($out).unwrap();
        }};
    }

/// One [out] struct's size handshake, restated as C.
///
/// The `abi_struct!` row above it already pins `size`'s offset, size and
/// type; this adds the two facts that make the handshake work and that
/// no field-by-field check would notice: that `size` is the *first*
/// field (so a library can read a caller's reservation before trusting
/// anything else in the struct), and that the layout has never shrunk
/// below what ABI 1 shipped (so `ABI_V1_SIZE` is still a floor and not
/// a ceiling). The floor comes from Rust's own `OutParam` impl, so the
/// two cannot drift.
macro_rules! abi_out_struct {
    ($out:expr, $ty:ident) => {{
        writeln!(
            $out,
            "KUI_OUT_STRUCT({}, {});",
            stringify!($ty),
            <$ty as OutParam>::ABI_V1_SIZE,
        )
        .unwrap();
        writeln!($out).unwrap();
    }};
}

/// One C enum whose members are the indices of a list Rust owns,
/// restated as C.
///
/// The values come from Rust and the spellings from the header, and the
/// array's length is the list's, so a member added to the list — a role
/// appended to `Role::ALL` — stops this module compiling until the
/// header's name for it is listed here, and a name the header does not
/// define stops the generated C compiling. `$base` is the value index 0
/// takes in C: 1 wherever zero has to stay free for "unset".
macro_rules! abi_enum {
        ($out:expr, $list:expr, $base:expr => [$($c:literal),* $(,)?]) => {{
            const NAMES: [&str; $list.len()] = [$($c),*];
            for (i, name) in NAMES.iter().enumerate() {
                writeln!($out, "KUI_ENUM({}, {});", name, i + $base).unwrap();
            }
            writeln!($out).unwrap();
        }};
    }

/// One family of plain constants, restated as C: a flag word's bits, a
/// `kind`'s verbs, a code the entry points read. The Rust side is the
/// named constant the entry point now reads (`types.rs`), so a value
/// renumbered on either side fails the C build by name instead of
/// meaning something else at runtime.
macro_rules! abi_consts {
    ($out:expr, [$($c:ident),* $(,)?]) => {{
        $(writeln!($out, "KUI_ENUM({}, {});", stringify!($c), $c).unwrap();)*
        writeln!($out).unwrap();
    }};
}

/// The C spelling of a type that crosses the ABI in a prototype.
///
/// Pointers compose (`*const KuiSpec` is `const KuiSpec *`), so only the
/// leaves are listed; a type not listed here cannot appear in an
/// `abi_fn!` row, which is how a new type reaching a signature gets
/// noticed.
trait CType {
    fn c() -> String;
}
macro_rules! c_type {
    ($($t:ty => $c:literal),* $(,)?) => {
        $(impl CType for $t {
            fn c() -> String {
                $c.to_string()
            }
        })*
    };
}
c_type! {
    () => "void", bool => "bool", u8 => "uint8_t", u16 => "uint16_t",
    u32 => "uint32_t", u64 => "uint64_t", i64 => "int64_t", f32 => "float",
    f64 => "double", usize => "size_t", std::ffi::c_void => "void",
    KuiStr => "KuiStr", KuiCtx => "KuiCtx", KuiValue => "KuiValue",
    KuiSpec => "KuiSpec", KuiTextStyle => "KuiTextStyle", KuiSpan => "KuiSpan",
    KuiCell => "KuiCell", KuiMenuItem => "KuiMenuItem", KuiMenu => "KuiMenu",
    KuiMenuAction => "KuiMenuAction", KuiTextHit => "KuiTextHit",
    KuiCaretRect => "KuiCaretRect", KuiLayoutRect => "KuiLayoutRect",
    KuiTextMetrics => "KuiTextMetrics",
    KuiScrollGeometry => "KuiScrollGeometry", KuiAccessNode => "KuiAccessNode",
    KuiAccessRun => "KuiAccessRun", KuiAnnouncement => "KuiAnnouncement",
    KuiWarning => "KuiWarning", KuiSystemFont => "KuiSystemFont",
    KuiPlay => "KuiPlay", KuiAudio => "KuiAudio",
    KuiAudioCommand => "KuiAudioCommand", KuiTheme => "KuiTheme",
    KuiMetrics => "KuiMetrics", KuiColorToken => "KuiColorToken",
    KuiLengthToken => "KuiLengthToken", KuiColorOp => "KuiColorOp",
    KuiDerivedToken => "KuiDerivedToken",
    KuiEvent => "KuiEvent", KuiWindowConfig => "KuiWindowConfig",
    KuiRunConfig => "KuiRunConfig",
    KuiFileFilter => "KuiFileFilter", KuiFileDialog => "KuiFileDialog",
    KuiWindowCommand => "KuiWindowCommand", KuiDrawData => "KuiDrawData",
    ViewFn => "KuiViewFn",
}
#[cfg(feature = "runner")]
c_type! { EventFn => "KuiEventFn", TeardownFn => "KuiTeardownFn" }
impl<T: CType> CType for *mut T {
    fn c() -> String {
        format!("{} *", T::c())
    }
}
impl<T: CType> CType for *const T {
    fn c() -> String {
        format!("const {} *", T::c())
    }
}

/// One entry point's prototype, pinned to Rust and restated as C.
///
/// The pin is the `let`: the function has to coerce to exactly the
/// signature the row spells, so an argument added, removed or retyped in
/// Rust stops this module compiling until the row follows. The C side is
/// a second declaration of the same function in the generated translation
/// unit, after `kui.h`'s: C requires the two to agree, so a header whose
/// prototype has drifted from the row - a `const` dropped, an argument
/// the Rust side gained (ABI 12's case), a return type that changed -
/// fails to compile as "conflicting types". Names carry nothing here, so
/// only the types are spelled.
macro_rules! abi_fn {
    ($out:expr, $names:expr, $name:ident ( $($a:ty),* $(,)? ) $(-> $r:ty)?) => {{
        let _: unsafe extern "C" fn($($a),*) $(-> $r)? = $name;
        let args: Vec<String> = vec![$(<$a as CType>::c()),*];
        let args = if args.is_empty() { "void".to_string() } else { args.join(", ") };
        let ret = abi_fn!(@ret $($r)?);
        writeln!($out, "{} {}({});", ret, stringify!($name), args).unwrap();
        $names.push(stringify!($name));
    }};
    (@ret) => { "void".to_string() };
    (@ret $r:ty) => { <$r as CType>::c() };
}

const PRELUDE: &str = r#"/* Generated by `cargo test -p kui-ffi abi_parity` (see
 * crates/kui-ffi/src/abi_parity.rs) - do not edit, do not commit.
 * `cargo run -p kui-devtools --bin cbuild` regenerates and compiles it.
 *
 * Every field of every public repr(C) struct in the C API is pinned to the
 * offset, size and type Rust lays it out with, so a header that has drifted
 * from crates/kui-ffi/src/types.rs fails to compile here instead of misreading
 * every field after the drift at runtime. Nothing links: the asserts are
 * settled in the front end.
 */
#include "kui.h"

#define KUI_STRUCT(T, size, align)                                            \
    _Static_assert(sizeof(T) == (size), "sizeof(" #T ") differs from Rust");  \
    _Static_assert(_Alignof(T) == (align), "_Alignof(" #T ") differs from Rust")

/* CT is the C type the field must have: same size at the same offset is not
 * enough, `float` and `uint32_t` swap silently. Arrays are named by what
 * they decay to (`float *` for `float[4]`); their length is covered by the
 * size assert. */
#define KUI_FIELD(T, f, CT, off, size)                                        \
    _Static_assert(offsetof(T, f) == (off), #T "." #f ": offset differs from Rust");    \
    _Static_assert(sizeof(((T *)0)->f) == (size), #T "." #f ": size differs from Rust"); \
    _Static_assert(_Generic(((T *)0)->f, CT: 1, default: 0), #T "." #f ": type differs from Rust")

/* An [out] struct: one the library writes into memory the host reserved.
 * `size` has to lead it, because the library reads the host's reservation
 * out of those four bytes before it trusts any other byte of the struct;
 * and v1 is the layout ABI 1 shipped, which is a floor the struct may grow
 * past but must never fall below - a removed or narrowed field would have
 * an old host's reservation accepted and then under-filled. */
#define KUI_OUT_STRUCT(T, v1)                                                 \
    _Static_assert(offsetof(T, size) == 0, #T ".size must be the first field"); \
    _Static_assert(sizeof(T) >= (v1), #T " shrank below its ABI 1 layout")

/* One member of an enum the library reads as an index into a list Rust owns.
 * The value assert catches a member that drifted; the name itself catches
 * the commoner failure, a list that grew and a header that did not, which
 * fails here as an undeclared identifier instead of leaving C the one
 * binding that cannot say the new value. */
#define KUI_ENUM(name, value)                                                 \
    _Static_assert((name) == (value), #name " differs from Rust")

/* Below the asserts, every entry point is declared a second time with the
 * prototype Rust exports it under. C requires two declarations of one
 * function to agree, so a header prototype that drifted from the Rust
 * signature - an argument gained (ABI 12), a `const` dropped, a return
 * type changed - fails here as "conflicting types" instead of reading a
 * register the caller never filled. */
"#;

fn asserts() -> (String, Vec<&'static str>) {
    let mut o = String::from(PRELUDE);
    let mut names: Vec<&'static str> = Vec::new();
    o.push('\n');

    // The header's KUI_ABI_VERSION is what a host compares against
    // kui_abi_version() at startup, so a bump made in one place and not
    // the other would make that check pass on a mismatched pair.
    writeln!(
        o,
        "_Static_assert(KUI_ABI_VERSION == {}u, \"KUI_ABI_VERSION differs from Rust\");\n",
        KUI_ABI_VERSION,
    )
    .unwrap();

    // The enums whose members are positions in a list the core owns, so
    // that a list growing on the Rust side cannot leave the header - the
    // one binding that does not read the tables at runtime - without a
    // name for the new member.
    abi_enum!(o, kui_core::Role::ALL, 1 => [
        "KUI_ROLE_NONE", "KUI_ROLE_BUTTON", "KUI_ROLE_CHECKBOX",
        "KUI_ROLE_RADIO", "KUI_ROLE_SWITCH", "KUI_ROLE_SLIDER",
        "KUI_ROLE_TAB", "KUI_ROLE_TAB_LIST", "KUI_ROLE_LINK",
        "KUI_ROLE_HEADING", "KUI_ROLE_LIST", "KUI_ROLE_LIST_ITEM",
        "KUI_ROLE_IMAGE", "KUI_ROLE_DIALOG", "KUI_ROLE_GROUP",
        "KUI_ROLE_WINDOW", "KUI_ROLE_TITLE_BAR", "KUI_ROLE_STATIC_TEXT",
        "KUI_ROLE_TEXT_INPUT", "KUI_ROLE_MULTILINE_TEXT_INPUT",
        "KUI_ROLE_SCROLL_VIEW", "KUI_ROLE_LINE",
        "KUI_ROLE_RADIO_GROUP", "KUI_ROLE_MENU", "KUI_ROLE_MENU_ITEM",
        "KUI_ROLE_TERMINAL",
    ]);
    abi_enum!(o, kui_core::schema::ORIENTATIONS, 1 => [
        "KUI_ORIENTATION_HORIZONTAL", "KUI_ORIENTATION_VERTICAL",
    ]);
    abi_enum!(o, kui_core::schema::CURSORS, 1 => [
        "KUI_CURSOR_DEFAULT", "KUI_CURSOR_TEXT", "KUI_CURSOR_POINTER",
        "KUI_CURSOR_GRAB", "KUI_CURSOR_GRABBING", "KUI_CURSOR_NOT_ALLOWED",
        "KUI_CURSOR_EW_RESIZE", "KUI_CURSOR_NS_RESIZE",
        "KUI_CURSOR_NWSE_RESIZE", "KUI_CURSOR_NESW_RESIZE",
    ]);
    // Base 0: "unknown" is a member here, not the absence of one, so a
    // zeroed call means what it says.
    abi_enum!(o, kui_core::schema::APPEARANCES, 0 => [
        "KUI_APPEARANCE_UNKNOWN", "KUI_APPEARANCE_LIGHT", "KUI_APPEARANCE_DARK",
    ]);
    abi_enum!(o, kui_core::schema::MOTIONS, 0 => [
        "KUI_MOTION_UNKNOWN", "KUI_MOTION_FULL", "KUI_MOTION_REDUCED",
    ]);
    abi_enum!(o, kui_core::schema::ASSISTIVE, 0 => [
        "KUI_ASSISTIVE_UNKNOWN", "KUI_ASSISTIVE_NONE", "KUI_ASSISTIVE_LISTENING",
    ]);
    abi_enum!(o, kui_core::schema::AUDIO_DEVICES, 0 => [
        "KUI_AUDIO_DEVICE_CLOSED", "KUI_AUDIO_DEVICE_OPENING",
        "KUI_AUDIO_DEVICE_OPEN", "KUI_AUDIO_DEVICE_FAILED",
    ]);
    abi_enum!(o, kui_core::schema::EXPANDED, 1 => [
        "KUI_EXPANDED_COLLAPSED", "KUI_EXPANDED_EXPANDED",
    ]);
    abi_enum!(o, kui_core::schema::SCROLLBARS, 1 => [
        "KUI_SCROLLBAR_VISIBLE", "KUI_SCROLLBAR_HIDDEN", "KUI_SCROLLBAR_AUTO",
    ]);
    abi_enum!(o, kui_core::schema::WINDOW_ROLES, 1 => [
        "KUI_WINDOW_DRAG", "KUI_WINDOW_CLOSE", "KUI_WINDOW_MINIMIZE",
        "KUI_WINDOW_MAXIMIZE",
    ]);
    abi_enum!(o, kui_core::schema::ALIGNS, 0 => [
        "KUI_START", "KUI_CENTER", "KUI_END", "KUI_SPACE_BETWEEN",
        "KUI_SPACE_AROUND", "KUI_SPACE_EVENLY", "KUI_BASELINE",
    ]);
    abi_enum!(o, kui_core::schema::FAMILIES, 0 => [
        "KUI_FONT_SANS", "KUI_FONT_SERIF", "KUI_FONT_MONO",
    ]);
    abi_enum!(o, kui_core::schema::WRAPS, 0 => [
        "KUI_WRAP_WORD", "KUI_WRAP_GLYPH", "KUI_WRAP_NONE", "KUI_WRAP_BREAK_SPACES",
    ]);
    abi_enum!(o, kui_core::schema::UNDERLINE_STYLES, 0 => [
        "KUI_UNDERLINE_SOLID", "KUI_UNDERLINE_WAVY", "KUI_UNDERLINE_DOTTED",
    ]);
    abi_enum!(o, kui_core::schema::EASINGS, 0 => [
        "KUI_EASE_OUT", "KUI_EASE_LINEAR", "KUI_EASE_IN",
        "KUI_EASE_IN_OUT", "KUI_EASE_SPRING", "KUI_EASE_BOUNCY",
    ]);
    abi_enum!(o, kui_core::schema::REPEATS, 0 => [
        "KUI_REPEAT_NORMAL", "KUI_REPEAT_REVERSE", "KUI_REPEAT_ALTERNATE",
        "KUI_REPEAT_ALTERNATE_REVERSE",
    ]);
    abi_enum!(o, kui_core::QuadKind::ALL, 0 => [
        "KUI_QUAD_SOLID", "KUI_QUAD_GLYPH_MASK", "KUI_QUAD_GLYPH_COLOR",
        "KUI_QUAD_IMAGE", "KUI_QUAD_GLYPH_SUBPIXEL", "KUI_QUAD_SHADOW",
        "KUI_QUAD_SEGMENT", "KUI_QUAD_FRAGMENT", "KUI_QUAD_TEXTURE",
    ]);
    abi_enum!(o, kui_core::Sampling::ALL, 0 => [
        "KUI_SAMPLING_LINEAR", "KUI_SAMPLING_NEAREST",
    ]);
    abi_enum!(o, kui_core::ImageFit::ALL, 0 => [
        "KUI_FIT_FILL", "KUI_FIT_CONTAIN", "KUI_FIT_COVER",
    ]);
    abi_enum!(o, kui_core::schema::LIVE, 0 => [
        "KUI_LIVE_OFF", "KUI_LIVE_POLITE", "KUI_LIVE_ASSERTIVE",
    ]);
    // Base 1: zero is "no cursor" on the `kui_cells` call.
    abi_enum!(o, kui_core::CellCursor::NAMES, 1 => [
        "KUI_CELL_CURSOR_BLOCK", "KUI_CELL_CURSOR_BAR", "KUI_CELL_CURSOR_UNDERLINE",
    ]);
    abi_enum!(o, kui_core::MenuRole::ALL, 0 => [
        "KUI_MENU_CUSTOM", "KUI_MENU_SEPARATOR", "KUI_MENU_CUT", "KUI_MENU_COPY",
        "KUI_MENU_PASTE", "KUI_MENU_SELECT_ALL", "KUI_MENU_LOOK_UP",
    ]);
    // The action bits are positions in `AccessAction::ALL` as powers of
    // two: `KuiAccessNode.actions` and `kui_input_access` both read them.
    {
        const NAMES: [&str; kui_core::AccessAction::ALL.len()] = [
            "KUI_ACCESS_CLICK",
            "KUI_ACCESS_FOCUS",
            "KUI_ACCESS_BLUR",
            "KUI_ACCESS_SET_VALUE",
            "KUI_ACCESS_INCREMENT",
            "KUI_ACCESS_DECREMENT",
            "KUI_ACCESS_SCROLL_INTO_VIEW",
            "KUI_ACCESS_SCROLL_UP",
            "KUI_ACCESS_SCROLL_DOWN",
            "KUI_ACCESS_SCROLL_LEFT",
            "KUI_ACCESS_SCROLL_RIGHT",
            "KUI_ACCESS_SET_TEXT_SELECTION",
            "KUI_ACCESS_REPLACE_SELECTED_TEXT",
        ];
        for (name, action) in NAMES.iter().zip(kui_core::AccessAction::ALL) {
            writeln!(o, "KUI_ENUM({name}, {});", action.bit()).unwrap();
        }
        writeln!(o).unwrap();
    }
    // The editing keys, in the header's order rather than `EditKey`'s.
    for (i, (name, _)) in KUI_EDIT_KEYS.iter().enumerate() {
        writeln!(o, "KUI_ENUM({name}, {i});").unwrap();
    }
    writeln!(o).unwrap();
    // The mouse buttons are `MouseButton::code`, so the core's numbering
    // is what the header has to say.
    for (name, button) in [
        ("KUI_MOUSE_PRIMARY", kui_core::MouseButton::Primary),
        ("KUI_MOUSE_SECONDARY", kui_core::MouseButton::Secondary),
        ("KUI_MOUSE_MIDDLE", kui_core::MouseButton::Middle),
        ("KUI_MOUSE_OTHER", kui_core::MouseButton::Other(0)),
    ] {
        writeln!(o, "KUI_ENUM({name}, {});", button.code()).unwrap();
    }
    writeln!(o).unwrap();
    // A cell's attribute bits pass through to the core's own.
    for (name, bit) in [
        ("KUI_CELL_BOLD", kui_core::cells::flags::BOLD),
        ("KUI_CELL_ITALIC", kui_core::cells::flags::ITALIC),
        ("KUI_CELL_UNDERLINE", kui_core::cells::flags::UNDERLINE),
        (
            "KUI_CELL_STRIKETHROUGH",
            kui_core::cells::flags::STRIKETHROUGH,
        ),
        ("KUI_CELL_WIDE", kui_core::cells::flags::WIDE),
        ("KUI_CELL_WAVY", kui_core::cells::flags::WAVY),
        ("KUI_CELL_DOTTED", kui_core::cells::flags::DOTTED),
    ] {
        writeln!(o, "KUI_ENUM({name}, {bit});").unwrap();
    }
    writeln!(o).unwrap();
    // Every other plain constant the entry points read, by its Rust name.
    abi_consts!(
        o,
        [
            KUI_SPAN_BOLD,
            KUI_SPAN_ITALIC,
            KUI_SPAN_UNDERLINE,
            KUI_SPAN_STRIKETHROUGH,
            KUI_KMOD_SHIFT,
            KUI_KMOD_CTRL,
            KUI_KMOD_ALT,
            KUI_KMOD_SUPER,
            KUI_EDIT_MULTILINE,
            KUI_EDIT_AUTOFOCUS,
            KUI_EDIT_WRAP,
            KUI_MOD_SHIFT,
            KUI_MOD_WORD,
            KUI_MOD_DOC,
            KUI_MENU_ITEM_ENABLED,
            KUI_MENU_ITEM_CHECKED,
            KUI_MENU_ACTION_SET_CLIPBOARD,
            KUI_MENU_ACTION_PASTE,
            KUI_MENU_ACTION_LOOK_UP,
            KUI_MENU_ACTION_SET_CLIPBOARD_SECRET,
            KUI_PASTE_CONCEALED,
            KUI_PASTE_TRANSIENT,
            KUI_BUTTONS_SECONDARY,
            KUI_BUTTONS_MIDDLE,
            KUI_BUTTONS_OTHER,
            KUI_COPY_READY,
            KUI_COPY_ASKED,
            KUI_COPY_NOTHING,
            KUI_OWED_TRANSITION,
            KUI_OWED_CYCLE,
            KUI_OWED_DEPART,
            KUI_OWED_REQUESTED,
            KUI_OWED_AUTOSCROLL,
            KUI_OWED_SCROLL,
            KUI_AUDIO_PLAY,
            KUI_AUDIO_STOP,
            KUI_AUDIO_SET_VOLUME,
            KUI_AUDIO_PAUSE,
            KUI_AUDIO_RESUME,
            KUI_AUDIO_MASTER_VOLUME,
            KUI_AUDIO_UNLOAD,
            KUI_FRAGMENT_IMAGE_NONE,
            KUI_FRAGMENT_IMAGE_ATLAS,
            KUI_FRAGMENT_IMAGE_TEXTURE,
            KUI_WINDOW_NONE,
            KUI_FLOAT_NONE,
            KUI_FLOAT_PARENT,
            KUI_FLOAT_VIEWPORT,
            KUI_CHROME_NATIVE,
            KUI_CHROME_CUSTOM,
            KUI_CHROME_BORDERLESS,
            KUI_FILE_DIALOG_OPEN,
            KUI_FILE_DIALOG_SAVE,
            KUI_FILE_DIALOG_FOLDER,
            KUI_TEXT_AA_AUTO,
            KUI_TEXT_AA_GRAYSCALE,
            KUI_TEXT_AA_SUBPIXEL,
            KUI_DIAG_DEFAULT,
            KUI_DIAG_ON,
            KUI_DIAG_OFF,
            KUI_KF_AT,
            KUI_KF_WIDTH,
            KUI_KF_HEIGHT,
            KUI_KF_BG,
            KUI_KF_RADIUS,
            KUI_KF_OPACITY,
            KUI_ENTER_OFFSET,
            KUI_ENTER_WIDTH,
            KUI_ENTER_HEIGHT,
            KUI_ENTER_BG,
            KUI_ENTER_RADIUS,
            KUI_ENTER_OPACITY,
            KUI_OP_LIFT,
            KUI_OP_DARKEN,
            KUI_OP_RAISE,
            KUI_OP_ALPHA,
            KUI_OP_MIX,
            KUI_OP_READABLE,
            KUI_VALUE_NOW,
            KUI_VALUE_MIN,
            KUI_VALUE_MAX,
            KUI_VALUE_CARET,
            KUI_VALUE_ANCHOR,
            KUI_VALUE_CARET_SOLID,
            KUI_VALUE_STEP,
            KUI_ACCESS_HAS_VALUE,
            KUI_ACCESS_HAS_SELECTION,
            KUI_ACCESS_FOCUSED,
            KUI_ACCESS_CHECKED_SET,
            KUI_ACCESS_CHECKED,
            KUI_ACCESS_HAS_NUMBER,
            KUI_ACCESS_HAS_MIN,
            KUI_ACCESS_HAS_MAX,
            KUI_ACCESS_HAS_SCROLL,
            KUI_ACCESS_HAS_TEXT_SELECTION,
            KUI_ACCESS_DISABLED,
            KUI_ACCESS_MODAL,
            KUI_ACCESS_SELECTED_SET,
            KUI_ACCESS_SELECTED,
            KUI_ACCESS_EXPANDED_SET,
            KUI_ACCESS_EXPANDED,
            KUI_ACCESS_HAS_POS_IN_SET,
            KUI_ACCESS_HAS_SET_SIZE,
            KUI_ACCESS_LIVE_POLITE,
            KUI_ACCESS_LIVE_ASSERTIVE,
            KUI_ACCESS_MIXED,
        ]
    );

    abi_struct!(o, KuiScrollGeometry {
        size: u32 => "uint32_t",
        x: f32 => "float",
        y: f32 => "float",
        w: f32 => "float",
        h: f32 => "float",
        content_w: f32 => "float",
        content_h: f32 => "float",
        offset_x: f32 => "float",
        offset_y: f32 => "float",
        max_offset_x: f32 => "float",
        max_offset_y: f32 => "float",
    });
    abi_out_struct!(o, KuiScrollGeometry);

    abi_struct!(o, KuiCell {
        ch: u32 => "uint32_t",
        fg: u32 => "uint32_t",
        bg: u32 => "uint32_t",
        flags: u32 => "uint32_t",
        ul: u32 => "uint32_t",
    });

    abi_struct!(o, KuiMenuItem {
        label: KuiStr => "KuiStr",
        role: u32 => "uint32_t",
        enabled: u32 => "uint32_t",
        id: *const KuiValue => "const KuiValue *",
        accel: KuiStr => "KuiStr",
        checked: u32 => "uint32_t",
    });

    abi_struct!(o, KuiMenu {
        label: KuiStr => "KuiStr",
        items: *const KuiMenuItem => "const KuiMenuItem *",
        count: usize => "size_t",
        enabled: u32 => "uint32_t",
    });

    abi_struct!(o, KuiMenuAction {
        size: u32 => "uint32_t",
        kind: u32 => "uint32_t",
        text: KuiStr => "KuiStr",
        html: KuiStr => "KuiStr",
            x: f32 => "float",
        y: f32 => "float",
    });
    abi_out_struct!(o, KuiMenuAction);

    abi_struct!(o, KuiTextHit {
        size: u32 => "uint32_t",
        line: u32 => "uint32_t",
        byte: u64 => "uint64_t",
    });
    abi_out_struct!(o, KuiTextHit);

    abi_struct!(o, KuiCaretRect {
        size: u32 => "uint32_t",
        x: f32 => "float",
        y: f32 => "float",
        w: f32 => "float",
        h: f32 => "float",
    });
    abi_out_struct!(o, KuiCaretRect);

    abi_struct!(o, KuiLayoutRect {
        size: u32 => "uint32_t",
        x: f32 => "float",
        y: f32 => "float",
        w: f32 => "float",
        h: f32 => "float",
    });
    abi_out_struct!(o, KuiLayoutRect);

    abi_struct!(o, KuiStr {
        ptr: *const u8 => "const uint8_t *",
        len: usize => "size_t",
    });

    abi_struct!(o, KuiSizing {
        tag: u32 => "uint32_t",
        value: f32 => "float",
    });

    abi_struct!(o, KuiKeyframe {
        set: u32 => "uint32_t",
        at: f32 => "float",
        width: KuiSizing => "KuiSizing",
        height: KuiSizing => "KuiSizing",
        bg: u32 => "uint32_t",
        radius: f32 => "float",
        opacity: f32 => "float",
    });

    abi_struct!(o, KuiEnter {
        set: u32 => "uint32_t",
        dx: f32 => "float",
        dy: f32 => "float",
        width: KuiSizing => "KuiSizing",
        height: KuiSizing => "KuiSizing",
        bg: u32 => "uint32_t",
        radius: f32 => "float",
        opacity: f32 => "float",
    });

    abi_struct!(o, KuiSpec {
        width: KuiSizing => "KuiSizing",
        height: KuiSizing => "KuiSizing",
        min_w: f32 => "float",
        max_w: f32 => "float",
        min_h: f32 => "float",
        max_h: f32 => "float",
        dir: u32 => "uint32_t",
        pad_l: f32 => "float",
        pad_r: f32 => "float",
        pad_t: f32 => "float",
        pad_b: f32 => "float",
        gap: f32 => "float",
        main_align: u32 => "uint32_t",
        cross_align: u32 => "uint32_t",
        bg: u32 => "uint32_t",
        border_color: u32 => "uint32_t",
        border_w: f32 => "float",
        radius: f32 => "float",
        overflow: u32 => "uint32_t",
        float_mode: u32 => "uint32_t",
        float_anchor_x: u32 => "uint32_t",
        float_anchor_y: u32 => "uint32_t",
        float_self_x: u32 => "uint32_t",
        float_self_y: u32 => "uint32_t",
        float_dx: f32 => "float",
        float_dy: f32 => "float",
        float_fit: u32 => "uint32_t",
        hoverable: u32 => "uint32_t",
        window_role: u32 => "uint32_t",
        transition_ms: f32 => "float",
        easing: u32 => "uint32_t",
        slide: u32 => "uint32_t",
        hover_bg: u32 => "uint32_t",
        pressed_bg: u32 => "uint32_t",
        hover_group: KuiStr => "KuiStr",
        per_corner: u32 => "uint32_t",
        radius_tl: f32 => "float",
        radius_tr: f32 => "float",
        radius_br: f32 => "float",
        radius_bl: f32 => "float",
        repeat: u32 => "uint32_t",
        delay_ms: f32 => "float",
        keyframes: *const KuiKeyframe => "const KuiKeyframe *",
        keyframes_len: usize => "size_t",
        enter: KuiEnter => "KuiEnter",
        click_sound: u64 => "uint64_t",
        hover_sound: u64 => "uint64_t",
        on_layout: *const KuiValue => "const KuiValue *",
        role: u32 => "uint32_t",
        label: KuiStr => "KuiStr",
        checked: u32 => "uint32_t",
        value_set: u32 => "uint32_t",
        value_now: f32 => "float",
        value_min: f32 => "float",
        value_max: f32 => "float",
        caret: u32 => "uint32_t",
        selection_anchor: u32 => "uint32_t",
        focusable: u32 => "uint32_t",
        disabled: u32 => "uint32_t",
        focus_bg: u32 => "uint32_t",
        tooltip: KuiStr => "KuiStr",
        modal: *const KuiValue => "const KuiValue *",
        on_context_menu: *const KuiValue => "const KuiValue *",
        cursor: u32 => "uint32_t",
        selected: u32 => "uint32_t",
        expanded: u32 => "uint32_t",
        opacity_set: u32 => "uint32_t",
        opacity: f32 => "float",
        shadow_color: u32 => "uint32_t",
        shadow_blur: f32 => "float",
        shadow_x: f32 => "float",
        shadow_y: f32 => "float",
        shadow_spread: f32 => "float",
        wrap_children: u32 => "uint32_t",
        cross_gap: f32 => "float",
        initial_focus: u32 => "uint32_t",
        exit: KuiEnter => "KuiEnter",
        live: u32 => "uint32_t",
        key_up: u32 => "uint32_t",
        value_text: KuiStr => "KuiStr",
        description: KuiStr => "KuiStr",
        animate: u32 => "uint32_t",
        accent: u32 => "uint32_t",
        selectable: u32 => "uint32_t",
        on_force_click: *const KuiValue => "const KuiValue *",
        focus_region: u32 => "uint32_t",
        scrollbar: u32 => "uint32_t",
        scrollbar_width: f32 => "float",
        scrollbar_color: u32 => "uint32_t",
        scrollbar_active_color: u32 => "uint32_t",
        anchor: u32 => "uint32_t",
        on_scroll: *const KuiValue => "const KuiValue *",
        on_drop: *const KuiValue => "const KuiValue *",
        drop_bg: u32 => "uint32_t",
        float_clip: u32 => "uint32_t",
        aspect_ratio: f32 => "float",
        mixed: u32 => "uint32_t",
        value_step: f32 => "float",
        on_change: *const KuiValue => "const KuiValue *",
        pixel_snap: u32 => "uint32_t",
        keep_focus: u32 => "uint32_t",
        on_focus: *const KuiValue => "const KuiValue *",
        rules: u32 => "uint32_t",
        rule_w: f32 => "float",
        on_button: *const KuiValue => "const KuiValue *",
        buttons: u32 => "uint32_t",
    });

    abi_struct!(o, KuiAccessNode {
        key: u64 => "uint64_t",
        parent: u64 => "uint64_t",
        origin: u32 => "uint32_t",
        role: u32 => "uint32_t",
        flags: u32 => "uint32_t",
        actions: u32 => "uint32_t",
        name: KuiStr => "KuiStr",
        description: KuiStr => "KuiStr",
        value: KuiStr => "KuiStr",
        x: f32 => "float",
        y: f32 => "float",
        w: f32 => "float",
        h: f32 => "float",
        caret: u32 => "uint32_t",
        selection_start: u32 => "uint32_t",
        selection_end: u32 => "uint32_t",
        value_now: f32 => "float",
        value_min: f32 => "float",
        value_max: f32 => "float",
        scroll_x: f32 => "float",
        scroll_y: f32 => "float",
        scroll_max_x: f32 => "float",
        scroll_max_y: f32 => "float",
        anchor_run: u64 => "uint64_t",
        anchor_char: u32 => "uint32_t",
        focus_run: u64 => "uint64_t",
        focus_char: u32 => "uint32_t",
        run_count: u32 => "uint32_t",
        pos_in_set: u32 => "uint32_t",
        set_size: u32 => "uint32_t",
        orientation: u32 => "uint32_t",
    });

    abi_struct!(o, KuiAccessRun {
        key: u64 => "uint64_t",
        line: u32 => "uint32_t",
        start: u32 => "uint32_t",
        end: u32 => "uint32_t",
        text: KuiStr => "KuiStr",
        x: f32 => "float",
        y: f32 => "float",
        w: f32 => "float",
        h: f32 => "float",
        char_count: u32 => "uint32_t",
        char_lengths: *const u8 => "const uint8_t *",
        char_positions: *const f32 => "const float *",
        char_widths: *const f32 => "const float *",
        word_start_count: u32 => "uint32_t",
        word_starts: *const u8 => "const uint8_t *",
        rtl: u32 => "uint32_t",
    });

    abi_struct!(o, KuiTextMetrics {
        size: u32 => "uint32_t",
        width: f32 => "float",
        height: f32 => "float",
        lines: u32 => "uint32_t",
    });
    abi_out_struct!(o, KuiTextMetrics);

    abi_struct!(o, KuiWarning {
        code: KuiStr => "KuiStr",
        key: u64 => "uint64_t",
        message: KuiStr => "KuiStr",
    });

    abi_struct!(o, KuiSystemFont {
        family: KuiStr => "KuiStr",
        weights: *const u16 => "const uint16_t *",
        weight_count: u32 => "uint32_t",
        monospaced: u32 => "uint32_t",
        italic: u32 => "uint32_t",
    });

    abi_struct!(o, KuiAnnouncement {
        text: KuiStr => "KuiStr",
        live: u32 => "uint32_t",
    });

    abi_struct!(o, KuiPlay {
        volume: f32 => "float",
        looped: u32 => "uint32_t",
        fade_in_ms: f32 => "float",
    });

    abi_struct!(o, KuiAudio {
        src: u64 => "uint64_t",
        volume: f32 => "float",
        looped: u32 => "uint32_t",
        paused: u32 => "uint32_t",
        finish: u32 => "uint32_t",
    });

    abi_struct!(o, KuiAudioCommand {
        kind: u32 => "uint32_t",
        playback: u64 => "uint64_t",
        sound: u64 => "uint64_t",
        volume: f32 => "float",
        ms: f32 => "float",
        looped: u32 => "uint32_t",
    });

    abi_struct!(o, KuiTheme {
        size: u32 => "uint32_t",
        appearance: u32 => "uint32_t",
        disabled_opacity: f32 => "float",
        bg: u32 => "uint32_t",
        surface: u32 => "uint32_t",
        raised: u32 => "uint32_t",
        sunken: u32 => "uint32_t",
        border: u32 => "uint32_t",
        border_strong: u32 => "uint32_t",
        fg: u32 => "uint32_t",
        muted: u32 => "uint32_t",
        faint: u32 => "uint32_t",
        accent: u32 => "uint32_t",
        accent_hover: u32 => "uint32_t",
        accent_pressed: u32 => "uint32_t",
        on_accent: u32 => "uint32_t",
        accent_soft: u32 => "uint32_t",
        selection: u32 => "uint32_t",
        focus_ring: u32 => "uint32_t",
        hover: u32 => "uint32_t",
        pressed: u32 => "uint32_t",
        success: u32 => "uint32_t",
        warning: u32 => "uint32_t",
        danger: u32 => "uint32_t",
        scrollbar: u32 => "uint32_t",
        scrollbar_active: u32 => "uint32_t",
    });
    abi_out_struct!(o, KuiTheme);

    abi_struct!(o, KuiMetrics {
        size: u32 => "uint32_t",
        control_text: f32 => "float",
        chrome_text: f32 => "float",
        hint_text: f32 => "float",
        radius: f32 => "float",
        radius_inner: f32 => "float",
        control_pad_x: f32 => "float",
        control_pad_y: f32 => "float",
        field_pad_x: f32 => "float",
        field_pad_y: f32 => "float",
        hint_pad_x: f32 => "float",
        hint_pad_y: f32 => "float",
        menu_pad_x: f32 => "float",
        menu_pad_y: f32 => "float",
        menu_width: f32 => "float",
        menu_bar_h: f32 => "float",
        titlebar_h: f32 => "float",
    });
    abi_out_struct!(o, KuiMetrics);

    abi_struct!(o, KuiTextStyle {
        size: f32 => "float",
        line_height: f32 => "float",
        color: u32 => "uint32_t",
        family: u32 => "uint32_t",
        font: u64 => "uint64_t",
        wrap: u32 => "uint32_t",
        max_lines: u32 => "uint32_t",
        ellipsis: u32 => "uint32_t",
        features: KuiStr => "KuiStr",
        decoration: u32 => "uint32_t",
        underline_color: u32 => "uint32_t",
        underline_style: u32 => "uint32_t",
    });

    abi_struct!(o, KuiSpan {
        text: KuiStr => "KuiStr",
        color: u32 => "uint32_t",
        flags: u32 => "uint32_t",
        bg: u32 => "uint32_t",
        underline_color: u32 => "uint32_t",
        underline_style: u32 => "uint32_t",
        bg_radius: f32 => "float",
    });

    abi_struct!(o, KuiColorToken {
        name: KuiStr => "KuiStr",
        light: u32 => "uint32_t",
        dark: u32 => "uint32_t",
    });
    abi_struct!(o, KuiLengthToken {
        name: KuiStr => "KuiStr",
        value: f32 => "float",
    });
    abi_struct!(o, KuiColorOp {
        op: u8 => "uint8_t",
        t: f32 => "float",
        other: KuiStr => "KuiStr",
    });
    abi_struct!(o, KuiDerivedToken {
        name: KuiStr => "KuiStr",
        from: KuiStr => "KuiStr",
        ops: *const KuiColorOp => "const KuiColorOp *",
        op_count: usize => "size_t",
    });

    abi_struct!(o, KuiEvent {
        size: u32 => "uint32_t",
        origin: u16 => "uint16_t",
        key: u64 => "uint64_t",
        payload: *const KuiValue => "const KuiValue *",
        window: u32 => "uint32_t",
        reply_sink: *mut KuiReplySink => "KuiReplySink *",
        slot: u64 => "uint64_t",
    });
    abi_out_struct!(o, KuiEvent);

    abi_struct!(o, KuiWindowConfig {
        kind: u32 => "uint32_t",
        width: f32 => "float",
        height: f32 => "float",
        activates: u32 => "uint32_t",
        anchor_x: f32 => "float",
        anchor_y: f32 => "float",
        anchor_w: f32 => "float",
        anchor_h: f32 => "float",
    });

    abi_struct!(o, KuiFileFilter {
        name: KuiStr => "KuiStr",
        extensions: *const KuiStr => "const KuiStr *",
        extension_count: usize => "size_t",
    });

    abi_struct!(o, KuiFileDialog {
        mode: u32 => "uint32_t",
        multiple: u32 => "uint32_t",
        title: KuiStr => "KuiStr",
        filters: *const KuiFileFilter => "const KuiFileFilter *",
        filter_count: usize => "size_t",
        directory: KuiStr => "KuiStr",
        file_name: KuiStr => "KuiStr",
    });

    abi_struct!(o, KuiRunConfig {
        width: f32 => "float",
        height: f32 => "float",
        min_w: f32 => "float",
        min_h: f32 => "float",
        max_w: f32 => "float",
        max_h: f32 => "float",
        chrome: u32 => "uint32_t",
        text_aa: u32 => "uint32_t",
        diagnostics: u32 => "uint32_t",
        frame_latency: u32 => "uint32_t",
    });

    abi_struct!(o, KuiWindowCommand {
        size: u32 => "uint32_t",
        kind: u32 => "uint32_t",
        window: u32 => "uint32_t",
        origin: u16 => "uint16_t",
        config: KuiWindowConfig => "KuiWindowConfig",
        width: f32 => "float",
        height: f32 => "float",
        owner: u32 => "uint32_t",
    });
    abi_out_struct!(o, KuiWindowCommand);

    // The command verbs, the window kinds and the dismiss reasons are
    // plain constants on both sides; pinned here so the header cannot
    // renumber one.
    for (name, value) in [
        ("KUI_CMD_START_DRAG", KUI_CMD_START_DRAG),
        ("KUI_CMD_CLOSE", KUI_CMD_CLOSE),
        ("KUI_CMD_MINIMIZE", KUI_CMD_MINIMIZE),
        ("KUI_CMD_SET_SIZE", KUI_CMD_SET_SIZE),
        ("KUI_CMD_FOCUS", KUI_CMD_FOCUS),
        ("KUI_CMD_REDRAW", KUI_CMD_REDRAW),
        ("KUI_CMD_TOGGLE_MAXIMIZE", KUI_CMD_TOGGLE_MAXIMIZE),
        ("KUI_CMD_OPEN", KUI_CMD_OPEN),
        ("KUI_WINDOW_KIND_NORMAL", KUI_WINDOW_KIND_NORMAL),
        ("KUI_WINDOW_KIND_POPUP", KUI_WINDOW_KIND_POPUP),
        ("KUI_DISMISS_OUTSIDE", KUI_DISMISS_OUTSIDE),
        ("KUI_DISMISS_ESCAPE", KUI_DISMISS_ESCAPE),
        ("KUI_WINDOW_MAIN", WindowId::MAIN.0),
    ] {
        writeln!(o, "KUI_ENUM({name}, {value});").unwrap();
    }
    writeln!(o).unwrap();

    abi_struct!(o, KuiQuad {
        x: f32 => "float",
        y: f32 => "float",
        w: f32 => "float",
        h: f32 => "float",
        color: [f32; 4] => "float *",
        border_color: [f32; 4] => "float *",
        radius: [f32; 4] => "float *",
        border_w: f32 => "float",
        blur: f32 => "float",
        kind: u32 => "uint32_t",
        clip: u32 => "uint32_t",
        uv: [u32; 4] => "uint32_t *",
    });

    abi_struct!(o, KuiClip {
        rect: [f32; 4] => "float *",
        radius: [f32; 4] => "float *",
    });

    abi_struct!(o, KuiDrawData {
        size: u32 => "uint32_t",
        quads: *const KuiQuad => "const KuiQuad *",
        quad_count: usize => "size_t",
        viewport_w: f32 => "float",
        viewport_h: f32 => "float",
        scale: f32 => "float",
        atlas_pixels: *const u8 => "const uint8_t *",
        atlas_size: u32 => "uint32_t",
        atlas_dirty: bool => "bool",
        atlas_epoch: u64 => "uint64_t",
        fragments: *const KuiFragmentDraw => "const KuiFragmentDraw *",
        fragment_count: usize => "size_t",
        time: f32 => "float",
        clips: *const KuiClip => "const KuiClip *",
        clip_count: usize => "size_t",
        textures: *const KuiTextureDraw => "const KuiTextureDraw *",
        texture_count: usize => "size_t",
    });
    abi_out_struct!(o, KuiDrawData);
    abi_struct!(o, KuiFragmentDraw {
        fragment: u64 => "uint64_t",
        params: [f32; 16] => "float *",
        image_source: u32 => "uint32_t",
        image_texture: u32 => "uint32_t",
        image_uv: [u32; 4] => "uint32_t *",
    });
    abi_struct!(o, KuiTextureDraw {
        image: u64 => "uint64_t",
        rev: u32 => "uint32_t",
        width: u32 => "uint32_t",
        height: u32 => "uint32_t",
        uv: [u32; 4] => "uint32_t *",
    });

    // Every entry point, one row each, generated by build.rs from the
    // `pub extern "C" fn kui_*` signatures themselves (backlog AR2) — so
    // a function cannot be exported and unpinned, and the row cannot
    // spell a signature its function does not have: the `let` in
    // `abi_fn!` coerces the function to the row, and a misread is a
    // compile error here. The runner's two are gated as their module is;
    // the header declares them unconditionally, and the name check below
    // knows to expect that. What the row does not carry is the header's
    // prose, which is why the header is written by hand (ADR 0020).
    let n = &mut names;
    include!(concat!(env!("OUT_DIR"), "/abi_rows.rs"));

    (o, names)
}

/// Writes the asserts next to the built library, where
/// the `cbuild` tool (examples/devtools/src/bin/cbuild.rs) picks them up. The C compiler is the check; this
/// test only produces what it checks (and fails if the tree is not
/// writable), so the two halves stay one `cbuild` run apart.
#[test]
fn writes_the_c_abi_asserts() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    std::fs::create_dir_all(&dir).expect("create target dir");
    let path = dir.join("kui-abi-assert.c");
    let (text, _) = asserts();
    assert!(text.contains("KUI_FIELD(KuiSpec, focus_bg,"));
    assert!(text.contains("KUI_OUT_STRUCT(KuiEvent,"));
    assert!(text.contains("KUI_OUT_STRUCT(KuiWindowCommand,"));
    assert!(text.contains("KUI_OUT_STRUCT(KuiTheme,"));
    assert!(text.contains("KUI_ENUM(KUI_ROLE_LINE, 22);"));
    assert!(text.contains("KUI_ENUM(KUI_ACCESS_REPLACE_SELECTED_TEXT, 4096);"));
    assert!(text.contains("bool kui_poll_event(KuiCtx *, KuiEvent *);"));
    assert!(text.contains(
        "void kui_fragment(KuiCtx *, uint64_t, const float *, size_t, const KuiSpec *);"
    ));
    std::fs::write(&path, text).expect("write kui-abi-assert.c");
}

/// `KUI_EDIT_KEYS` restates `EditKey` in the header's order, which is not
/// the enum's: so the table is checked against `EditKey::ALL` both ways —
/// every key has a C name, no C name is a key twice — and a variant added
/// to the enum fails here until the header gets its constant.
#[test]
fn every_edit_key_has_one_c_constant() {
    use std::collections::BTreeSet;
    let table: Vec<kui_core::EditKey> = KUI_EDIT_KEYS.iter().map(|(_, k)| *k).collect();
    for k in kui_core::EditKey::ALL {
        assert_eq!(
            table.iter().filter(|t| **t == k).count(),
            1,
            "{k:?} ({}) must appear in KUI_EDIT_KEYS exactly once",
            k.name()
        );
    }
    assert_eq!(table.len(), kui_core::EditKey::ALL.len());
    let names: BTreeSet<&str> = KUI_EDIT_KEYS.iter().map(|(n, _)| *n).collect();
    assert_eq!(names.len(), KUI_EDIT_KEYS.len(), "a C name is repeated");
}

/// The prototype rows above are only a pin for the functions they list,
/// so this holds the list to the two places a function can otherwise
/// appear: every `kui_*` the header declares (which is what a C host can
/// call) and every `extern "C" fn kui_*` the sources export (which is
/// what the library links). An entry point added to either without a row
/// here fails by name. The header's `kui_ext_*` are the plugin's exports,
/// not the library's, and `kui_str_eq` is a static inline the header
/// defines itself; neither is a symbol of this crate.
/// The functions `include/kui.h` declares, by name: every `kui_*` that
/// opens a prototype, less the plugin's exports and the header's own
/// static inline.
fn header_declares() -> std::collections::BTreeSet<&'static str> {
    let header = include_str!("../include/kui.h");
    let mut declared = std::collections::BTreeSet::new();
    for line in header.lines() {
        // A prototype: the first `kui_` word on a line that ends in `(`
        // territory and is not a comment. Multi-line prototypes name the
        // function on their first line, which is the one that matters.
        let trimmed = line.trim_start();
        if trimmed.starts_with('*') || trimmed.starts_with("/*") || trimmed.starts_with("#") {
            continue;
        }
        let Some(idx) = line.find("kui_") else {
            continue;
        };
        let name: &str = line[idx..]
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .next()
            .unwrap();
        let after = &line[idx + name.len()..];
        if !after.trim_start().starts_with('(') {
            continue;
        }
        // A prototype starts the line with its return type; a call in a
        // comment's code sample is indented under the comment and skipped
        // above, and the macro bodies are `#define` lines.
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        if name.starts_with("kui_ext_") || name == "kui_str_eq" {
            continue;
        }
        declared.insert(name);
    }
    declared
}

#[test]
fn every_entry_point_is_pinned() {
    use std::collections::BTreeSet;
    let (_, pinned) = asserts();
    let pinned: BTreeSet<&str> = pinned.into_iter().collect();
    let declared = header_declares();

    let mut exported = BTreeSet::new();
    for src in [
        include_str!("lib.rs"),
        include_str!("abi.rs"),
        include_str!("access.rs"),
        include_str!("dialogs.rs"),
        include_str!("focus.rs"),
        include_str!("frame.rs"),
        include_str!("input.rs"),
        include_str!("menu.rs"),
        include_str!("resources.rs"),
        include_str!("run.rs"),
        include_str!("scrolling.rs"),
        include_str!("select.rs"),
        include_str!("slots.rs"),
        include_str!("value.rs"),
        include_str!("widgets.rs"),
        include_str!("windows.rs"),
    ] {
        for piece in src.split("extern \"C\" fn ").skip(1) {
            let name = piece
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .next()
                .unwrap();
            if name.starts_with("kui_") {
                exported.insert(name);
            }
        }
    }
    // The runner's four are pinned only in a build that has them.
    let runner = ["kui_run", "kui_run_with", "kui_on_teardown", "kui_set_icon"];
    let mut expected = declared.clone();
    if cfg!(not(feature = "runner")) {
        for name in runner {
            expected.remove(name);
        }
    }
    assert_eq!(
        pinned, expected,
        "the header's prototypes and abi_parity's rows name different functions"
    );
    assert_eq!(
        exported, declared,
        "the sources export and the header declares different functions"
    );
}

/// `examples/c/tools/surface.c` opens with "every prototype in kui.h
/// called once", and for a long while twenty-one were called by nothing
/// in the tree (backlog AR46): the pin above holds a prototype's *types*
/// to the Rust signature, and a door nobody calls can still decode its
/// arguments wrong for a release without failing anything. This holds
/// the walk's claim: every function the header declares is called by
/// one of the C programs `cbuild` compiles and the C round runs —
/// `surface.c` for nearly all of them, `counter.c` / `host.c` for the
/// runner, `host.c` / `panel.c` for the extension contract,
/// `conformance.c` for the corpus's elements. A call in a comment does
/// not count; a prototype no program calls fails by name, and the fix is
/// a call with something checked, not a line here — which is why there
/// is no exempt list.
#[test]
fn every_entry_point_is_called() {
    let programs = [
        include_str!("../../../examples/c/common.h"),
        include_str!("../../../examples/c/tools/surface.c"),
        include_str!("../../../examples/c/tools/conformance.c"),
        include_str!("../../../examples/c/apps/counter.c"),
        include_str!("../../../examples/c/features/slots/host.c"),
        include_str!("../../../examples/c/features/slots/panel.c"),
    ];
    let mut called = std::collections::BTreeSet::new();
    for src in programs {
        // Comments out first: a prototype a program only talks about is
        // not one it calls.
        let mut code = String::with_capacity(src.len());
        let mut rest = src;
        while let Some(open) = rest.find("/*") {
            code.push_str(&rest[..open]);
            let Some(close) = rest[open..].find("*/") else {
                break;
            };
            rest = &rest[open + close + 2..];
        }
        code.push_str(rest);
        for (i, _) in code.match_indices("kui_") {
            let name: &str = code[i..]
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .next()
                .unwrap();
            if code[i + name.len()..].trim_start().starts_with('(') {
                called.insert(name.to_string());
            }
        }
    }
    let uncalled: Vec<&str> = header_declares()
        .into_iter()
        .filter(|name| !called.contains(*name))
        .collect();
    assert!(
        uncalled.is_empty(),
        "no C program under examples/c calls these; add each to surface.c's walk with something checked: {uncalled:?}"
    );
}

/// ADR 0006 decision 7 put the audit of who writes which struct in the
/// header, where the person appending a field reads it - so it has to be
/// true. This holds it to the structs the rows above lay out: every
/// struct with a layout is in exactly one of the four paragraphs, and the
/// `[out]` paragraph names exactly the structs that implement `OutParam`
/// (the ones with a leading `size`), so a struct that gained the
/// handshake without joining the paragraph, or joined it without the
/// handshake, fails here by name.
#[test]
fn the_headers_audit_names_every_struct_once() {
    use std::collections::{BTreeMap, BTreeSet};
    let (text, _) = asserts();
    let laid_out: BTreeSet<&str> = text
        .lines()
        .filter_map(|l| l.strip_prefix("KUI_STRUCT("))
        .map(|l| l.split(',').next().unwrap())
        .collect();
    let out_params: BTreeSet<&str> = text
        .lines()
        .filter_map(|l| l.strip_prefix("KUI_OUT_STRUCT("))
        .map(|l| l.split(',').next().unwrap())
        .collect();

    let header = include_str!("../include/kui.h");
    let start = header
        .find("/* -- Who writes what")
        .expect("the audit block");
    let end = header[start..]
        .find("KuiStr is the exception")
        .expect("the audit block's end")
        + start;
    let audit = &header[start..end];
    // Each paragraph opens with its tag in column 3; the names it lists
    // are every `Kui*` word in it (KUI_*_INIT and KUI_ABI_VERSION are
    // uppercase and fall out of the filter).
    let mut where_named: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut tag = "";
    for line in audit.lines() {
        let body = line.trim_start_matches([' ', '*']);
        for t in ["[in]", "[out]", "[out[]]", "[lib]"] {
            if body.starts_with(t) {
                tag = t;
            }
        }
        for word in body.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            if word.starts_with("Kui") && word != "KuiStr" {
                where_named.entry(word).or_default().push(tag);
            }
        }
    }
    // Two mentions are not listings: `KuiDrawData.quads` names the field's
    // struct inside the [lib] paragraph, and the closing note says where
    // else a KuiEvent travels. Everything else in the block is a listing.
    if let Some(v) = where_named.get_mut("KuiDrawData") {
        v.retain(|t| *t == "[out]");
    }
    if let Some(v) = where_named.get_mut("KuiEvent") {
        v.retain(|t| *t == "[out]");
    }
    for name in &laid_out {
        if *name == "KuiStr" {
            // Frozen, and the block says so in its own sentence.
            continue;
        }
        let tags: BTreeSet<&str> = where_named
            .get(name)
            .unwrap_or_else(|| {
                panic!("{name} has a layout but the header's audit does not say who writes it")
            })
            .iter()
            .copied()
            .collect();
        assert_eq!(
            tags.len(),
            1,
            "{name} is in more than one audit paragraph: {tags:?}"
        );
        let is_out = tags.contains("[out]");
        assert_eq!(
            is_out,
            out_params.contains(name),
            "{name}: the audit's [out] paragraph and the OutParam impls disagree"
        );
    }
    for name in where_named.keys() {
        assert!(
            laid_out.contains(name) || *name == "KuiCtx",
            "{name} is in the audit but has no layout row"
        );
    }
}

/// An [in] struct's layout is the ABI's like any other's (ADR 0006's
/// 2026-09-14 amendment, backlog AR50): the library reads the whole
/// struct, so a field appended to one is read from past the end of a
/// host that did not recompile, and the version has to say so. This
/// holds every struct the header's `[in]` paragraph lists to its size
/// here, so an append fails until the row and `KUI_ABI_VERSION` both
/// move — the table is the reminder, the message is the rule. Sizes are
/// the 64-bit ones (`KuiStr` and the tag pointers are pointer-wide);
/// the field-by-field pin above is what holds the layout per target.
#[cfg(target_pointer_width = "64")]
#[test]
fn an_in_struct_s_size_is_the_abi_s() {
    use std::collections::BTreeMap;
    // (name, size in bytes, the ABI the size is from)
    const IN_LAYOUTS: &[(&str, usize, u32)] = &[
        // ABI 20: `pixel_snap` appended, after ABI 19's `float_clip`,
        // `aspect_ratio`, `mixed`, `value_step` and `on_change`; then
        // `keep_focus`, `on_focus`, `rules` and `rule_w`; then
        // `on_button` and `buttons` (backlog F105).
        ("KuiSpec", 640, 20),
        ("KuiSizing", 8, 16),
        ("KuiKeyframe", 36, 16),
        ("KuiEnter", 40, 16),
        ("KuiTextStyle", 72, 17),
        // ABI 20: `bg_radius` appended into what was the tail padding, so
        // the 64-bit size stayed at 40 while the layout moved.
        ("KuiSpan", 40, 20),
        ("KuiCell", 20, 17),
        ("KuiMenuItem", 56, 16),
        ("KuiMenu", 40, 16),
        ("KuiPlay", 12, 16),
        ("KuiAudio", 24, 16),
        ("KuiWindowConfig", 32, 16),
        ("KuiRunConfig", 40, 19),
        // ABI 19: the file dialogs (backlog C51).
        ("KuiFileFilter", 32, 19),
        ("KuiFileDialog", 72, 19),
        ("KuiColorToken", 24, 16),
        ("KuiLengthToken", 24, 16),
        ("KuiColorOp", 24, 16),
        ("KuiDerivedToken", 48, 16),
    ];
    let (text, _) = asserts();
    let sizes: BTreeMap<&str, usize> = text
        .lines()
        .filter_map(|l| l.strip_prefix("KUI_STRUCT("))
        .map(|l| {
            let mut it = l.split(", ");
            (it.next().unwrap(), it.next().unwrap().parse().unwrap())
        })
        .collect();
    let header = include_str!("../include/kui.h");
    let start = header.find(" * [in]     ").expect("the [in] paragraph");
    let end = header[start..].find(" * [out]    ").expect("its end") + start;
    let listed: Vec<&str> = header[start..end]
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| w.starts_with("Kui") && *w != "KuiStr")
        .collect();
    for name in &listed {
        let (_, pinned, abi) = IN_LAYOUTS
            .iter()
            .find(|(n, ..)| n == name)
            .unwrap_or_else(|| panic!("{name} is an [in] struct with no size row here"));
        assert!(*abi <= KUI_ABI_VERSION, "{name}'s row is from the future");
        assert_eq!(
            sizes[name], *pinned,
            "{name} changed size: an [in] struct's layout is the ABI's, so bump \
             KUI_ABI_VERSION, log it in include/kui.h and abi.rs, and set this row \
             to the new size and number"
        );
    }
    for (name, ..) in IN_LAYOUTS {
        assert!(
            listed.contains(name),
            "{name} has a row here but is not [in]"
        );
    }
}
