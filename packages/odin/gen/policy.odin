package gen

// What the typed layer needs that neither kui.h nor the schema says in a
// form a program can read: which C constants form a type, which integer
// parameter takes which of them, and how the rows that do not land on a
// KuiSpec field of their own name are lowered. Every row is checked
// against both inputs when it is used - a row naming a constant, a field,
// a parameter or a prop that is not there stops the generator - so a
// stale row is an error, never a silent no-op.

// -- Enums ------------------------------------------------------------------

// One Odin type over a family of C constants: the family of `first` (its C
// enum, or its run of adjacent #defines), the members named after
// `prefix`. `flag` names a bit set's element enum; `list` is the schema
// name list the members must cover (`lists` in the dump); `backing` the
// integer type when the C field is not a uint32_t.
Enum_Rule :: struct {
	name:    string,
	first:   string,
	prefix:  string,
	flag:    string,
	list:    string,
	backing: string,
}

ENUMS :: []Enum_Rule {
	// The node's.
	{name = "Sizing_Tag", first = "KUI_FIT", prefix = "KUI_"},
	{name = "Dir", first = "KUI_COLUMN", prefix = "KUI_"},
	{name = "Align", first = "KUI_START", prefix = "KUI_"},
	{name = "Overflow", first = "KUI_CLIP", prefix = "KUI_", flag = "Overflow_Flag"},
	{name = "Float_Mode", first = "KUI_FLOAT_NONE", prefix = "KUI_FLOAT_"},
	{name = "Window_Role", first = "KUI_WINDOW_NONE", prefix = "KUI_WINDOW_"},
	{name = "Ease", first = "KUI_EASE_OUT", prefix = "KUI_EASE_"},
	{name = "Repeat", first = "KUI_REPEAT_NORMAL", prefix = "KUI_REPEAT_"},
	{name = "Keyframe_Slots", first = "KUI_KF_AT", prefix = "KUI_KF_", flag = "Keyframe_Slot"},
	{name = "Enter_Slots", first = "KUI_ENTER_OFFSET", prefix = "KUI_ENTER_", flag = "Enter_Slot"},
	{name = "Gradient_Kind", first = "KUI_GRADIENT_LINEAR", prefix = "KUI_GRADIENT_"},
	{name = "Role", first = "KUI_ROLE_NONE", prefix = "KUI_ROLE_", list = "accessRoles"},
	{name = "Cursor", first = "KUI_CURSOR_DEFAULT", prefix = "KUI_CURSOR_"},
	{name = "Expanded", first = "KUI_EXPANDED_COLLAPSED", prefix = "KUI_EXPANDED_"},
	{name = "Live", first = "KUI_LIVE_OFF", prefix = "KUI_LIVE_", list = "live"},
	{name = "Scrollbar", first = "KUI_SCROLLBAR_VISIBLE", prefix = "KUI_SCROLLBAR_"},
	{name = "Overscroll", first = "KUI_OVERSCROLL_AUTO", prefix = "KUI_OVERSCROLL_"},
	{name = "Scroll_Axes", first = "KUI_SCROLL_AXES_BOTH", prefix = "KUI_SCROLL_AXES_"},
	{name = "Mods", first = "KUI_KMOD_SHIFT", prefix = "KUI_KMOD_", flag = "Mod"},
	{name = "Mouse_Buttons", first = "KUI_BUTTONS_SECONDARY", prefix = "KUI_BUTTONS_", flag = "Mouse_Button"},
	// Text.
	{name = "Font_Family", first = "KUI_FONT_SANS", prefix = "KUI_FONT_"},
	{name = "Wrap", first = "KUI_WRAP_WORD", prefix = "KUI_WRAP_"},
	{name = "Decorations", first = "KUI_DECO_UNDERLINE", prefix = "KUI_DECO_", flag = "Decoration"},
	{name = "Underline_Style", first = "KUI_UNDERLINE_SOLID", prefix = "KUI_UNDERLINE_"},
	{name = "Span_Flags", first = "KUI_SPAN_BOLD", prefix = "KUI_SPAN_", flag = "Span_Flag"},
	{name = "Cell_Flags", first = "KUI_CELL_BOLD", prefix = "KUI_CELL_", flag = "Cell_Flag"},
	{name = "Cell_Cursor", first = "KUI_CELL_CURSOR_BLOCK", prefix = "KUI_CELL_CURSOR_"},
	{name = "Edit_Flags", first = "KUI_EDIT_MULTILINE", prefix = "KUI_EDIT_", flag = "Edit_Flag"},
	// Input.
	{name = "Edit_Key", first = "KUI_KEY_LEFT", prefix = "KUI_KEY_", list = "editKeys"},
	{name = "Edit_Mods", first = "KUI_MOD_SHIFT", prefix = "KUI_MOD_", flag = "Edit_Mod"},
	{name = "Mouse", first = "KUI_MOUSE_PRIMARY", prefix = "KUI_MOUSE_", list = "mouseButtons"},
	{name = "Paste_Marks", first = "KUI_PASTE_CONCEALED", prefix = "KUI_PASTE_", flag = "Paste_Mark"},
	{name = "Access_Actions", first = "KUI_ACCESS_CLICK", prefix = "KUI_ACCESS_", flag = "Access_Action", list = "accessActions"},
	{name = "Access_Flags", first = "KUI_ACCESS_HAS_VALUE", prefix = "KUI_ACCESS_", flag = "Access_Flag"},
	{name = "Orientation", first = "KUI_ORIENTATION_HORIZONTAL", prefix = "KUI_ORIENTATION_", list = "orientations"},
	// The host's.
	{name = "Appearance", first = "KUI_APPEARANCE_UNKNOWN", prefix = "KUI_APPEARANCE_", list = "appearances"},
	{name = "Motion", first = "KUI_MOTION_UNKNOWN", prefix = "KUI_MOTION_", list = "motions"},
	{name = "Assistive", first = "KUI_ASSISTIVE_UNKNOWN", prefix = "KUI_ASSISTIVE_", list = "assistive"},
	{name = "Audio_Device", first = "KUI_AUDIO_DEVICE_CLOSED", prefix = "KUI_AUDIO_DEVICE_", list = "audioDevices"},
	{name = "Backdrop", first = "KUI_BACKDROP_OPAQUE", prefix = "KUI_BACKDROP_"},
	{name = "Audio_Cmd", first = "KUI_AUDIO_PLAY", prefix = "KUI_AUDIO_"},
	{name = "Window_Kind", first = "KUI_WINDOW_KIND_NORMAL", prefix = "KUI_WINDOW_KIND_", list = "windowKinds"},
	{name = "Window_Cmd", first = "KUI_CMD_START_DRAG", prefix = "KUI_CMD_"},
	{name = "Dismiss_Reason", first = "KUI_DISMISS_OUTSIDE", prefix = "KUI_DISMISS_"},
	{name = "Option_As_Alt", first = "KUI_OPTION_AS_ALT_NONE", prefix = "KUI_OPTION_AS_ALT_"},
	{name = "Chrome", first = "KUI_CHROME_NATIVE", prefix = "KUI_CHROME_"},
	{name = "Text_AA", first = "KUI_TEXT_AA_AUTO", prefix = "KUI_TEXT_AA_"},
	{name = "Diagnostics", first = "KUI_DIAG_DEFAULT", prefix = "KUI_DIAG_"},
	{name = "Menu_Role", first = "KUI_MENU_CUSTOM", prefix = "KUI_MENU_", list = "menuRoles"},
	{name = "Menu_Item_Flags", first = "KUI_MENU_ITEM_ENABLED", prefix = "KUI_MENU_ITEM_", flag = "Menu_Item_Flag"},
	{name = "Menu_Action_Kind", first = "KUI_MENU_ACTION_SET_CLIPBOARD", prefix = "KUI_MENU_ACTION_"},
	{name = "Copy_Answer", first = "KUI_COPY_READY", prefix = "KUI_COPY_"},
	{name = "Frame_Causes", first = "KUI_FRAME_CAUSE_POINTER_MOVE", prefix = "KUI_FRAME_CAUSE_", flag = "Frame_Cause"},
	{name = "Owed", first = "KUI_OWED_TRANSITION", prefix = "KUI_OWED_", flag = "Owed_Reason"},
	{name = "File_Dialog_Mode", first = "KUI_FILE_DIALOG_OPEN", prefix = "KUI_FILE_DIALOG_"},
	{name = "Color_Op_Kind", first = "KUI_OP_LIFT", prefix = "KUI_OP_", backing = "u8"},
	{name = "Fill_Rule", first = "KUI_FILL_NONZERO", prefix = "KUI_FILL_"},
	{name = "Sampling", first = "KUI_SAMPLING_LINEAR", prefix = "KUI_SAMPLING_", list = "imageSampling"},
	{name = "Image_Fit", first = "KUI_FIT_FILL", prefix = "KUI_FIT_", list = "imageFit"},
	{name = "Quad_Kind", first = "KUI_QUAD_SOLID", prefix = "KUI_QUAD_"},
	{name = "Fragment_Image", first = "KUI_FRAGMENT_IMAGE_NONE", prefix = "KUI_FRAGMENT_IMAGE_"},
}

