# OCTOWHERE — implementation handoff
Date: 30 September 2026

## Status and scope

The owner has approved the typography selections below. Apply them to the
existing implementation, which already incorporates the previous design
handoffs.

This update covers:
- Clock typography.
- Compass typography and background texture colors.
- Settings typography.
- Settings halftone coverage and brightness.

Preserve the previously implemented layouts, state semantics, battery design,
icons, gestures, color roles, and animation timings unless explicitly changed
below. These font changes do not introduce new numeral animations.

Render at native 466 × 466 resolution. Use natural font proportions:
do not stretch or compress glyphs.

## Reference images

The user will provide the reference images with this document.

| Reference | Use |
| --- | --- |
| clock-band-selected-all-states.png | Selected clock labels and data across states |
| clock-band-selected-copy-edges.png | Clock copy and value edge cases |
| clock-band-selected-battery.png | Clock typography alongside battery states |
| compass-kh-d-selected.png | Approved combined compass typography |
| 01-overview-pages.png | Settings overview: use column B |
| 02-inner-values.png | Settings inner screens: use column D |
| 03-device-replay.png | DEVICE and replay: use column D |
| 00-title.png | SETTINGS title: retain column A, Shapiro |

The settings comparison boards are exploratory artifacts, but the selections
specified in this document are settled. Do not implement the other columns.

## 1. Clock typography

### Main time

Use KH Interference Bold for active hours, minutes, and matched seconds.

| Element | Reference geometry |
| --- | --- |
| HH/MM digits | KH Interference Bold, natural 136 px size |
| Two digit ink centers | x = 103 and 185 |
| Hours baseline | y = 184 |
| Minutes baseline | y = 305 |
| Seconds | KH Interference Bold, natural 40 px size |
| Seconds ink centers | x = 401 and 425 |
| Seconds baseline | y = 244 |

Keep digit positions stable when values change.

The unavailable-time `--` treatment retains the previous Fraktion Mono Bold
face. KH's dash was too heavy in this context. Preserve the selected
STOPPED/withheld-time behavior and existing state semantics.

### Clock band

Use the selected “KH label with Sans data” treatment.

| Element | Face and size | Position |
| --- | --- | --- |
| Status label | KH Interference Bold 18 | Visible ink starts x = 262; baseline y = 225 |
| UTC data | PP Fraktion Sans Light 16 | Visible ink starts x = 262; baseline y = 248 |
| Battery data | PP Fraktion Sans Light 16 | Visible ink starts x = 262; baseline y = 266 |

Preserve the existing band colors and actual state-dependent copy.
Validate all clock states, battery unknown/low/charging, and `100% CHG`.

AOD typography is not changed by this handoff.

## 2. Compass typography

The selected combination is:
- KH Interference Bold for the large reading.
- Small-text option D from the compass study.

Use `compass-kh-d-selected.png` as the combined reference.

| Element | Selected treatment |
| --- | --- |
| Three main reading characters | KH Interference Bold 86 |
| Inactive `---` | KH Interference Bold 86 |
| Degree/percent suffix | Existing Fraktion Mono Bold 40, raised beside the reading |
| MAGNETIC / CALIBRATION / COMPASS caption | KH Interference Bold 18 |
| HOLD LEVEL and other ordinary state instructions | Existing Fraktion Mono Regular treatment |
| INTERFERENCE | Existing Fraktion Mono Bold attention treatment |
| Pitch and roll | Existing Fraktion Mono Regular 23 |
| Dial N/E/S/W | Existing lettering |
| NO DATA slab text | Existing face |

The study uses Mono Regular 20 for ordinary state lines and Mono Bold 24
for INTERFERENCE.

Keep the existing 196 × 91 reading slab and center the visible reading group
inside it. Validate `047°`, `358°`, and `054%`.

Retain the selected centered horizon glyph for HOLD LEVEL.
Do not restore the removed perimeter outline.

## 3. Compass background texture

The owner prefers the design render's blue noise colors and wants the
implementation to reproduce them.

The source palette is defined in:
`renderer/concept/compass_noise.py`

### Palette

| Level | RGB | Hex |
| --- | --- | --- |
| 0 | 4, 11, 29 | #040B1D |
| 1 | 6, 19, 49 | #061331 |
| 2 | 9, 31, 72 | #091F48 |
| 3 | 15, 47, 103 | #0F2F67 |
| 4 | 25, 66, 140 | #19428C |
| 5 | 37, 88, 170 | #2558AA |

The main block field uses levels 1–4.
The calibration texture includes sparse brighter specks from level 5.
Black remains the underlying field.

For fine horizontal highlights, the source transforms each channel as:

    highlight = min(255, floor(channel * 1.30) + 2)

Apply the state brightness multiplier after constructing the texture.

### State brightness

| State | Texture multiplier |
| --- | --- |
| Settled heading | 0.62 |
| Calibration | 0.47 |
| Interference | 0.26 |
| NO DATA | No texture; black background |

Within the central content region:

    115 < x < 351
    80 < y < 344

apply a further texture multiplier of 0.27.

Keep the foreground dial, icon, captions, reading slab, and pitch/roll colors
independent of this texture dimming. Preserve clear space around their ink.

