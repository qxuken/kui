# Settings

Kit's look for this window: the skin, the density, the hard shadows and any role the brand overrides.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/kit.tl`. In an app, place `kit.settings` and kui paints it.

## When to use

Place `kit.settings` once a frame, before the components; it draws nothing. It picks the skin (Ledger by default, Neutral, Phosphor), holds the appearance light or dark or follows the OS, sets the density (rows 22 and controls 26, or 20 and 22 compact), turns the shadows off, and takes a brand's radius, hairline and rule widths, faces and colour roles. The preview shows the same controls at both densities in the theme you are viewing.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `skin` | `skin` | Skin? (ledger · phosphor · neutral) | The skin; Ledger when left out. |
| `density` | `density` | Density? (comfortable · compact) | Comfortable when left out. |
| `shadows` | `shadows` | boolean? | The skin's hard shadows; on when left out. |
| `radius` | `radius` | number? | Corner radius in px, for a brand with its own; the skin's when left out. |
| `roles` | `roles` | any? | Roles the brand overrides, by name: `{ accent = 0x2f6fedff, panel = "$brandSurface" }` — a colour, `{ light, dark }` or a token each. An accent given as a colour derives the palette around it: what the skin drew in its accent takes it (the selection, a view switch's chosen segment, the first chart series and waterfall group), what it drew in the accent's hue takes the new hue, the ground and surfaces lean a few percent toward it, and every text role is moved in lightness until it reads at 4.5:1 on what it sits on (WCAG AA), what reads on the accent worked out. Roles the brand names are kept as named. |
| `appearance` | `appearance` | Appearance? (system · light · dark) | Kit's colours held light or dark whatever the OS says; the OS's when left out (`system`). |
| `hair` | `hair` | number? | The hairline's width in px (rows, a field's edge); the skin's when left out. |
| `rule` | `rule` | number? | The region rule's width in px (a panel's frame, a control's border); the skin's when left out. |
| `font_ui` | `fontUi` | {string}? | The UI face, as a family list (`{ "Inter", "sans" }`); the skin's when left out. |
| `font_data` | `fontData` | {string}? | The data face (ids, numbers, code), as a family list; the skin's when left out. |

Every component also takes `key` and `fresh`.

## Do

- Pick the skin per window and let the user switch it: every sample offers Ledger, Neutral and Phosphor.
- Give a brand colour as `roles = { accent = 0x2f6fedff }`: Kit derives the selection, the chosen segment, the first series and the surfaces' tint from it and moves every text role until it reads at 4.5:1.
- Name a token instead (`panel = "$brandSurface"`) when the host has its own light and dark for a role.
- Paint your own nodes with Kit's exported tokens (`$kitPanel`, `$kitFg`, `$kitRadius`) so they follow the skin.

## Don't

- Place it more than once a frame, or after the components it styles.
- Hard-code a skin's colours in your own nodes: they would not follow a skin or appearance change.
- Override `fg`, `muted` or `faint` with a colour that has not been checked on panel, bg, side, sunken and raised: Kit keeps a role you name as named.
