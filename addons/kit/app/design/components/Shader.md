# Shader

A box a fragment shader paints: one of Kit's presets, WGSL handed over as `source`, or a fragment the host registered (`id`).


## When to use

Use a shader box for a surface that is generated rather than drawn: a live texture, a preview of a material, a decorative ground under a header. Kit rounds it by the skin's radius and keeps a shader that reads `time` still under reduced motion; with no shader, or one that does not compile, it is the skin's well. The `noise` preset drifts gradient noise between sunken and accent (scale 56px, drift 0.12); `shimmer` is a skeleton's sweep.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `preset` | `preset` | ShaderPreset? (shimmer · noise) | One of Kit's shaders, its colours the skin's: `shimmer` (a skeleton's sweep) or `noise` (gradient noise drifting between the well and the accent). Animated unless `animate = false`. |
| `source` | `source` | string? | A WGSL `fragment` function (ADR 0015), which Kit registers: the same source, the same fragment. |
| `id` | `id` | integer? | A fragment the host registered (`Core::add_fragment`, `ctx.addFragment`, `kui_fragment_add`, `env.add_fragment`). |
| `params` | `params` | {number}? | Up to sixteen numbers the shader reads as four `vec4<f32>`; a preset's come from the skin when left out. |
| `image` | `image` | integer? | A registered image's handle, which the shader samples with `kui_sample`. |
| `animate` | `animate` | boolean? | A frame every frame, for a shader that reads `time`; off under reduced motion. |
| `width` | `width` | number? | In px; it grows when left out. |
| `height` | `height` | number? | In px; 80 when left out. |
| `grow` | `grow` | boolean? | Takes the height its parent leaves. |
| `label` | `label` | string? | What it shows, as it is read; decoration when left out. |

Every component also takes `key` and `fresh`.

## Do

- Use a preset when the skin's colours should drive it; its params come from the skin.
- Hand your own WGSL as `source` with up to sixteen `params`; set `animate` when it reads `time`.
- Give `label` when the box shows something; leave it out for decoration.

## Don't

- Put text over a shader box without checking its contrast at every frame.
- Animate a shader that does not read `time`: it costs a frame every frame for nothing.