// The registered-resource handles, distinct so one cannot stand in for
// another; what an id parameter is follows from the function's family.
HANDLES :: []string{"Image", "Font", "Sound", "Fragment", "Playback"}

// -- Typed parameters, returns and struct fields ----------------------------

// "fn.param", "*.param" for every function, "fn.return", or "Struct.field"
// (the C struct name without Kui) -> the Odin type. A handle, an enum from
// ENUMS, Color, Mods, bool or int.
TYPED :: [][2]string {
	{"kui_announce.live", "Live"},
	{"kui_token_color.out", "Color"},
	{"*.accent", "Color"},
	{"*.color", "Color"},
	{"*.cursor_color", "Color"},
	{"*.playback", "Playback"},
	{"*.sound", "Sound"},
	{"*.fill_rule", "Fill_Rule"},
	{"*.sampling", "Sampling"},
	{"*.fit", "Image_Fit"},
	{"*.cursor_shape", "Cell_Cursor"},
	{"*.appearance", "Appearance"},
	{"*.motion", "Motion"},
	{"*.assistive", "Assistive"},
	{"*.backdrop", "Backdrop"},
	{"*.option_as_alt", "Option_As_Alt"},
	{"*.marks", "Paste_Marks"},
	{"kui_input_modifiers.mods", "Mods"},
	{"kui_input_key.key", "Edit_Key"},
	{"kui_input_key.mods", "Edit_Mods"},
	{"kui_input_mouse_button.button", "Mouse"},
	{"kui_input_access.action", "Access_Action"},
	{"kui_input_access_text.action", "Access_Action"},
	{"*.kmods", "Key_Mods"},
	{"kui_latency_hud.x", "Align"},
	{"kui_latency_hud.y", "Align"},
	{"kui_window_dismissed.reason", "Dismiss_Reason"},
	{"kui_env_set_audio.device", "Audio_Device"},
	{"kui_note_frame_cause.cause", "Frame_Causes"},
	{"kui_fragment_with.image", "Image"},
	{"kui_fragment_open_with.image", "Image"},
	{"kui_cursor_shape.return", "Cursor"},
	{"kui_option_as_alt_get.return", "Option_As_Alt"},
	{"kui_request_copy.return", "Copy_Answer"},
	{"kui_frame_cause.return", "Frame_Causes"},
	{"kui_owed.return", "Owed"},
	{"kui_play.return", "Playback"},
	{"kui_take_file_request.mode", "File_Dialog_Mode"},
	{"kui_menu_bar_item.role", "Menu_Role"},
	{"kui_menu_bar_item.flags", "Menu_Item_Flags"},
	{"kui_menu_item.role", "Menu_Role"},
	{"kui_menu_item.flags", "Menu_Item_Flags"},
	{"kui_menu_bar_item_path.role", "Menu_Role"},
	{"kui_menu_bar_item_path.flags", "Menu_Item_Flags"},
	{"kui_menu_item_path.role", "Menu_Role"},
	{"kui_menu_item_path.flags", "Menu_Item_Flags"},
	{"kui_ctx_backdrop.return", "Backdrop"},
	{"AccessNode.role", "Role"},
	{"AccessNode.flags", "Access_Flags"},
	{"AccessNode.actions", "Access_Actions"},
	{"AccessNode.orientation", "Orientation"},
	{"Announcement.live", "Live"},
	{"MenuItem.role", "Menu_Role"},
	{"MenuAction.kind", "Menu_Action_Kind"},
	{"WindowConfig.kind", "Window_Kind"},
	{"WindowCommand.kind", "Window_Cmd"},
	{"AudioCommand.kind", "Audio_Cmd"},
	{"AudioCommand.playback", "Playback"},
	{"AudioCommand.sound", "Sound"},
	{"Audio.src", "Sound"},
	{"Theme.appearance", "Appearance"},
	{"FileDialog.mode", "File_Dialog_Mode"},
	{"ColorOp.op", "Color_Op_Kind"},
	{"TextureDraw.image", "Image"},
	{"FragmentDraw.fragment", "Fragment"},
	{"FragmentDraw.image_source", "Fragment_Image"},
}

