//! The binary frame decoder (`binary::lower_binary`) against streams no
//! encoder wrote: a stale one, a hand-rolled one, a buffer cut short or
//! overwritten. Whatever the stream, a frame is lowered or refused with an
//! error — the addon runs inside Node, where a panic is the whole process.
//!
//! The decoder lives in this cdylib, which a cargo-fuzz target cannot link
//! (fuzz/Cargo.toml), so this is the fuzz loop in miniature, under
//! `cargo test` on stable: a seeded generator writes streams that open as
//! an encoder's do — the version, the root op, a prop list — and then
//! draws every slot from a pool of the values that matter (op codes, prop
//! ids with and without the token bit, string refs into the table, counts,
//! NaN, the infinities, -1, 2^53); streams that lowered are kept and
//! mutated, slot by slot, so later cases reach past the first refusal.
//!
//! `KUI_NODE_FUZZ_CASES` raises the case count (default 1500) and
//! `KUI_NODE_FUZZ_SEED` picks another seed; a failure prints both, and the
//! stream and string table it failed on. `KUI_NODE_FUZZ_LOG` names a file
//! each case is written to before it runs, for the one failure that
//! prints nothing: an allocation refused, which aborts the process.

use std::panic::{AssertUnwindSafe, catch_unwind};

use kui_core::{Core, Size};

use crate::binary::{self, OP_CLOSE, OP_END, OP_ROOT, TOKEN_TAG, VERSION};
use crate::schema;

/// xorshift64*: enough to spread a seed, and the same streams every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn chance(&mut self, one_in: usize) -> bool {
        self.below(one_in) == 0
    }

    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len())]
    }
}

/// What the string table holds: payloads and options as JSON, spellings
/// of the props that take strings, a lone surrogate, and bytes that are
/// not UTF-8.
const STRINGS: &[&[u8]] = &[
    b"hi",
    b"\"save\"",
    b"{\"kind\":\"x\",\"n\":1}",
    b"[1,2,3]",
    b"\"ab\\ud83d\"",
    b"[{\"label\":\"File\",\"items\":[{\"label\":\"Open\",\"accel\":\"mod+o\"},{\"role\":\"separator\"}]}]",
    b"[{\"label\":\"One\"},{\"label\":\"Two\"}]",
    b"[{\"name\":\"main\"},{\"name\":\"pop\",\"kind\":\"popup\",\"w\":100,\"h\":80}]",
    b"[{\"t\":0,\"opacity\":0},{\"t\":1,\"opacity\":1}]",
    b"clamp(10px, 50%, 1000px)",
    b"M10 10 L50 10 L30 40 Z",
    b"Inter",
    b"mono",
    b"liga=0 tnum",
    b"secondary middle",
    b"\xff\xfe\x80",
    b"",
    b"\xe2\x82",
];

/// The values a slot is drawn from.
fn slot(rng: &mut Rng, strings_len: usize, ids: &[u32]) -> f64 {
    match rng.below(16) {
        0 => f64::NAN,
        1 => rng.pick(&[
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            -0.5,
            1e300,
            9_007_199_254_740_993.0,
        ]),
        2 => rng.pick(&[
            u32::MAX as f64,
            (u32::MAX as f64) + 2.0,
            0x8000 as f64,
            65_536.0,
        ]),
        3 | 4 => rng.pick(ids) as f64,
        5 => (rng.pick(ids) | TOKEN_TAG) as f64,
        6 | 7 => rng.below(strings_len + 2) as f64,
        8 => rng.below(28) as f64,
        9 => rng.below(1 << 24) as f64,
        10 => rng.next() as f64 / 1e9,
        _ => rng.below(8) as f64,
    }
}

/// A stream an encoder could have started: the version, the root op, a
/// prop list, then ops until the end op — every slot after the root's
/// count drawn from the pool.
fn generate(rng: &mut Rng, strings_len: usize, ids: &[u32]) -> Vec<f64> {
    let mut s = vec![VERSION as f64, OP_ROOT as f64];
    let props = rng.below(4);
    s.push(props as f64);
    let len = 1 + rng.below(80);
    for _ in 0..len {
        s.push(slot(rng, strings_len, ids));
    }
    if rng.chance(2) {
        s.push(OP_CLOSE as f64);
    }
    s.push(OP_END as f64);
    s
}

