# Performance

`cargo bench -p kui-core`, measured 2026-09-28 on an Apple M3 Pro MacBook Pro
(macOS 27.0, rustc 1.98.0, release, steady-state warm caches — full frame:
build + layout + emit) for the `frame` rows, by the alpha.22 pre-tag
`scripts/bench-check.sh` run against the alpha.21 tag (worst guarded
spread 6.2%; five rows whose two runs disagreed by more than 5% —
`frame_1k_curves`, `copy_1080p_frame`, `update_image_1080p_and_frame`,
`frame_10k_rects_all_transitioning` and `list_100k_rows_virtual` — keep
their earlier numbers; `replay_a_full_depart_store` and
`drop_1k_rows_declaring_exit` moved with the exit budget, 512 to 4096
nodes, not with the code that replays it; the alpha.15 refresh of 2026-09-16 was the
first since the OS moved from 26.6.2, under which the 2026-09-15 numbers
were taken — alpha.13's own code read ~9% slower here, so the table's
absolute numbers moved with the OS and not the code); the `long_line`
rows are from the same day's `KUI_BENCH=long_line` run against alpha.21
(flat within noise), the `stream` and
`cells` rows from 2026-09-07, not re-run. The suite was run twice back to back and the second
run read; most rows reproduce to within ~3% that way. Two do not:
`frame_10k_rects_all_transitioning` and `frame_10k_rects_all_declaring_exit`
disagree with themselves by 5–6% run to run where every other row holds to
~3%, so their medians below should be read as **±10%**, not as three
significant figures. Nothing separated the passes — the same binary on mains
and on battery agrees to within 1%, and every other row held steady across all
of them. Measuring a row alone after an idle also reads lower than measuring it
inside the whole suite (`frame_10k_rects` ~715 µs against ~725 µs), so these
are the numbers the command above gives, which is the point of quoting them.
The grid benches all go through the same builder at 1920×1080 and differ only
in which props are switched on, so the difference between two of them is what
that prop costs.

