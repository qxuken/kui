# Skeleton

A placeholder for what is still loading: lines a text high, a block or a circle, in the skin's well colour and rounded as its controls are, with a shimmer crossing them in step.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/data.tl`. In an app, place `kit.skeleton` and kui paints it.

## When to use

Use skeletons in the shape of content that is on its way: an avatar, a name, a bio, a pair of counts. Lines are 10px high and 6 apart, the last of several at 60%; blocks default to 80px; circles to 32. The shimmer is Kit's own fragment shader sweeping shimmer across sunken, one period (1.5 s) for every bar, held still under reduced motion. The preview freezes the band mid-sweep.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `shape` | `shape` | SkeletonShape? (text · block · circle) | Lines of text (the default), a block or a circle. |
| `lines` | `lines` | integer? | Text: how many lines, the last of several shorter; 1 when left out. |
| `width` | `width` | number? | In px, or a circle's diameter (32 when left out); a text or a block grows when left out. |
| `height` | `height` | number? | A block's height in px; 80 when left out. |
| `label` | `label` | string? | What is loading, as it is read; `Loading` when left out. |

Every component also takes `key` and `fresh`.

## Do

- Shape the skeleton like what will replace it, at its size.
- Give `label` for what is loading (Loading the bio); it is read as one image.
- Swap it for the content in one frame when it arrives.

## Don't

- Show skeletons for content that is already there.
- Use a skeleton for work with no shape (a save): use progress.
