package kui

import c "c"

// Channels (docs/adr/0065-a-data-channel.md): bulk data the core owns and a
// handle names. Build one a column at a time and hold the ^c.Channel for as
// long as the data should live; channel_handle makes the value that names it
// for a component's props or a message, and a reader resolves a handle it
// was given to read by row and column, never copying the whole.
//
//     b := kui.channel_new()
//     kui.channel_add_str(b, "name", names[:])
//     kui.channel_add_f64(b, "ms", ms[:])
//     ch := kui.channel_finish(b)   // nil, with channel_error, on a mismatch
//     defer kui.channel_free(ch)
//     kui.component(ui, "kit/table", Table_Props{rows = kui.channel_handle(ch)})
//
// The builder and the channel are opaque C types, so these are written here
// rather than generated.

// A builder with no columns; channel_finish consumes it.
// Rust: Channel::table.
channel_new :: proc() -> ^c.ChannelBuilder {
	return c.channel_new()
}

// Adds a column, copied.
channel_add_f64 :: proc(b: ^c.ChannelBuilder, name: string, values: []f64) -> bool {
	return c.channel_add_f64(b, name, raw_data(values), uint(len(values)))
}

channel_add_i64 :: proc(b: ^c.ChannelBuilder, name: string, values: []i64) -> bool {
	return c.channel_add_i64(b, name, raw_data(values), uint(len(values)))
}

channel_add_str :: proc(b: ^c.ChannelBuilder, name: string, values: []string) -> bool {
	return c.channel_add_str(b, name, raw_data(values), uint(len(values)))
}

// The channel, yours until channel_free; the builder is consumed either way.
// nil for columns of two lengths or a name given twice, the reason in
// channel_error.
channel_finish :: proc(b: ^c.ChannelBuilder) -> ^c.Channel {
	return c.channel_finish(b)
}

// Why the last channel_finish on this thread answered nil.
channel_error :: proc() -> (why: string, ok: bool) #optional_ok {
	ok = c.channel_error(&why)
	return
}

// The channel a handle value names, held anew (free it), or nil.
channel_resolve :: proc(handle: ^c.Value) -> ^c.Channel {
	return c.channel_resolve(handle)
}

channel_free :: proc(ch: ^c.Channel) {
	c.channel_free(ch)
}

// A new value naming the channel, for props or a message; it does not hold
// the channel. Free it with value_free, or hand it to a door that consumes
// one.
channel_handle :: proc(ch: ^c.Channel) -> ^c.Value {
	return c.channel_handle(ch)
}

channel_len :: proc(ch: ^c.Channel) -> int {
	return int(c.channel_len(ch))
}

channel_rev :: proc(ch: ^c.Channel) -> u64 {
	return c.channel_rev(ch)
}

// The index of the column called name, or -1.
channel_column :: proc(ch: ^c.Channel, name: string) -> int {
	return int(c.channel_column(ch, name))
}

// A cell as a number (an integer widened).
channel_get_f64 :: proc(ch: ^c.Channel, row, col: int) -> (value: f64, ok: bool) #optional_ok {
	ok = c.channel_get_f64(ch, uint(row), uint(col), &value)
	return
}

// A cell as a string, borrowed while you hold ch.
channel_get_str :: proc(ch: ^c.Channel, row, col: int) -> (value: string, ok: bool) #optional_ok {
	ok = c.channel_get_str(ch, uint(row), uint(col), &value)
	return
}
