# Palette reference

Colour values taken from the Marathon reference board, a Milanote canvas exported as
`canvas_Octowhere-260920_2101.pdf`. The export lives outside the repository.

The board prints a generated name under each swatch. Those names are not reproduced here: they come
from a colour-naming library, they carry no meaning about what the colour is for, and two different
swatches on the board share one of them. Refer to a colour by its hex value, or by the grid position
recorded below.

## Board grid

The board groups fifteen swatches under one heading, in three rows. Positions are the board's own
layout, left to right.

| Position | Hex | RGB | Appearance |
| --- | --- | --- | --- |
| R1C1 | `#FFFAC3` | 255, 250, 195 | pale yellow |
| R1C2 | `#F1710D` | 241, 113, 13 | orange |
| R1C3 | `#F24723` | 242, 71, 35 | orange-red |
| R1C4 | `#E8337C` | 232, 51, 124 | deep pink |
| R1C5 | `#B32BE5` | 179, 43, 229 | violet |
| R2C1 | `#ECDB0B` | 236, 219, 11 | yellow |
| R2C2 | `#C0FE04` | 192, 254, 4 | yellow-green |
| R2C3 | `#01E67C` | 1, 230, 124 | green |
| R2C4 | `#81EBB1` | 129, 235, 177 | pale green |
| R2C5 | `#409DE4` | 64, 157, 228 | mid blue |
| R2C6 | `#5500E4` | 85, 0, 228 | blue-violet |
| R3C1 | `#D2D3D6` | 210, 211, 214 | light neutral |
| R3C2 | `#888E98` | 136, 142, 152 | mid neutral |
| R3C3 | `#31333B` | 49, 51, 59 | dark neutral |
| R3C4 | `#1E1F24` | 30, 31, 36 | near-black neutral |

Row 3 is a neutral ramp from light to dark. Rows 1 and 2 are saturated hues.

Two more swatches sit elsewhere on the canvas, away from that group and next to unrelated material.
The board does not say what they are for.

| Hex | RGB | Appearance |
| --- | --- | --- |
| `#47771E` | 71, 119, 30 | olive green |
| `#714CC8` | 113, 76, 200 | muted violet |

## What the code uses

Every constant in [`crates/octowhere-ui/src/chrome.rs`](../crates/octowhere-ui/src/chrome.rs) is a
board value except `BLACK`.

| Token | Hex | Board position | Role |
| --- | --- | --- | --- |
| `LIME` | `#C0FE04` | R2C2 | accent, ok and live states |
| `RED` | `#F24723` | R1C3 | faults and unavailable data |
| `ORANGE` | `#F1710D` | R1C2 | second data series, second map node, idle prompt |
| `PURPLE` | `#5500E4` | R2C6 | header slab |
| `BLUE` | `#409DE4` | R2C5 | compass status icon while a heading shows |
| `GRAY` | `#888E98` | R3C2 | frames and secondary text |
| `WHITE` | `#D2D3D6` | R3C1 | primary text |
| `BLACK` | `#000000` | none | background field, knockout text on saturated fills |

`BLACK` stays pure instead of taking the board's `#1E1F24`. An unlit pixel on this AMOLED is off
rather than dim, so pure black is a contrast step no near-black reaches. The doctrine's preference
for a dark neutral field does not survive that, and changing it back would cost contrast for
nothing.

Red is spent only on faults. The second data series, the second map node and the footer's
un-selected prompt are all `ORANGE`, because none of them reports a failure.