/// One stream changed a little: a slot replaced, inserted, removed or
/// duplicated, or the tail cut.
fn mutate(rng: &mut Rng, from: &[f64], strings_len: usize, ids: &[u32]) -> Vec<f64> {
    let mut s = from.to_vec();
    for _ in 0..1 + rng.below(4) {
        let at = rng.below(s.len());
        match rng.below(6) {
            0 | 1 => s[at] = slot(rng, strings_len, ids),
            2 => s.insert(at, slot(rng, strings_len, ids)),
            3 if s.len() > 3 => {
                s.remove(at);
            }
            4 => {
                let end = (at + 1 + rng.below(6)).min(s.len());
                let run: Vec<f64> = s[at..end].to_vec();
                s.splice(at..at, run);
            }
            _ => s.truncate(at.max(2)),
        }
    }
    s
}

/// The table: some of [`STRINGS`], back to back.
fn table(rng: &mut Rng) -> Vec<u8> {
    let mut t = Vec::new();
    for _ in 0..rng.below(6) {
        t.extend_from_slice(rng.pick(STRINGS));
    }
    t
}

/// Lowers one stream into a fresh frame the way `Surface::frame_binary`
/// does, and finishes it whatever the decoder said.
fn lower(core: &mut Core, stream: &[f64], strings: &[u8]) -> bool {
    let mut ui = core.frame(Size::new(320.0, 240.0), 1.0);
    let ok = binary::lower_binary(&mut ui, stream, strings).is_ok();
    ui.finish();
    let _ = core.take_pending_events();
    ok
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[test]
fn no_stream_panics_the_decoder() {
    let cases = env_u64("KUI_NODE_FUZZ_CASES", 1500);
    let seed = env_u64("KUI_NODE_FUZZ_SEED", 0x6b75_6921);
    let mut rng = Rng(seed | 1);
    let mut ids: Vec<u32> = kui_core::schema::PROPS.iter().map(|p| p.id).collect();
    ids.extend([
        schema::P_DIR,
        schema::P_SIZE,
        schema::P_KEY,
        schema::P_TITLE,
        schema::P_PAD,
        schema::P_BORDER,
        schema::P_OVERFLOW,
        schema::P_FLOAT,
        schema::P_KEY_FOCUS,
        schema::P_TOOLTIP,
        schema::P_WINDOWS,
        schema::P_INDEX,
        schema::P_ROW_COUNT,
    ]);
    // Every op code too: they are slots like any other.
    ids.extend(0..=binary::OP_PATH);
    // Streams that lowered, to build on; the smoke test's frame to start.
    let mut kept: Vec<(Vec<f64>, Vec<u8>)> = vec![(
        vec![
            VERSION as f64,
            OP_ROOT as f64,
            1.0,
            schema::P_DIR as f64,
            0.0,
            binary::OP_TEXT as f64,
            0.0,
            2.0,
            0.0,
            binary::OP_OPEN as f64,
            0.0,
            OP_CLOSE as f64,
            OP_END as f64,
        ],
        b"hi".to_vec(),
    )];
    let mut core = Core::new();
    let mut failed = Vec::new();
    for case in 0..cases {
        // A fresh core now and then: one long-lived core is what a Node
        // app has, but a failure should not need a thousand frames to show.
        if case % 64 == 0 {
            core = Core::new();
        }
        let (stream, strings) = if kept.len() > 1 && rng.chance(2) || rng.chance(3) {
            let (s, t) = &kept[rng.below(kept.len())];
            let t = if rng.chance(4) {
                table(&mut rng)
            } else {
                t.clone()
            };
            (mutate(&mut rng, s, t.len(), &ids), t)
        } else {
            let t = table(&mut rng);
            (generate(&mut rng, t.len(), &ids), t)
        };
        // An allocation that fails aborts rather than unwinds, and takes
        // the report with it: with a path here, each case is written there
        // first, so the last one written is the one that aborted.
        if let Ok(path) = std::env::var("KUI_NODE_FUZZ_LOG") {
            let _ = std::fs::write(
                path,
                format!("case {case} (seed {seed:#x})\nstream {stream:?}\nstrings {strings:?}\n"),
            );
        }
        match catch_unwind(AssertUnwindSafe(|| lower(&mut core, &stream, &strings))) {
            Ok(true) if kept.len() < 256 => kept.push((stream, strings)),
            Ok(_) => {}
            Err(_) => {
                failed.push(format!(
                    "case {case} (seed {seed:#x}): stream {stream:?}, strings {:?}",
                    String::from_utf8_lossy(&strings)
                ));
                core = Core::new();
                if failed.len() >= 5 {
                    break;
                }
            }
        }
    }
    assert!(
        failed.is_empty(),
        "the decoder panicked on {} stream(s):\n{}",
        failed.len(),
        failed.join("\n")
    );
}
