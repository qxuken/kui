# kui-fuzz

Fuzz targets over the kui crates, run with
[cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) (libFuzzer under
AddressSanitizer), and the stable replay of everything they found. Not
published.

| Target    | What it feeds                                                                                                    | What it checks besides "no panic"                                                                   |
| --------- | ---------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `parsers` | the string parsers every binding hands a prop to: size expressions, accelerators, the name lists, font features   | an accepted size resolves and respells; an accelerator's spelling and display read back to it       |
| `values`  | the parsers that read a prop as data (`Value`): gradients, keyframes, entrances, windows, menus, file dialogs, sizes | what reads as a size reduces to a sizing                                                            |
| `path`    | SVG `d` and the wire's floats, flattened, rasterized, and drawn as a node                                          | the wire form of parsed ops reads back to them                                                       |
| `dtcg`    | DTCG token documents (ADR 0062): JSON, then a token file or an inline-only resolver, the table added to a core and read under both appearances | no panic; a `$ref` with no folder is refused, never read |
| `scenes`  | a corpus scene, or an editor — its text now and then repeated past `LONG_LINE_BYTES`, shaped in chunks — driven by every input a driver sends: pointer, wheel pixels and notches, keys, text, IME, files, access requests, resizes, the clock — a faded subtree drawn as one group or faded per quad | every quad is finite while every input was; the editor keeps its text                               |
| `decode`  | bytes to `kui_native::decode_image` and `decode_animation`                                                        | every frame is `width * height * 4` bytes; `Animation::at` names a frame                            |
| `encode`  | pixels and frame times to `kui_native::encode_image` in four formats and `AnimationEncoder` as APNG and GIF, read back by the decoder | a PNG and a WebP read back exactly, a GIF exactly at 256 colours or fewer and opaque or clear as written; an APNG's frames are the frames added, in order, the last shown, the time kept to the millisecond a frame |
| `events`  | a view whose handler rows declare the fuzzer's messages — maps whose keys may be the event's own field, strings, numbers, `Null` — and an editor whose `value` the app declares back through a filter, driven by pointer, keys, text, IME, menus and assistive requests (ADR 0047) | every event reads back whole; a node's payload keeps its message's fields, a non-map one under `tag`; a `change` carries the editor's text; the editor holds the declared value, its caret on a character boundary |

The Node addon's binary decoder is fuzzed in its own crate's tests
(`crates/kui-node/src/fuzz_binary.rs`): the addon is a cdylib, which a
fuzz target cannot link.

## Running

Nightly Rust and cargo-fuzz, on Linux (WSL on Windows) or macOS:

```sh
rustup toolchain install nightly --profile minimal
cargo install cargo-fuzz --locked
nu scripts/fuzz.nu run scenes --time 600   # one target, ten minutes
nu scripts/fuzz.nu all --time 300          # every target, five minutes each
```

The `fuzz` workflow (`.forgejo/workflows/fuzz.yml`) runs the same script
on the runner, by hand only, and carries each target's corpus from run
to run.

Several targets at once is several `run`s, each its own process. Stop
one with Ctrl-C: killing the `nu` process alone leaves cargo-fuzz's
libFuzzer process fuzzing on under it (`pkill -f fuzz/target/` stops
those).

A crash lands in `fuzz/artifacts/<target>/`. Minimize it into a
regression named after the backlog item that fixes it:

```sh
nu scripts/fuzz.nu keep scenes fuzz/artifacts/scenes/crash-<hash> fz7
```

## What is kept

- `seeds/<target>/`: the inputs a run starts from.
- `regressions/<target>/`: every input a fuzzer crashed on, minimized,
  named after its backlog item (`fz2-*` is FZ2).

Both are replayed by `tests/regressions.rs` through the function the
fuzz target calls, under `cargo test --workspace` on stable, so a fixed
crash stays fixed without a fuzzer. `corpus/` and `artifacts/` are a
run's working state and are not checked in.

`events` and `scenes` build each input's `Core` in a session kept from
the inputs before it, because a new session's font caches were nine
tenths of an input's time (`src/reuse.rs` says what a kept session
holds and when it is dropped). `tests/reuse.rs` checks that an input runs
the same on a kept session as on a new one; `KUI_FUZZ_CORPUS=1 cargo
test --release -p kui-fuzz --test reuse` checks a run's whole corpus
too, and is worth running after every long run: a kept session that
runs an input differently is a finding no target crashes on. The
alpha.66 hour's corpus showed one that way — 516 of 33 860 runs — and
it was a cache in cosmic-text that every window of a session shares
(backlog FZ14). To find which earlier input changed a run, give each
sequence a thread of its own: `reuse::set_fresh` does not empty the
kept slot.

The target binaries need libFuzzer, so they sit behind the `libfuzzer`
feature, which only `scripts/fuzz.nu` turns on: a workspace build never
compiles them.
