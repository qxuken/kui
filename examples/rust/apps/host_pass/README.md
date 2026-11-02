# Host pass: Replicator Bloom

A complete game HUD over a wgpu world, using `Renderer::render_into` instead
of an offscreen Kui texture and a fullscreen composite.

Run it from the repository root:

```sh
cargo run -p kui-native --example host_pass --features host-pass
```

For a finite smoke run:

```sh
cargo run -p kui-native --example host_pass --features host-pass -- --frames 120
```

Controls: Harvest or Space gains gold, Build drone spends 10, Auto-harvest
turns drone income on, log rows select, the wheel scrolls, a world click moves
the ship, and Escape quits.

The example is gated by `host-pass`, so normal Kui builds do not compile it or
its optional host-access and bytemuck dependencies.

The frame is one command encoder and one submission:

1. Acquire the surface and record the world pass.
2. Call `render_into(list, atlas, encoder, view, LoadOp::Load)` to draw Kui
   over it.
3. Call `Renderer::submit` to finalize Kui's mapped frame slot and submit the
   world and UI together, then present.

`game.rs` owns the model, `hud.rs` the view and messages, `input.rs` the winit
and accessibility bridges, `render.rs` the GPU frame, and `main.rs` the winit
application. The log uses Kui's uniform-list helper and stable line ids.

The stock `widgets::latency_hud` floats in the lower-left corner. The host feeds
`core.stats` directly: winit event time becomes `input_ms`, the HUD call becomes
`view_ms`, `ui.finish` and event draining become `layout_ms`, the world pass,
Kui's render and submit become `render_ms`, and surface acquisition becomes `wait_ms`. No custom
latency metrics or chart implementation is needed.