| bench | what it holds | median |
|---|---|---|
| `frame_1k_typical` | 32×32 grid, every 8th cell a label, every 4th clickable — a "typical app" frame | ~123 µs |
| `frame_1k_typical_with_100_floats` | the typical frame with a hundred tooltips floating over it every frame — what a hundred layers on the float stack cost at rest (ADR 0023) | ~151 µs |
| `frame_10k_rects` | 100×100 plain rects, nothing switched on | ~742 µs |
| `frame_10k_rects_with_text_and_hits` | the same grid plus 1.2k texts and 2.5k hit regions | ~1.24 ms |
| `hover_over_10k_regions` | the cursor moving between two cells of the 10k-region frame, so the hovered node changes every move and the scan runs to its end (ADR 0026) | ~3.67 µs |
| `frame_10k_rects_with_access_tree` | that frame with `core.access_tree()` derived after it — what a frame costs while assistive technology is attached | ~1.39 ms |
| `frame_10k_rects_with_shadows_and_opacity` | the plain grid with only the paint props on: every cell casts a shadow under a faded root | ~827 µs |
| `frame_10k_rects_square_clip` | the plain grid with every row clipping, so all 10k cells inherit a clip | ~848 µs |
| `frame_10k_rects_rounded_clip` | the same with a radius on every clipping row, so each cell pays the per-corner intersect | ~845 µs |
| `frame_10k_segments` | 10k one-segment `line` floats — the same 10k quads as `frame_10k_rects`, so the gap between the two is what a segment costs over a box | ~846 µs |
| `frame_1k_curves` | 1k curves through eight knots each, re-flattened by chord length every frame — 35 segments a curve | ~246 µs |
| `frame_1k_polygons` | 1k six-point fills, one fragment quad each (ADR 0025) | ~97.5 µs |
| `frame_1k_closed_lines` | the same thousand outlines as closed strokes, six segment quads each | ~97.3 µs |
| `frame_1k_typical_with_8_textures` | `frame_1k_typical` plus eight texture-backed images, registered once and updated once, so each is its own texture and a side-list entry (ADR 0025) | ~128 µs |
| `update_image_1080p_and_frame` | replacing a 1080p frame — the `Vec` handoff, the revision bump, then the frame that draws it; the upload is the backend's (`benches/split.rs` in kui-wgpu under `TEX=1`) | ~136 µs |
| `copy_1080p_frame` | the app's own copy of that 1080p frame, measured beside it so the core's share of `update_image_1080p_and_frame` is the difference | ~139 µs |
| `update_image_1080p_recycled_and_frame` | the same frame copied through `update_image_with` into the buffer the core recycles, as the Node and C doors do: the memcpy and no allocation (backlog W20) | ~116 µs |
| `frame_10k_rects_all_transitioning` | every cell declares a `transition` — nine retained tween slots each | ~1.96 ms |
| `frame_10k_rects_all_declaring_exit` | every cell also declares an `exit`, so the whole frame is kept for the next one to diff against | ~2.46 ms |
| `frame_10k_rects_one_exit` | the same 10k grid with a single cell declaring an `exit` | ~752 µs |
| `drop_1k_rows_plain` | 1k rows removed from the tree in one frame, no exits declared | ~62.2 µs |
| `drop_1k_rows_declaring_exit` | the same removal with exits declared — under the 4096-node budget (512 until backlog DX23), so all 1000 are copied into the store | ~275 µs |
| `drop_5k_rows_declaring_exit` | 5000 rows with exits declared — over the budget, so ADR 0012 refuses it whole: the diff and the count, and no copies | ~817 µs |
| `drop_500_rows_declaring_exit` | 500 rows with exits declared, under the budget, so all 500 are copied into the store | ~136 µs |
| `replay_a_full_depart_store` | replaying a saturated depart store (the 4096-node budget; 512 until backlog DX23) for one frame | ~141 µs |
| `frame_10k_chips_unwrapped` | 10k chips in 100 rows, one line per row | ~658 µs |
| `frame_10k_chips_wrapped` | the same tree with every row breaking onto several lines | ~827 µs |
| `deep_nesting_64_levels` | 16 chains nested 64 levels deep | ~84.6 µs |
| `frame_1k_grow_rows_capped` | a column of 1k grow rows under a staircase of `max_height`s — four passes of the freeze loop with 348 rows frozen, what a pass costs over children the earlier passes settled (RG5) | ~75.2 µs |
| `list_10k_rows_naive` | a 10k-row list held at its middle, built row by row | ~3.83 ms |
| `list_10k_rows_virtual` | the same list through `widgets::uniform_list` | ~18.8 µs |
| `list_100k_rows_virtual` | 100k rows through the same widget | ~18.3 µs |
| `list_10k_rows_variable` | 10k rows of no fixed height through `widgets::list` — two searches and the build; its rows average 32 px against the uniform bench's 24, so fewer are on screen | ~14.7 µs |
| `list_10k_rows_variable_at_one_height` | the variable list told, row by row, that every row is the same height — against `list_10k_rows_virtual`, what the stride buys | ~18.9 µs |
| `list_10k_rows_variable_learning` | a frame that learns a visible row's height — what every frame of a scroll is, and what invalidates the prefix sums | ~14.7 µs |
| `list_10k_rows_variable_learning_far` | the same frame told about a row nowhere near the window, so everything between it and the window is summed again | ~20.1 µs |
| `list_100k_rows_variable` | an order of magnitude more variable rows, where an O(n) rebuild would show | ~14.9 µs |
| `list_100k_rows_variable_learning` | the same 100k list learning a visible row's height | ~14.7 µs |
| `warm_50x200` (`--bench stream`) | fifty 200-column mono lines, the same every frame — a terminal pane at rest | ~85 µs |
| `stream_50x200_log` | the same pane with every line new each frame, thirty-word log vocabulary plus numbers | ~25 ms |
| `stream_50x200_random` | every line new and random printable ASCII, nothing for the shape-run cache to hit | ~64 ms |
| `long_line_100k_first_frame` (`--bench long_line`) | a 100k-character no-wrap line opened in a horizontally scrolling view — shaped in chunks as they show | ~18 ms (was 662 ms whole) |
| `long_line_100k_scroll` | a viewport's width of scrolling through it per frame | ~20.9 µs |
| `long_line_100k_wrapped_first_frame` | the same line as a `wrap: word` paragraph in a vertically scrolling view — rows broken from the chunks' positions | ~33.7 ms |
| `long_line_100k_wrapped_scroll` | a viewport's height of scrolling through it per frame | ~30 µs, up to ~30 ms on a frame that brings a chunk in |
| `long_line_100k_edit` | two characters inserted in the middle, different every frame, the view held there — the chunk they land in reshaped | ~1.26 ms |
| `long_line_100k_rich_first_frame` | the same line as three spans — an editor's caret row — opened at its start | ~17.9 ms |
| `long_line_100k_rich_caret` | the caret span moved one character a frame along it | ~1.23 ms |
| `long_rows_34x500k_steady` | thirty-four rows of 500k characters each, every chunk shaped, nothing changing — what a screenful of a binary costs when it draws none of it | ~2.98 ms |
| `cells_200x50_warm` (`--bench cells`) | a terminal's screen as one `ui.cells` node, unchanged | ~58 µs |
| `cells_200x50_streaming` | the same grid with every character new each frame | ~60 µs |
| `cells_200x50_as_text_nodes` | the same 10k cells as one text node each — the path an app had | ~2.2 ms |

