package kui

// What the core's events carry (kui_core::schema::EVENTS), ADR 0047.
//
// An event arrives as the message its handler declared: a node that
// declares on_drag = Pan{} hears a drag as {kind = "pan", drag = {phase, x,
// y, dx, dy, parent}}, the app's own message with the event's data in one
// field named for the event. Ev.event says which event it was. So there are
// two halves here:
//
//   - X_Event, the data: what rides under the field (Drag_Event is
//     {phase, x, y, dx, dy, parent, ...}). The events no node has arrive
//     the same way (ADR 0048), as the messages the window declared: its
//     root's Window_Events (root_with) for a resize, its keyboard focus,
//     the OS's settings, the fonts, the modifiers and the documents; its
//     entry in the window set (window_declare_with) for its opening,
//     closing and dismissal; the menu bar (menu_bar_with) for its choice.
//   - With_X, the field alone: With_Drag :: struct {drag: Drag_Event}. An
//     app's message embeds it, and kui.decode fills it:
//
//         Pan    :: struct {using _: kui.With_Drag}       // {kind = "pan"}
//         Rename :: struct {id: int, using _: kui.With_Edit_Change}
//         Msg    :: union {Pan, Rename}
//
//         kui.open(ui, "canvas", {}, on_drag = Pan{})     // declared bare:
//                                                         // the field is the core's
//         switch m in kui.message(ev, Msg) {
//         case Pan:    s.x = s.x0 + m.drag.dx
//         case Rename: rename(s, m.id, m.change.text)
//         }
//
//     A field named `_` is never written into a message, so the With_X a
//     message embeds costs the declaration nothing.
//
// One on_key sink hears key, text, preedit and select events: its messages
// embed the field each handles, and Ev.event tells them apart. A message
// declared as kui.NO_TAG (no message at all) arrives as the field alone, which a With_X decodes directly: kui.message(ev, kui.With_Key). A
// submit, a cancel and a plain click carry no data and arrive exactly as
// declared. The generator checks every event has its X_Event, and its With_X
// where it rides in a field, holding every field the schema says it carries.

// A point in a rect's coordinates, as several payloads carry one.
Box :: struct {
	x, y, w, h: f32,
}

Cell_Position :: struct {
	row, col: int,
}

Text_Position :: struct {
	line, offset: int,
}

// A click that can say more (ADR 0047, decision 2): on a cells grid, or
// inside an on_key sink that draws line rows. A plain click has none.
Click_Event :: struct {
	cell:   Maybe(Cell_Position), // on a cells grid
	line:   Maybe(int),
	byte:   Maybe(int),
	inside: Maybe(bool), // on the line's own box, not the margin beside it
	clicks: Maybe(int),
}

Drag_Event :: struct {
	phase:        string, // "start" | "move" | "end"
	x, y, dx, dy: f32, // dx, dy: from the press point, in every phase
	parent:       Box,
	cell:         Maybe(Cell_Position), // on a cells grid
	line:         Maybe(int), // inside a sink that draws line rows
	byte:         Maybe(int),
	inside:       Maybe(bool),
	clicks:       Maybe(int),
}

Key_Event :: struct {
	phase:                   string, // "down" | "up"
	code:                    string, // a character or a name: "left", "f5"
	physical:                string, // the US-QWERTY key at that position
	shift, ctrl, alt, super: bool,
	text:                    string, // what the press inserts; empty on a release
	repeat:                  bool,
	location:                string, // "standard" | "left" | "right" | "numpad"
	caps_lock, num_lock:     bool,
}

Text_Event :: struct {
	text:      string,
	pasted:    bool,
	concealed: bool,
	transient: bool,
}

Preedit_Event :: struct {
	text:   string,
	cursor: Maybe([2]int), // the IME's caret, a byte range in text
}

// Assistive technology setting a custom editor's selection, on its on_key.
Select_Event :: struct {
	anchor, focus: Text_Position,
}

Selection_Point :: struct {
	index: i64,
	byte:  int,
}

Selection_Range_Event :: struct {
	from, to: Selection_Point,
}

Context_Menu_Event :: struct {
	x, y: f32,
}

// A menu row chosen: under `menu` in the on_menu message, a node's or the
// menu bar's (menu_bar_with).
Menu_Event :: struct {
	role: string,
	item: Value,
}

Force_Click_Event :: struct {
	x, y: f32,
}

Button_Event :: struct {
	phase:  string, // "press" | "move" | "release"
	button: Value, // "secondary" | "middle" | a further button's number
	x, y:   f32,
	clicks: int,
	cell:   Maybe(Cell_Position), // on a cells grid
	line:   Maybe(int),
	byte:   Maybe(int),
	inside: Maybe(bool), // on the line's own box, not the margin beside it
}

Held_Mods :: struct {
	shift, ctrl, alt, super: bool,
}

Scroll_Event :: struct {
	x, y, dx, dy: f32,
	lines:        Maybe(f64), // on a cells grid
	mods:         Maybe(Held_Mods), // on a scrollMods node
}

