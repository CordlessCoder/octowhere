# Design response to implementation update 4

**Current status (owner clarification):** The owner has personally used and validated the resulting UI. All design choices described below are settled, including the scatter spread/density, 75–100% breathing, 40% compass fade, and hour-rail reveal. No further captures or tuning are needed for design approval. Technical measurements can continue independently.

This responds to `IMPLEMENTATION-RESPONSE-2026-09-27-4.md`. No conclusion is offered about update 3, which was not supplied with this response.

## Decisions

| Change | Design response |
| --- | --- |
| Settings overview scatter continues beneath transparent scrolling rows; title and hint clear | **Keep.** Removing the rectangular cut fixes the visible straight seam. The 220 px fields can thin naturally towards the grid. Marks near row text should remain sparse and dim enough that indices, values, rules and glyphs read at native size during motion. Preserve the title/hint clearances and the static full-density inner settings treatment previously agreed. |
| Wider clock scatter, upper radius 220 and lower 178 | **Accepted density.** The wider fields and the owner-validated 75–100% awake breathing cycle are the current design. No further density pass is requested. |
| Compass field dims to 40% during the first half of a departing swipe | **Accepted at 40%.** This makes the background recede before the page is gone while the reading remains clear. A reversed drag should restore brightness from the current swipe progress without a jump. The fault/no-data treatment must still take precedence over this decorative field. |
| Hour rail opens from its middle over 240–400 ms and closes on exit | **Keep.** Synchronizing it with the zone line makes the rail feel constructed rather than abruptly inserted. Reveal a known hour marker only after its cell exists; do not invent a local-hour marker in STOPPED or NO ZONE. Reverse from the current reveal fraction on an interrupted entry or exit. |

## Acceptance status

The owner validated these behaviors through use of the resulting UI. The enlarged settings and clock fields, compass fade and rail reveal are the current visual authority over the older family-pass board. Further recordings or comparisons are not a condition of design sign-off. Draw/transfer timing and awake power remain separate engineering measurements if needed; they do not reopen the approved design choices.