What the pairs say. Deriving the access tree costs **~1.10×** the frame it
follows — it was 1.35× before ADR 0016's one built decision, the digest
that skips the derivation when nothing it reads has changed. Shadows under
a faded root are **twice the quads** (20k against 10k)
for **~8%** more frame time, because most of a frame is build and layout
rather than emitting quads. Clipping costs ~11% over the unclipped grid and
rounding that clip costs ~4% more — the radius is nearly free once a node
clips at all (both a few points more than at alpha.10; backlog C29 has the
numbers). A `line` costs ~17% over a plain rect at the same 10k quads —
the line store, the float placement, the endpoint encoding, and since ADR
0023 a layer of its own on the float stack (C29 again; it was ~8%) — and
flattening
is cheaper than the node it hangs off: 1k eight-knot curves cut into ~35k
segments cost ~250 µs, under a third of the 10k-node segment grid, because
per-node work is most of what a frame is and 35 segments ride on one node.
Wrapping every row runs **~1.23×** the same tree laid out one
line per row, and that is the worst case: a row that does not wrap pays
nothing, because the break, the per-line grow and the per-line alignment are
all behind the flag. An exit on one node out of 10k costs ~2% over the plain
grid, so the `any_exit` gate holds — it is declaring exits on *every* node
that triples the frame. And virtualisation is the one difference worth
orders of magnitude: 10k rows cost ~4 ms built row by row and ~18 µs
through the widget, with 100k rows costing the same ~18 µs, because the frame
stops growing with the data. The two `stream` rows are the shaper's, not
the tree's: a pane whose fifty lines are all new every frame costs 25–64 ms
because each line is shaped from scratch, and cosmic-text alone on the same
fifty lines measures the same — a cell grid that never shapes ASCII is
backlog C20. What those frames leave behind is bounded: the shaped-text
cache holds a byte budget (`Core::set_text_cache_budget`, 64 MB by
default) and evicts the least recently drawn entries past it, never what
the last frame drew, so the stream that used to park 3.6 GB of shaped lines
settles at the budget (backlog C16, `tests/text_budget.rs`).

**The text rows measure the platform's font as much as the machine.**
`Mono` resolves to a face per platform: SF Mono on macOS, Cascadia Mono
on Windows, DejaVu Sans Mono on Linux (backlog C32). Shaping cost
follows the face, so the `stream` and `long_line` rows do not compare
across operating systems. `long_line_100k_first_frame` reads 18 ms on
the M3 Pro, 2.8 ms on a Ryzen 9 9950X3D under Windows and 0.77 ms on the
same Ryzen under Linux. The 23× spread is mostly the font, not the CPU.
That machine's rows at alpha.15 match HEAD's within 4%, and pinning
Consolas over Cascadia Mono moves them under 2%. Compare those rows
only within one OS. The `cells` rows shape no ASCII and agree
everywhere (55–60 µs).

**On other machines.** The same suite on that Ryzen (2026-09-26, rustc
1.98.1, Windows 11 on the Balanced power plan, and Ubuntu 24.04 under
WSL 2 with rustc 1.96.1): the plain frame rows read 0.88× the table's
medians under Windows and 0.81× under Linux (geometric mean of 27 rows).
Windows costs ~9% on the same CPU. Some rows read 1.3–1.75× slower
under Windows than under Linux: the exit-copying rows, the naive list,
the access tree and `hover_over_10k_regions`. The first three allocate
heavily, and the hover row's reason is not isolated. The 1080p image copy
was 7× slower, because Windows' heap faults in a fresh 8 MB block page
by page where macOS's and glibc's reuse the freed one. That is why a
stream should go through `update_image_with`, which recycles its buffer
(backlog W20). `frame_1k_curves` is the one row the M3 Pro wins on both.

**These numbers went the wrong way once, and this is where that is
recorded.** The four rows this table used to carry were measured on
2026-08-31 (`dabe671`) at ~70 µs, ~510 µs, ~740 µs and ~58 µs. Re-measuring
for the alpha.6 release found them 2.4–2.7× worse — a frame cost that had
grown a little at a time across ~10 feature commits, on benches that declare
none of the features. The cause was `NodeSpec`: it had reached **728 bytes**,
it is moved by value for every node a frame builds, and 31% of a frame was
going into `memmove`.