// uint32_t fields that are a yes or no.
BOOL_FIELDS :: []string{"looped", "paused", "finish", "checked", "multiple", "monospaced", "italic", "rtl"}

// A yes-or-no whose C zero is the unusual case: an Odin literal leaves a
// field it does not name at zero, so the field is turned around, and a menu
// row is enabled unless it says otherwise.
INVERTED :: [][2]string {
	{"MenuItem.enabled", "disabled"},
	{"Menu.enabled", "disabled"},
}

// What a function's `id` (and its u64 return) is, by the function's
// family: kui_image_* and kui_image take an Image.
handle_of_family :: proc(c_name: string) -> string {
	n := c_name[len("kui_"):]
	switch {
	case has_word_prefix(n, "image"):
		return "Image"
	case has_word_prefix(n, "font"):
		return "Font"
	case has_word_prefix(n, "sound"):
		return "Sound"
	case has_word_prefix(n, "fragment"):
		return "Fragment"
	}
	return ""
}

// -- Structs with an Odin type of their own ---------------------------------

// C structs the hand-written files give an Odin twin of the same layout
// (each pinned by #asserts there), so they cross by a cast.
SAME_LAYOUT :: [][2]string {
	{"KuiSizing", "Size"},
	{"KuiKeyframe", "Keyframe"},
	{"KuiEnter", "Enter"},
	{"KuiGradientStop", "Gradient_Stop"},
	{"KuiSpan", "Span"},
	{"KuiCell", "Cell"},
	{"KuiRunConfig", "Run_Config"},
	{"KuiQuad", "Quad"},
}

