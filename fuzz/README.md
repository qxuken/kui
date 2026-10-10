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
| `scenes`  | a corpus scene, or an editor, driven by every input a driver sends: pointer, wheel, keys, text, IME, files, access requests, resizes, the clock | every quad is finite while every input was; the editor keeps its text                               |
| `decode`  | bytes to `kui_native::decode_image` and `decode_animation`                                                        | every frame is `width * height * 4` bytes; `Animation::at` names a frame                            |

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

The target binaries need libFuzzer, so they sit behind the `libfuzzer`
feature, which only `scripts/fuzz.nu` turns on: a workspace build never
compiles them.
