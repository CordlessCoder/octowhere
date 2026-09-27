# Validation map for the incremental implementation

Compare at **466 × 466 native pixels**, then inspect the actual round AMOLED at normal brightness. All renders are concepts, not captured firmware. Keep old-backup acceptance checks for unchanged screens.

## Charging bar

| Capture / stimulus | Reference | Pass condition |
| --- | --- | --- |
| GNSS, RTC, MANUAL, STOPPED, NO ZONE, NO DATA; 87%, not charging | `references/battery/clock-solid-all-states.png` | Same gauge height and full solid width in every state; state-specific colors and surrounding UI retained |
| The same six while charging | `clock-solid-charging-pairs.png`, `clock-all-states-charging.png` | Uneven full-width horizontal bands at same total level, no hatch or horizontal movement |
| 5, 12, 20, 35, 50, 70, 87, 100% | `clock-level-scaling.png` | Segment count scales with height; 12% has two bands; no unreadable uniform ladder |
| 12%, unknown and 0% | `clock-solid-edge-states.png`, `clock-cross-state-edges.png` | Orange known low battery, static gray dashes for unknown, empty interior at zero |
| Stable charging over ≥2.4 s | `clock-charging-across-states.gif` | Irregular y-only gather/hold/pair/spread rhythm repeats; top/bottom still encode level |
| Plug and unplug at 87%; repeat at 12% | `clock-charge-entry.gif`, `clock-charge-interruption.gif`, `clock-charge-entry-low.gif`, `clock-charge-interruption-low.gif`; `clock-charge-transition-storyboard.png` | Solid retreats downward on entry and covers upward on stop; endpoints have no flicker or level jump; status text changes with signal |
| Interrupt halfway and resume; change battery reading halfway | `IMPLEMENTATION-DELTA.md` | Wipe reverses from current edge and rebases at new fill height without snapping |

The storyboard samples five moments for GNSS 87%, GNSS 12%, STOPPED, NO ZONE and NO DATA in both directions. `clock-charge-transition-full-screens.png` shows the result in the full clock composition. AOD remains a static, independently refreshed design from the old handoff.

## Fault

| Capture / stimulus | Reference | Pass condition |
| --- | --- | --- |
| One actual MAGNET failure, 4 s hold | `references/fault/fault-alternating-single.gif`, `fault-blue-name.png`, `fault-yellow-fault.png` | Blue names and yellow FAULT switch every 400 ms; red field and truthful reason retained |
| Observe 0–47 ticker frames | `ticker-continuity-board.png` | Both lines' positions advance while hidden; no repeated 400 ms reset |
| MAGNET + GNSS failure | `fault-alternating-two-failures.gif`, `fault-two-failures.png` | First part remains giant and owns reason; fixed summary lists both; blue ticker joins both |
| Three or more failures, long names and DEMO replay | `IMPLEMENTATION-DELTA.md` | Canonical order, `+N`, full stored result, visible DEMO marker and no overwritten real test |
| Natural timeout after exactly 120 fault frames | `fault-damaged-slide-exit.gif`, `exit-sequence-board.png`, `exit-frame-00/01/02/08/17.png` | Early severe red dropout, broken horizontal red edge, ease-in upward slide, no red hairline, black by final frame, clock entry after 600 ms |
| Touch during hold or exit | Old handoff plus delta | Immediate clock entry; touch consumed until lift; no forced wait for the slide |
| CLOCK peripheral failure | Old handoff | NO DATA destination, not the GNSS render fixture |

The `source/fault-cinematic-timing-comparison.md` compares the inspiration's timestamps with this accepted study; use **the study's 600 ms** as the implementation target. Original source videos are omitted; the user will supply them if necessary.

## Evidence to return to the owner

Supply native 466 px stills for all six noncharging and charging clock states, a full 2.4 s charging loop, plug/unplug videos at normal and low level, and fault recordings for one and two failures including the natural exit and a touch skip. Record frame timestamps, draw/transfer duration, RGB565/color/font differences, and any lost frames. Do not compare fixture times, battery percentages or simulated failure names as though they were device data.

## Reproducing concept renders

Extract the original design backup into a sibling directory named `octowhere-design-project`. With Pillow available, run `python3 source/render_transition.py` for battery stills/GIFs and `python3 source/fault_study.py` for the fault study. The scripts use their sibling project for fonts, the original K1 stills and the dated fault capture. Firmware should draw primitives rather than ship GIF/PNG frames.