Focus_Event :: struct {
	phase: string, // "in" | "out"
	by:    string, // "pointer" | "keyboard" | "assistive" | "program"
}

Hover_Event :: struct {
	phase: string, // "enter" | "leave"
	by:    string, // "pointer" | "content"
}

Drop_Event :: struct {
	phase: string, // "enter" | "move" | "leave" | "drop"
	paths: []string,
	x, y:  f32,
}

// A file dialog's answer, in the message request_files was given.
Files_Event :: struct {
	paths: []string,
}

Layout_Event :: struct {
	x, y, w, h: f32,
	parent:     Box,
	scale:      f32,
}

// A slider's proposed value, under `change` in its on_change message.
// The data rows in an on_range node's view (docs/adr/0065): the least and
// greatest data index among its children inside its box.
Range_Event :: struct {
	first, last: int,
}

Slider_Change_Event :: struct {
	value: f64,
	phase: string, // "move" | "end"
}

// An editor's text after an edit, under `change` in its on_change message.
// A secure editor's too.
Edit_Change_Event :: struct {
	text: string,
}

// An editing key that met the edge of an editor's text and did nothing
// (backlog F145): ↑ on the first line, ← or Backspace at the start, ↓ on the
// last, → or Delete at the end.
Boundary_Event :: struct {
	key:       string, // "up" | "down" | "left" | "right" | "backspace" | "delete"
	edge:      string, // "start" | "end"
	word, doc: bool, // the editing modifiers the key carried
}

Sound_Event :: struct {
	phase:    string, // "ended" | "refused"
	playback: Playback,
}

// A still or a recording the app asked for with capture_to / record_to,
// written or not (ADR 0055), in the message the verb named.
Capture_Event :: struct {
	phase:   string, // "saved" | "failed"
	path:    string,
	width:   u32, // physical px
	height:  u32,
	frames:  u32, // 1 for a still; the frames drawn while recording
	seconds: f64, // a recording's length; 0 for a still
	error:   string, // why a "failed" one failed; "" when saved
}

Dismiss_Event :: struct {
	reason: string, // "escape" | "outside"
	id:     u32, // a popup window's, in its on_dismiss; 0 on a modal node
}

// -- The window's own events (ADR 0048) -----------------------------------------

// The documents the OS asked the app to open, in the main window's on_open.
Open_Event :: struct {
	paths: []string,
}

Resize_Event :: struct {
	width, height, scale: f32,
}

// A declared window's opening and closing, in its on_window; this
// window's keyboard, in its root's on_focus. The message names the window.
Window_Event :: struct {
	phase: string, // "opened" | "closed" | "focused" | "blurred"
	id:    u32, // 0 for focused and blurred
}

System_Event :: struct {
	appearance: string,
	accent:     Value,
	motion:     string,
	locale:     string,
	assistive:  string,
}

Fonts_Event :: struct {}

Modifiers_Event :: struct {
	shift, ctrl, alt, super: bool,
}

// -- The fields: what an app's message embeds ----------------------------------

With_Click :: struct {
	click: Click_Event,
}

With_Drag :: struct {
	drag: Drag_Event,
}

With_Key :: struct {
	key: Key_Event,
}

With_Text :: struct {
	text: Text_Event,
}

With_Preedit :: struct {
	preedit: Preedit_Event,
}

With_Select :: struct {
	select: Select_Event,
}

With_Selection_Range :: struct {
	selectionrange: Selection_Range_Event,
}

With_Context_Menu :: struct {
	contextmenu: Context_Menu_Event,
}

With_Menu :: struct {
	menu: Menu_Event,
}

With_Force_Click :: struct {
	forceclick: Force_Click_Event,
}

With_Button :: struct {
	button: Button_Event,
}

With_Scroll :: struct {
	scroll: Scroll_Event,
}

With_Focus :: struct {
	focus: Focus_Event,
}

With_Hover :: struct {
	hover: Hover_Event,
}

With_Drop :: struct {
	drop: Drop_Event,
}

With_Files :: struct {
	files: Files_Event,
}

With_Layout :: struct {
	layout: Layout_Event,
}

With_Range :: struct {
	range: Range_Event,
}

With_Slider_Change :: struct {
	change: Slider_Change_Event,
}

With_Edit_Change :: struct {
	change: Edit_Change_Event,
}

With_Boundary :: struct {
	boundary: Boundary_Event,
}

With_Sound :: struct {
	sound: Sound_Event,
}

With_Capture :: struct {
	capture: Capture_Event,
}

With_Dismiss :: struct {
	dismiss: Dismiss_Event,
}

With_Resize :: struct {
	resize: Resize_Event,
}

With_Window :: struct {
	window: Window_Event,
}

With_System :: struct {
	system: System_Event,
}

With_Modifiers :: struct {
	modifiers: Modifiers_Event,
}

With_Open :: struct {
	open: Open_Event,
}