The cold fields are now behind four boxed groups (events, animation,
accessibility, hover styling), which puts `NodeSpec` at **224 bytes** and
`memmove` back below the profiler's noise floor. That recovered about
two-thirds of the regression — `frame_10k_rects` 1.37 ms → 788 µs — and every
bench in the table moved with it, including ones that touch none of those
fields. A second profile then went after the rest (2026-09-07): the passes
that had grown a per-node read for a feature the frame does not use — the
float, wrap and text walks in layout, the float check in emission, the
hover-style and transition early-outs in the builder — are now behind
tree-level flags or inlined branches, and the open chain carries a spec by
pointer instead of moving it at every call. That took `frame_10k_rects`
from 816 µs to 725 µs and the chips, clip and exit grids 10–14% with it. What
is still above the 2026-08-31 baseline (~1.4×) is measured and is not a pass:
a padded copy of the old struct at 224 bytes reproduces most of it, and the
profile puts it in the app's own builder chain, where a `NodeSpec` is
default-constructed and moved by value before the core sees it. C15 carries
the bisect, both profiles, the padding experiments and both fixes in
[docs/backlog/closed-2026-09.md](backlog/closed-2026-09.md).
`size_of::<NodeSpec>()` now has a
test with a bound on it, so the next inline field has a number to fail against
rather than a release audit to wait for. A fat struct is not the only way to
lose a frame, though, so there is a second guard for the case that test cannot
see: `nu scripts/bench-check.nu` benches HEAD against the previous `v*` tag in a
worktree and fails if one of eight frame benches is more than 10% slower
(`KUI_BENCH=stream` reads another bench file's rows, unjudged). It is
run before tagging rather than in CI — the docker runner is too weak to
measure a frame and would false-fail — and it prints the table above with the
run's own medians, so re-measuring these numbers is that same command.

A built-in latency graph shows per-phase frame cost live —
`widgets::latency_hud(ui)` floats it in a viewport corner as a translucent
overlay (`latency_hud_at` picks the corner; `latency_graph` is the inline
form): the last 120 frames as stacked bars (input / view / layout / render /
vsync wait) against the display's frame budget (`env.refresh_hz`, 120 Hz
fallback), with a red cap on frames whose work exceeds it. The runner feeds
`core.stats` and `core.env` automatically; all examples show it.

Pacing: the surface keeps two frames queued ahead of the one on screen
(`Launcher::frame_latency`, 2 by default; C47). On macOS that is triple
buffering, which gpui also uses, and every vsync gets a frame even when
little is drawn. With one queued frame, a frame whose thread woke a
little late at light load found no free drawable and missed its vsync:
112.5–118.6 fps at 100 and 2,500 boxes on an M3 Pro, against 119.7–120.0
with two. Frames that run back to back (an animation, a drag, a scroll)
start at the display's vsync, from a `CADisplayLink` on the window's view
(macOS 14+), so the extra drawable is slack and not a queue. Measured from
the moment a frame sampled its state to the moment it was on screen, it
takes 17.5–19.2 ms paced, 19.2–19.5 with one queued frame, and 27.5–27.9
with two queued unpaced. A frame drawn from idle, such as a keystroke into
a still editor, is drawn at once. A Node window turns its loop from a
timer and is not paced, so it pays the queued frame: `frameLatency: 1`
trades back. `KUI_FRAME_LATENCY` and `KUI_FRAME_PACING=0` compare without a
rebuild. On Windows one frame is queued (RG46): D3D12 waits on its
frame-latency object, and one delivered every vsync of 240 Hz at 100 to
10,000 boxes, so a second would be a vsync of latency for nothing.

Editing latency (`cargo bench -p kui-core --bench editing`, same machine and
day — one keystroke: applying the edit, then the full frame it causes, warm
caches). Each cell is the median of four runs, because a single run of the
100k row moves by ~10%:

| document | apply | frame | quads |
|---|---|---|---|
| 50 lines | 0.087 ms | 0.050 ms | 1860 |
| 500 lines | 0.090 ms | 0.052 ms | 1860 |
| 2k lines | 0.096 ms | 0.067 ms | 1860 |
| 10k lines | 0.130 ms | 0.140 ms | 1860 |
| 100k lines | 1.79 ms | 2.08 ms | 1860 |

So a keystroke costs well under a frame's worth up to 10k lines, and ~3.9 ms
at 100k. The quad count is flat because glyph emission is viewport-culled (a
huge document emits only the visible screenful), and single-line reshapes go
through cosmic-text's shape-run cache. This bench never regressed with the
frame benches above — at `dabe671` it measured 0.084/0.045 ms at 50 lines and
0.134/0.116 ms at 10k — because its frame is 1860 quads, not 10k nodes, which
was itself a clue that the cost was per node.

Layout solver, atlas packer, key scheme, event dispatch, editing,
measurement, layout events, diagnostics and the Lua binding are covered by
tests (`cargo test --workspace`; `nu scripts/test.nu` runs the same binaries
side by side, `--node` adds Node's); `npm test` in `packages/kui` covers the
Node encoder and the corpus scenes. The smoke round
(`cargo run -p kui-devtools --bin smoke`) opens eight windows at a time. [CHANGELOG.md](../CHANGELOG.md)
lists per release what was added and, separately, what an app can delete.
