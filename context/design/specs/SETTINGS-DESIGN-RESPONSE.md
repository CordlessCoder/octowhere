# Settings round: design response

This replies to `IMPLEMENTATION-RESPONSE.md`. It covers two changes, and the interpretations the
design accepts. `SCREEN-DESIGN-BRIEF.md` has been refreshed to describe the build as it stands.

## Changes

1. **Settle with a fixed-duration ease.** The panel's entry waits until the sheet reaches rest,
   and the exponential tail takes about 200 ms more after the sheet looks settled. The pager does
   the same. Every build and reveal is designed to start as the page lands, so that delay undoes
   them.
   - Change: after a committed release, and after one that springs back, ease from the release
     position to the target with a cubic ease-out over 160 ms. That is the curve the grid snap
     already uses. It applies to the sheet (open and close) and the pager.
   - The entry starts on the frame the page arrives. The commit rules are unchanged: past a
     quarter, or faster than 600 px/s.
   - If a fixed 160 ms feels wrong on the panel after a fast flick, fall back to keeping the
     exponential ease and counting the page as arrived within 2 px. Either way, the entry must
     start when the page looks settled.
2. **Show the result before saving.** A save holds the display for about 330 ms. When the hold
   lands before the confirming frame, it reads as the device ignoring the tap.
   - Change: for "tap to keep" a brightness, storing a zone, and completing a clear, cut back to
     the panel and show the new value first. Start the flash write only once that frame has
     reached the panel. The pause then happens on a screen that has already confirmed the
     action.
   - For a clear, the panel shows the defaults before the erase finishes. That is acceptable. If
     power is lost in that window, the old settings come back on the next boot, and nothing
     false was shown about the running state.
   - Moving the write off the display core, or deferring it until the panel is idle, removes the
     freeze entirely. It stays in the backlog. Put it ahead of other work that touches settings.

## Interpretations accepted as built

- Drag routing on a face and on the panel, including the directions that do nothing.
- Open and close use the pager's commit rules (their settle changes, above).
- Grid snap, second-level entry, picker stepping and fling, and picker order with `Etc` zones
  last.
- The clear confirm counts when the handle's middle is over the target (left edge at 308 or
  more).
- The values with no reading: `--` for an unknown battery or zone offset, `NONE` with no battery,
  `NO ZONE` with none set. The four forms of the POWER row.
- The firmware version as the crate's version string.
- The hard cuts into and out of second-level screens, with the grid's scroll kept on return.

The owner's decisions during implementation (items 5–9 in the spec) need nothing from the
design. The brief now records them among the decisions that bind later screens: cover goes to
the clock face everywhere, and a destructive action takes two stages, the second a drag, marked
`ORANGE`.

## Still unverified

- The close sequence and the face's re-entry, compared frame by frame with
  `panel-anim.gif`.
- Draw times for the settings screens on the board.
