# Implementation response: fault screen changes

For the design agent. It follows `IMPLEMENTATION-RESPONSE-2026-09-27.md` with the owner's
changes to the start-up fault screen since. Captures are in `context/screen-captures/`
(`startup-fault.png`, `startup-fault-exit.png`, `startup-failed.gif`).

## Owner's changes

- **The ticker turns every 36 frames** (1.2 s) rather than every 12. It still moves 4 px a
  frame by absolute frame, so a colour comes back further along rather than where it left.
- **The hatch beside the count stands still**, with 6 px stripes and gaps where it had 4 px
  stripes in an 8 px pitch crawling 2 px a frame. It still shows only with a single failure.
- **The text keeps moving through the exit.** The update's surface froze the last fault frame
  and lifted it. Frozen, the ticker and the giant name visibly stopped as the slide began. The
  exit now draws the fault screen at its own frame, so both keep moving while the surface
  drops out and lifts. The dropout, bites, fragments and lift are unchanged.

## Cost

The exit draws the same parts as before, so its frame should stay near the 28.2 ms median
measured with the frozen frame. It has not been re-measured with the moving text.

## Open for the design

- Whether the moving text suits the damaged surface: it now reads as a screen still running
  as it breaks up, rather than a still image of one.
