package kui

// What the core emits on its own behalf (kui_core::schema::EVENTS), one
// struct per kind, ready to sit in an app's message union beside its own
// types: X_Event decodes the kind "x". The generator checks every kind has
// one here and every field the schema says the payload carries is a field
// of it. `tag` is the message the handler was declared with: decode it with
// kui.decode(m.tag, &msg). A click carries the app's own payload, so it
// has none.

// A point in a rect's coordinates, as several payloads carry one.
Box :: struct {
	x, y, w, h: f32,
}

Drag_Event :: struct {
	phase:        string, // "start" | "move" | "end"
	x, y, dx, dy: f32, // dx, dy: from the press point, in every phase
	parent:       Box,
	tag:          Value,
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
	tag:                     Value,
}

Text_Event :: struct {
	text:      string,
	pasted:    bool,
	concealed: bool,
	transient: bool,
	tag:       Value,
}

Preedit_Event :: struct {
	text:   string,
	cursor: Maybe([2]int), // the IME's caret, a byte range in text
	tag:    Value,
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
	tag:  Value,
}

Menu_Event :: struct {
	role: string,
	item: Value,
}

Force_Click_Event :: struct {
	x, y: f32,
	tag:  Value,
}

Cell_Position :: struct {
	row, col: int,
}

Button_Event :: struct {
	phase:  string, // "press" | "move" | "release"
	button: Value, // "secondary" | "middle" | a further button's number
	x, y:   f32,
	clicks: int,
	cell:   Maybe(Cell_Position), // on a cells grid
	line:   Maybe(int),
	byte:   Maybe(int),
	tag:    Value,
}

Held_Mods :: struct {
	shift, ctrl, alt, super: bool,
}

Scroll_Event :: struct {
	x, y, dx, dy: f32,
	lines:        Maybe(f64), // on a cells grid
	mods:         Maybe(Held_Mods), // on a scrollMods node
	tag:          Value,
}

Focus_Event :: struct {
	phase: string, // "in" | "out"
	by:    string, // "pointer" | "keyboard" | "assistive" | "program"
	tag:   Value,
}

Hover_Event :: struct {
	phase: string, // "enter" | "leave"
	by:    string, // "pointer" | "content"
	tag:   Value,
}

Drop_Event :: struct {
	phase: string, // "enter" | "move" | "leave" | "drop"
	paths: []string,
	x, y:  f32,
	tag:   Value,
}

Files_Event :: struct {
	paths: []string,
	tag:   Value,
}

Open_Event :: struct {
	paths: []string,
}

Layout_Event :: struct {
	x, y, w, h: f32,
	parent:     Box,
	scale:      f32,
	tag:        Value,
}

Resize_Event :: struct {
	width, height, scale: f32,
}

Window_Event :: struct {
	phase: string, // "opened" | "closed" | "focused" | "blurred"
	name:  string,
	id:    u32,
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

Changed_Event :: struct {}

Submit_Event :: struct {}

Sound_Event :: struct {
	phase:    string, // "ended" | "refused"
	playback: Playback,
	tag:      Value,
}

Dismiss_Event :: struct {
	reason: string, // "escape" | "outside"
	tag:    Value,
	// On the root, for a popup window.
	name:   string,
	id:     u32,
}

Text_Position :: struct {
	line, offset: int,
}

Access_Event :: struct {
	action:         string,
	tag:            Value,
	text:           string,
	value:          Value,
	anchor, focus:  Maybe(Text_Position),
}

Change_Event :: struct {
	value: f64,
	phase: string, // "move" | "end"
	tag:   Value,
}