// Hand-written types that cross to C as a u32 by transmute: the key
// modifiers word, a bit_field over KUI_KMOD_*, KUI_KLOCK_* and KUI_KLOC_*.
TRANSMUTED :: []string{"Key_Mods"}

// And the ones lowered by a procedure: Spec and Text_Style are generated
// from the schema below, Gradient is hand-written.
CONVERTED :: [][2]string {
	{"KuiSpec", "Spec"},
	{"KuiTextStyle", "Text_Style"},
	{"KuiGradient", "Gradient"},
}

// A mirror's name when its C name, Ada-cased, is taken: an event the core
// emits is `Menu_Event`, so the menu bar's menu is a Bar_Menu.
STRUCT_NAMES :: [][2]string {
	{"KuiMenu", "Bar_Menu"},
}

// -- The schema rows ----------------------------------------------------------

// A prop whose value is a string in the other bindings but a set of bits
// in C.
PROP_TYPES :: [][2]string {
	{"buttons", "Mouse_Buttons"},
	{"scrollMods", "Mods"},
	{"font", "Font"},
	{"clickSound", "Sound"},
	{"hoverSound", "Sound"},
}

// A row with its own lowering: Odin statements over `s` (the Spec or
// Text_Style) and `out` (the C struct). Every other row is the C field of
// its snake_case name, converted by its kind; a row in neither stops the
// generator. `{f}` is the row's Odin field.
LOWERING :: [][2]string {
	{"minWidth", "lower_min(s.{f}, &out.min_w, &out.min_w_size)"},
	{"maxWidth", "lower_max(s.{f}, &out.max_w, &out.max_w_size)"},
	{"minHeight", "lower_min(s.{f}, &out.min_h, &out.min_h_size)"},
	{"maxHeight", "lower_max(s.{f}, &out.max_h, &out.max_h_size)"},
	{"center", "if s.{f} do out.main_align, out.cross_align = c.CENTER, c.CENTER"},
	{"ruleWidth", "out.rule_w = s.{f}"},
	{"window", "out.window_role = u32(s.{f})"},
	{"transition", "out.transition_ms = s.{f}"},
	{"delay", "out.delay_ms = s.{f}"},
	{"underline", "if s.{f} do out.decoration |= c.DECO_UNDERLINE"},
	{"strikethrough", "if s.{f} do out.decoration |= c.DECO_STRIKETHROUGH"},
	{"caretSolid", "if s.{f} do out.value_set |= c.VALUE_CARET_SOLID"},
	{"opacity", "if v, ok := s.{f}.?; ok do out.opacity_set, out.opacity = 1, v"},
	{"valueNow", "if v, ok := s.{f}.?; ok do out.value_set, out.value_now = out.value_set | c.VALUE_NOW, v"},
	{"valueMin", "if v, ok := s.{f}.?; ok do out.value_set, out.value_min = out.value_set | c.VALUE_MIN, v"},
	{"valueMax", "if v, ok := s.{f}.?; ok do out.value_set, out.value_max = out.value_set | c.VALUE_MAX, v"},
	{"valueStep", "if v, ok := s.{f}.?; ok do out.value_set, out.value_step = out.value_set | c.VALUE_STEP, v"},
	{"caret", "if v, ok := s.{f}.?; ok do out.value_set, out.caret = out.value_set | c.VALUE_CARET, u32(v)"},
	{"selectionAnchor", "if v, ok := s.{f}.?; ok do out.value_set, out.selection_anchor = out.value_set | c.VALUE_ANCHOR, u32(v)"},
	{"radiusTL", "lower_corner(s.{f}, s.radius, &out.radius_tl, &out.per_corner)"},
	{"radiusTR", "lower_corner(s.{f}, s.radius, &out.radius_tr, &out.per_corner)"},
	{"radiusBR", "lower_corner(s.{f}, s.radius, &out.radius_br, &out.per_corner)"},
	{"radiusBL", "lower_corner(s.{f}, s.radius, &out.radius_bl, &out.per_corner)"},
}

