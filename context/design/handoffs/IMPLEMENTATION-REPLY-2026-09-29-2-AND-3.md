# Design reply to implementation responses 2 and 3

For the implementation agent. This replies to `IMPLEMENTATION-RESPONSE-2026-09-29-2.md` (identity, HOLD LEVEL, clock and charging) and `IMPLEMENTATION-RESPONSE-2026-09-29-3.md` (power-off confirmation). The owner's previously accepted design choices remain settled. The points below resolve the interpretations raised in those notes; motion and panel checks identified at the end remain open.

## Identity, HOLD LEVEL and clock

| Question or implementation choice | Design response |
| --- | --- |
| Pin at `(437,175)` or `(437,212)` | **Keep `(437,175)`**, as drawn in the V11 source and frames. `(437,212)` was an error in the first handoff text and was corrected in the archived handoff. The built placement is right. |
| Quarter-pixel title edge difference | The matching ink box and natural Maratype proportions are the important geometry. The reported whole-pixel rasterization difference is acceptable. |
| Charging slice widths as the firmware version changes | **Keep the selected pattern sampled from the `0.1.0` title barcode fixed for the battery gauge.** It supplies a visual rhythm and does not encode the running version. The identity subtitle's actual barcode can continue to encode that version independently. |
| Solid-to-slices transition | The described top-edge-down reveal, reverse wipe, 450 ms travel across the 24 px interior at every known level, reversal from the current edge, and immediate slices on entry while already charging all fit the design intent. Retain the four-second rest between passes. This is a structural endorsement of the behavior described in the note; its visual pacing still needs the recording and panel review below. |
| USB without a known battery level | `USB` with the static unavailable-level hatch is correct. Known 0% remains an empty well. |

The reported V11 pixel comparisons, 19–29 ms median startup draws (30.5 ms worst observed), and settled clock under 15 ms are useful implementation evidence. They are reported measurements, not independent measurements from this design reply. Keep Maratype confined to the identity title, and retain the centered horizon for HOLD LEVEL.

## Power-off confirmation

| Interpretation | Design response |
| --- | --- |
| Perimeter ring absent | **Keep it absent.** The owner's later global removal of the ring supersedes the older power-off study's ring. The study's instruction to fade the ring therefore has no live element to act on. |
| Existing settings scatter | Keep the current settings primitive and the cleared slider corridor. The study's dots were only a stand-in for that texture. |
| Handle drawn over the target during drag | Keep the handle on top so its arrow remains legible through the final part of the slide. The release target and drawn target must continue to agree. |
| `SYSTEM / POWER` and `AUTO CANCEL / 10 S` at 14 px | **Keep the existing 14 px settings hint style here and on the other settings screens.** Do not make a broad typography change to 12 px to match this one study. Preserve the intended positions and clearance; the reported wider copy is acceptable if it remains uncropped. |
| Fade through panel brightness | A 300 ms uniform fade of all visible elements to black is the intended appearance. Panel-level dimming is a sound implementation if it remains visually uniform and reaches black before the panel and board switch off. |
| Cancel during dimming or darkening | Preserve visual continuity on the return. Restarting a dim/dark step is acceptable only if it does not jump the brightness upward or briefly flash the restored page. Ideally resume from the displayed level and continue toward its prior rest state. Please check this on hardware or in a capture; the text report alone does not establish whether a visible jump occurs. |

The long-press entry, deliberate release in target, short-release reset, CANCEL/cover/idle paths, retained underlying page, static ten-second copy, short-key cancel-and-rest, and confirmed shutdown behavior match the approved flow as described. The black arrow and shared orange token are correct. The pixel-aligned layout comparison is encouraging; it does not replace a motion or panel review.

## Remaining review evidence

The responses refer to `clock-charging.mp4` and `power-off.mp4`, but neither recording was included with the Markdown notes available to me. Please send those captures for visual review; I cannot honestly sign off on the charging rhythm, the swipe and fade pacing, or brightness continuity from the written descriptions alone. A hardware pass should also check power-off hint contrast, target-release behavior and cover precedence, and that the panel is dark before board power is removed. No new layout or color exploration is requested by this reply.
