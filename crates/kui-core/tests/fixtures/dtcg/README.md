# DTCG fixture

One brand in two appearances, as a Resolver Module document (`brand.resolver.json`)
composing a base set and a `mode` modifier with `light` and `dark` contexts. Every
binding's DTCG test loads it with `prefix: "brand"` and expects the same table
(ADR 0062):

| name | kind | light | dark |
| --- | --- | --- | --- |
| `brandSpaceGap` | length | 8 | 8 |
| `brandSpaceGutter` | length | 16 | 16 |
| `brandFontCode` | family | IBM Plex Mono, mono | |
| `brandColorSurface` | colour | `#ffffff` | `#1a1a18` |
| `brandColorInk` | colour | `#141413` | `#f1eee6` |
| `brandColorAccent` | colour | `#ffd400` | `#ffd400` |
| `brandColorBrand` | colour | `#ffd400` | `#ffd400` |

and one warning, `shadow.card` (a shadow has no kui kind).