// What a lowering helper (types.odin) writes, which a scan of the
// lowering's text cannot see: the coverage check counts these.
HELPER_WRITES :: [][2]string {
	{"lower_float", "float_mode float_anchor_x float_anchor_y float_self_x float_self_y float_dx float_dy float_fit float_clip"},
}

// Rows whose Odin type is a Maybe: their C side says "unset" with a bit or
// a flag rather than a zero, so `opacity = 0` must stay expressible.
MAYBE_ROWS :: []string{"opacity", "valueNow", "valueMin", "valueMax", "valueStep", "caret", "selectionAnchor", "radiusTL", "radiusTR", "radiusBR", "radiusBL"}

// Rows whose C side is an integer though the schema's kind is a number.
INT_ROWS :: []string{"maxLines", "caret", "selectionAnchor"}

// The CUSTOM rows, which every binding spells by hand. A row here is a
// field of Spec or Text_Style (Odin declaration, lowering), or a verb
// rather than a node prop, with the door that has it.
Custom_Rule :: struct {
	name:   string,
	target: string, // spec | style | verb
	decl:   string,
	lower:  string,
	doc:    string,
}

CUSTOM_RULES :: []Custom_Rule {
	{"dir", "spec", "dir: Dir", "out.dir = u32(s.dir)", "Column (the zero value), row, or table."},
	{"pad", "spec", "pad: Pad", "out.pad_l, out.pad_r, out.pad_t, out.pad_b = s.pad.l, s.pad.r, s.pad.t, s.pad.b", "Padding per side: pad(16), pad(16, 8), or {l = .., t = ..}."},
	{"border", "spec", "border_w: f32,\n\tborder_color: Color", "out.border_w, out.border_color = s.border_w, s.border_color", "The border's width, and its colour."},
	{"overflow", "spec", "overflow: Overflow", "out.overflow = transmute(u32)s.overflow", "{.Clip}, {.Scroll_X}, {.Scroll_Y}: what overflows the box is cut, or scrolls."},
	{"float", "spec", "float: Float", "lower_float(s.float, out)", "Out of flow: placed against the parent or the viewport (float_preset fills one by name)."},
	{"tooltip", "spec", "tooltip: string", "out.tooltip = s.tooltip", "A hint floated below the node while it is hovered, and its accessible description."},
	{"key", "spec", "key: string", "", "A stable key (kui_open_keyed): what transitions, focus and layout events follow; a node with on_drag, on_key or on_hover needs one."},
	{"index", "spec", "index: Maybe(u64)", "", "A virtual list row's index (kui_open_indexed), a namespace apart from `key`."},
	{"keyFocus", "spec", "key_focus: bool", "", "Declare this node keyboard-focused this frame (kui_set_key_focus), edge-triggered."},
	{"rowCount", "spec", "row_count: Maybe(u64)", "", "How many rows this virtual list has, built or not (kui_row_count)."},
	{"size", "style", "size: f32", "out.size = s.size", "The text's size in logical px; zero is the theme's."},
	{"title", "verb", "", "", "kui_window_title"},
	{"alwaysOnTop", "verb", "", "", "kui_set_always_on_top"},
	{"secureInput", "verb", "", "", "kui_set_secure_input"},
	{"optionAsAlt", "verb", "", "", "kui_set_option_as_alt"},
	{"imeOff", "verb", "", "", "kui_set_ime_off"},
	{"windows", "verb", "", "", "kui_window_declare"},
}

