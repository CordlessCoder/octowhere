# OCTOWHERE color roles — 26 September 2026

This table formalizes the owner's accepted color direction after reviewing the V3 screen family. These roles describe meaning, not a requirement to fill every screen with every color. The concrete screen states and geometry are in `handoffs/IMPLEMENTATION-HANDOFF-CURRENT.md` and the relevant specs. Test contrast and brightness on the 466 × 466 AMOLED.

| Token | Value | Meaning | Current use and boundary |
| --- | --- | --- | --- |
| `BLACK` | `#000000` | Unlit field and knockout ink | Pure black is the primary AMOLED ground. Black text on bright slabs. |
| `WHITE` | `#D2D3D6` | Neutral primary information and major neutral forms | Time when no local zone is chosen, plain labels, neutral bands and compass heading slab. It asserts legibility, not health. |
| `GRAY` | `#888E98` | Secondary, inactive or unconfirmed information | Captions, rules, neighboring picker values, unknown/unconfirmed icons, mode tags. Keep unavailable versus unconfirmed explicit in text. |
| `LIME` | `#C0FE04` | OCTOWHERE identity and normal clock operation | Startup title/card and the local clock band. It is no longer the default color for an ordinary settings edit. It is not a universal “success” badge. |
| `VIOLET` | `#B32BE5` | The active editable choice within settings | D3 offset, zone, brightness, timeout, replay GOOD selection and their outlined icons; black knockout value. This is the previously unused reference swatch, now a settings role. |
| `PURPLE` | `#5500E4` | Brand texture and registration detail, with no status claim | Low-density scatter and small marks in clock/settings. The matched identity uses a deliberately dimmer purple field for its plume. Never make a critical status depend on this texture. |
| `BLUE` | `#409DE4` | A live, valid reading or linked live subsystem | Valid compass heading, GNSS fix/detail icon. Sparse use is deliberate. Do not apply it to ordinary buttons or claim a GNSS fix merely because the receiver answered its boot test. |
| `ORANGE` | `#F1710D` | Attention, degraded operation or an action requiring deliberate care | Calibration, interference, stopped clock, battery at 15% or below, CLEAR SETTINGS and the marked simulated-failure choice. A destructive action additionally requires its two-stage drag and explicit text. |
| `RED` | `#F24723` | A genuine fault | Failed self-test, actual startup fault, clock/compass NO DATA fault. A simulated failure must be labeled as a demonstration and must not masquerade as a fresh hardware test. |

## Precedence and use

1. **State overrides screen identity.** A setting normally selects in violet, but its clear action remains orange and a real fault remains red. A clock normally uses lime, but STOPPED is orange and NO DATA is red.
2. **The screen still works without color.** Use words, glyphs, number changes, fill/outline and the two-stage confirm to carry meaning. Preserve the difference between unknown time, known UTC with no zone, no GNSS fix and a hardware fault.
3. **One dominant saturated field is usually enough.** Supporting scatter stays dark and stable. AOD uses restrained lit area at its actual panel level; its battery label remains legible even when clock time is invalid.
4. **Do not invent status from appearance.** `OK` in self-test means a peripheral answered by its deadline; GNSS `OK` does not prove a position fix. Demo replay has its own label. Text and live data take precedence over the brand palette.
5. **Keep approved meanings across surfaces.** `BLUE` belongs to actual valid readings, `ORANGE` to attention, `RED` to fault; `VIOLET` belongs to editing rather than an outcome. Within a component, use black on a bright fill and white/gray on black. Validate on hardware, including dim and low-brightness states.

## Reserved reference swatches

These are in the source reference board but have **no adopted OCTOWHERE semantic role**. Do not add them to firmware just to diversify a screen.

| Swatch | Possible future role to discuss, not approved |
| --- | --- |
| Deep pink `#E8337C` | Explicit simulation/demo mode, if it becomes a repeated product-wide distinction from real red faults. A `DEMO` label would still be required. |
| Yellow `#ECDB0B` and pale yellow `#FFFAC3` | An intermediate caution tier, only if a concrete state exists between neutral and orange. |
| Green `#01E67C` and pale green `#81EBB1` | A distinct completion or power state, only if lime no longer expresses it clearly. |
| Dark neutrals `#31333B` and `#1E1F24` | Future grouping surfaces if density requires them; broad fills illuminate pixels that pure black leaves off. |

The earlier Round 4 brief calls `LIME` and `PURPLE` unused and the older settings spec describes ten discrete brightness steps. Later accepted clock work and the built whole-percent brightness editor supersede those particular statements. This document does not change any screen layout or animation timing.
