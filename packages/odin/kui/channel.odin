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

// Doubles a bucket of channel_reduce takes: the least, greatest, first and
// last value, then the row each was read from.
CHANNEL_BUCKET :: c.CHANNEL_BUCKET

// Column col over rows [from, to) in at most len(out) / CHANNEL_BUCKET
// buckets (backlog C95), CHANNEL_BUCKET doubles each. count is how many
// were written; ok is false for no column col or a column that is not
// numbers, the reason in channel_error.
channel_reduce :: proc(ch: ^c.Channel, col, from, to: int, out: []f64) -> (count: int, ok: bool) {
	n: uint
	ok = c.channel_reduce(ch, uint(col), uint(from), uint(to), uint(len(out) / CHANNEL_BUCKET), (^f64)(raw_data(out)), &n)
	count = int(n)
	return
}

// The next revision of ch's identity: its rows, then the builder's (backlog
// C147), each of ch's columns named, of its kind and all of one length; with
// cap above 0 the oldest rows past it go. The builder is consumed either way;
// ch is untouched. nil on a mismatch, the reason in channel_error.
// Rust: Channel::append.
channel_append :: proc(ch: ^c.Channel, rows: ^c.ChannelBuilder, cap := 0) -> ^c.Channel {
	return c.channel_append(ch, rows, uint(cap))
}

// A string filter's way: the cell is the text, or holds it (its letters'
// case as given, or any).
Channel_Match :: enum i32 {
	Equal         = c.CHANNEL_EQUAL,
	Contains      = c.CHANNEL_CONTAINS,
	Contains_Any_Case = c.CHANNEL_CONTAINS_ANY_CASE,
}

// The rows - all, or within's in its order - whose number in column col is
// at least min and at most max (min == max for one value, math.INF_F64 for an
// open end; NaN is in no range), written into out, which has room for the
// channel's length or len(within) (backlog C148). ok is false for no column
// col, a column that is not numbers, or a row past the end.
// Rust: Channel::filter.
channel_filter_f64 :: proc(ch: ^c.Channel, col: int, min, max: f64, out: []uint, within: []uint = nil) -> (rows: []uint, ok: bool) {
	n: uint
	ok = c.channel_filter_f64(ch, uint(col), min, max, raw_data(within), uint(len(within)), raw_data(out), &n)
	return out[:n] if ok else nil, ok
}

// channel_filter_f64 for a string column: the rows whose cell is text, or
// holds it.
channel_filter_str :: proc(ch: ^c.Channel, col: int, text: string, how: Channel_Match, out: []uint, within: []uint = nil) -> (rows: []uint, ok: bool) {
	n: uint
	ok = c.channel_filter_str(ch, uint(col), text, i32(how), raw_data(within), uint(len(within)), raw_data(out), &n)
	return out[:n] if ok else nil, ok
}

// The rows - all, or within's - ordered by column col, least first or
// greatest with descending; ties keep their order, NaN goes last.
// Rust: Channel::sort.
channel_sort :: proc(ch: ^c.Channel, col: int, out: []uint, descending := false, within: []uint = nil) -> (rows: []uint, ok: bool) {
	n: uint
	ok = c.channel_sort(ch, uint(col), descending, raw_data(within), uint(len(within)), raw_data(out), &n)
	return out[:n] if ok else nil, ok
}

// A new channel of rows, in that order, yours until channel_free; nil for a
// row past the end.
// Rust: Channel::take.
channel_take :: proc(ch: ^c.Channel, rows: []uint) -> ^c.Channel {
	return c.channel_take(ch, raw_data(rows), uint(len(rows)))
}

// Doubles a run of channel_lanes_window takes: its start, end, row (-1 for a
// strip of several) and how many intervals it holds.
CHANNEL_RUN :: c.CHANNEL_RUN

// A channel's intervals on lanes (ADR 0075): column start to start + len
// (len -1: points) on the lanes column lane names (-1: one lane), built once;
// nil for a column that is not numbers. Free it with channel_lanes_free.
// Rust: Channel::lanes.
channel_lanes :: proc(ch: ^c.Channel, start: int, len := -1, lane := -1) -> ^c.ChannelLanes {
	return c.channel_lanes(ch, uint(start), i32(len), i32(lane))
}

channel_lanes_free :: proc(l: ^c.ChannelLanes) {
	c.channel_lanes_free(l)
}

channel_lanes_count :: proc(l: ^c.ChannelLanes) -> int {
	return int(c.channel_lanes_count(l))
}

channel_lanes_extent :: proc(l: ^c.ChannelLanes) -> (lo, hi: f64, ok: bool) {
	ok = c.channel_lanes_extent(l, &lo, &hi)
	return
}

// Lane lane's runs reaching into [v0, v1] at the detail where an interval at
// least min long is a run of its own, CHANNEL_RUN doubles each, as many as
// out has room for; count is how many were written.
channel_lanes_window :: proc(l: ^c.ChannelLanes, lane: int, v0, v1, min: f64, out: []f64) -> (count: int, ok: bool) {
	n: uint
	ok = c.channel_lanes_window(l, uint(lane), v0, v1, min, (^f64)(raw_data(out)), uint(len(out) / CHANNEL_RUN), &n)
	count = int(n)
	return
}