// -- Doors --------------------------------------------------------------------

// C functions with no generated door and no hand-written one, each with
// why: what calls it instead.
SKIP :: [][2]string {
	{"kui_button", "kui.button lowers to kui_button_with, which with an empty spec is kui_button"},
	{"kui_run", "kui.run lowers to kui_run_with, which with no context and no config is kui_run"},
}

// A parameter's default, which makes it and everything after it optional:
// a cells grid without a cursor is the common one.
DEFAULTS :: [][2]string {
	{"kui_cells.cursor_row", "0"},
	{"kui_cells.cursor_col", "0"},
	{"kui_cells.cursor_shape", ".Unset"},
	{"kui_cells.cursor_color", "0"},
	{"kui_cells.origin_line", "0"},
	{"kui_latency_hud.x", ".End"},
	{"kui_latency_hud.y", ".Start"},
}

// A struct parameter kui.h says may be NULL, and what NULL means there: a
// Maybe, nil by default.
NULLABLE :: []string{"kui_theme_set.theme", "kui_metrics_set.metrics", "kui_window_declare.cfg", "kui_play.opts"}

// A field whose C zero is not the default its INIT macro gives: a Maybe in
// the mirror, the INIT value when it is nil. A window activates by
// KUI_WINDOW_CONFIG_INIT and a popup does not, by KUI_WINDOW_POPUP_INIT.
FIELD_DEFAULTS :: [][2]string {
	{"Play.volume", "1"},
	{"Audio.volume", "1"},
	{"WindowConfig.activates", "v.kind != .Popup"},
}

// A pointer whose count is not the field after it: an access run's three
// per-character arrays share char_count, which comes before them.
SLICE_COUNTS :: [][2]string {
	{"AccessRun.char_lengths", "char_count"},
	{"AccessRun.char_positions", "char_count"},
	{"AccessRun.char_widths", "char_count"},
	{"AccessRun.word_starts", "word_start_count"},
}

// Arrays whose count is in points, not in floats: kui.h's `const float *xy,
// size_t count` is x0, y0, x1, y1 ... with count the number of points. The
// door takes [][2]f32 - the same memory - so the count is its length and a
// half point cannot be written. (Lowered as floats, the call claimed twice
// its points and the core read past the array; the scene corpus found it.)
POINTS :: []string{"kui_polyline.xy", "kui_polygon.xy"}

// A float pointer with no count is a fixed shape the call reads whole or
// takes NULL for: a fixed array in a Maybe, so a short one cannot be
// written. (As a slice, `dash = {4, 2}` let the core read three floats
// past it.)
FIXED_FLOATS :: [][2]string {
	{"kui_polyline.dash", "5"},
	{"kui_path.pivot", "2"},
	{"kui_path.dash", "5"},
	{"kui_path_d.pivot", "2"},
	{"kui_path_d.dash", "5"},
}

// Pixels the call reads by the size beside them: a slice, asserted to hold
// that many bytes (nil passes, as NULL, which the call refuses).
BYTE_LENGTHS :: [][2]string {
	{"kui_image_add.rgba", "int(w) * int(h) * 4"},
	{"kui_image_update.rgba", "int(w) * int(h) * 4"},
	{"kui_set_icon.rgba", "int(width) * int(height) * 4"},
}

// Doors that open a node the caller closes: generated with
// @(deferred_in), so `if kui.fragment_open(...) { ... }` closes itself.
OPENERS :: []string{"kui_fragment_open", "kui_fragment_open_with"}