The original selected texture fixtures are:
- Heading: blocks, seed 4.
- Calibration: triangles, seed 8.
- Interference: blocks, seed 5.

Use the existing source when exact procedural reproduction is useful.
The texture is stable once settled; this update adds no idle refresh motion.
Retain the approved HOLD LEVEL appearance.

Check the appearance after RGB565 conversion. The darkest palette levels
can become difficult to distinguish on the physical panel.

## 4. Settings overview

The owner selected option B for both overview pages.

| Element | Selected treatment |
| --- | --- |
| SETTINGS title | Existing Shapiro, 27 px |
| Row names | KH Interference Bold, 20 px |
| Row values | Existing Fraktion Mono Bold, 20 px |
| Indices, page descriptor, footer | Existing Mono treatment |

Apply this to all eight entries:
ZONE, BRIGHTNESS, TIMEOUT, ALWAYS ON, COMPASS, GNSS, BATTERY, DEVICE.

Preserve the selected vertical index layout, icons, row geometry,
state colors, and purple outer-arc texture.

## 5. Inner settings, DEVICE, and replay

The owner selected column D in both:
- `02-inner-values.png`
- `03-device-replay.png`

Keep the SETTINGS title in Shapiro 27.

### Large selected values

| Screen | Selected value face and size |
| --- | --- |
| OFFSET time | KH Interference Bold 51 |
| ZONE name | KH Interference Bold 37 |
| BRIGHTNESS percentage | KH Interference Bold 54 |
| TIMEOUT selection | KH Interference Bold 43 |
| Successful replay GOOD | KH Interference Bold 36 |
| Selected failure, e.g. 05 MAGNET FAIL | KH Interference Bold 25 |

Keep the selected violet surfaces and the orange failure-demo surface.
Center visible glyph bounds vertically within their existing boxes.
Brightness digits must retain clear top and bottom margins.

### Exact secondary-text mapping

Column D is a specific field mapping, not a global Sans replacement.

In the study renderer:
- Large selected text originally using Mono Bold at 25 px or more becomes KH Bold.
- Neighbor text originally using Mono Regular at 17 px or more becomes
  Fraktion Sans Light, one point larger.
- Smaller Mono text and neighboring Mono Bold choices retain their existing face.

Consequently:
- OFFSET upper neighbor remains Mono Regular 16.
- OFFSET lower neighbor becomes Sans Light 18.
- ZONE's neighboring LONDON entry remains Mono Bold 24.
- TIMEOUT neighbors remain Mono Bold 23.
- Replay neighbors retain their existing Mono Bold treatment.
- UTC annotations, LIVE, zone metadata, page counters, instructions,
  CANCEL/BACK/AUTO, and DEVICE utility buttons remain Mono.
- DEVICE labels, values, attribution, and compact operational copy remain Mono.

Use the actual D reference columns when resolving any ambiguity.
Do not extend KH into all small labels or DEVICE metadata.

Test long real zone names and identifiers using the existing fitting or
truncation policy. Preserve natural font proportions.

## 6. Settings halftone: coverage and brightness

The owner specifically approved the pattern varying both:
1. The coverage/density of its marks.
2. The brightness of those marks.

The firmware should carry this visual relationship forward:
sparse areas should read as darker; denser areas should also use brighter
marks. Reproduce both dimensions of the tonal progression.

Do not use a single fixed dot color with density variation alone.

The existing purple source swatches are:

| RGB | Hex |
| --- | --- |
| 24, 10, 54 | #180A36 |
| 37, 12, 84 | #250C54 |
| 55, 19, 116 | #371374 |

The prototype varies coverage spatially, with approximately 9% occupancy
in quiet regions and 43% in the emphasized outer lobes. It also uses multiple
purple brightness levels and small black knockouts.

These are source parameters, not a claim that an exact per-cell tone-to-density
formula has already been defined. Preserve the approved appearance and make
coverage and brightness work together in the firmware rendering.

Keep the pattern in the outer arcs and clear of the central content.
It remains static at rest. Preserve the violet selection color #B32BE5.

## 7. Fonts and rasterization

Use the supplied KH Interference Bold and PP Fraktion font assets for visual
comparison. The KH asset used in the study is a trial font; use a licensed
production asset for release.

Match visible ink placement rather than assuming different faces share
identical metrics. Use native proportions and stable digit cells.

Where the embedded rasterizer differs from the Pillow references, compare
glyph size, weight, spacing, baseline, and clearance on the panel.

## 8. Validation requested

Provide native 466 × 466 captures covering:

- Clock states with active time and withheld-time dashes.
- Clock seconds and band data, including battery edge cases.
- Compass heading, calibration, interference, HOLD LEVEL, and NO DATA.
- Both settings overview pages.
- OFFSET, ZONE, BRIGHTNESS, and TIMEOUT.
- DEVICE top and end.
- Successful replay and selected peripheral-failure replay.

Also check:
- Compass `358°` and `054%`.
- Brightness `100%`.
- Long zone names and identifiers.
- Text clearance inside selected boxes.
- Compass texture and settings halftone at normal physical panel brightness.

Existing animation durations and state-transition behavior remain the
previously approved implementation. Capture any layout or rasterization
deviation explicitly rather than silently changing the selected typography.