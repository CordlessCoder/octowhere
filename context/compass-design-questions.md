# Compass screen: questions for the design agent

The compass screen now implements `compass-implementation-handoff.md`. These are the places where
the handoff left a choice open, or where the implementation raised something the handoff does
not cover. Each says what the firmware does today, so an answer can be "keep it". Renders of
every state come from the command in `COMPASS-SCREEN-HANDOFF.md`; ask the owner for them.

## Layout

1. **`N` above the status icon.** Near heading 0°, the `N` at r 172 sits about 7 px above the
   icon, and the `N`, icon and caption stack into one column. At 047° as in the concept they do
   not meet. Is the stacking acceptable, or should the letters sit further out, or smaller?
2. **Cardinal letter size.** The handoff gives r 172 but no size. The firmware uses Shapiro
   40 px, which matches the concept by eye.
3. **`NO DATA` size.** Unspecified. The firmware uses Shapiro 28 px, centred in the slab by its
   ink, leaving about 8 px each side.
4. **`---` in TOP EDGE UP.** Drawn in the readout's style, PP Fraktion Mono Bold 86 px, from the
   readout's first pen position, with no suffix. Should it carry a `°`, or sit centred in the
   slab instead?
5. **Weights not stated.** The firmware sets the readout, suffix, captions, direction
   abbreviation and `INTERFERENCE` in Bold, and `TURN ALL WAYS`, `TOP EDGE UP`, the tilt and
   the hint in Regular. The concept looks the same. Confirm.
6. **Caption colours.** `MAGNETIC` and `COMPASS` are `GRAY` and `CALIBRATION` is `ORANGE`, as in
   the concept. The NO DATA caption could be `RED` instead. Which?
7. **Icon and caption position.** At the owner's request the icon moved to y 87 and the caption
   baseline to 138, so the icon, caption and slab inks stand 7 px apart. The handoff's 83 and
   132 gave gaps of 5 and 13 px. Likewise the direction abbreviation grew from 32 to 36 px on
   the same baseline, so it sits 16 px from both the slab and the tilt line. The interference
   line got the same 36 px, shortened to `INTERFERENCE`: `MAG INTERFERENCE` at that size
   reaches the turning `E` or `W`. Keep all of these in future revisions.

## Motion

8. **State changes on the settled page.** The handoff specifies fades for entry, swipe-out and a
   heading arriving after calibration. Everything else changes in one frame:
   - heading to interference and back: slab, icon, status and caption colours swap at once;
   - `live` lost: the ring turns red and the dial disappears at once;
   - heading lost to TOP EDGE UP: the dial disappears at once.

   Is instant right for all of these?
9. **Heading regained after TOP EDGE UP.** A returning heading runs the reveal again: ticks over
   170 ms, letters starting 70 ms later. Tilting the board through vertical can drop and regain
   the heading repeatedly, so the dial may pulse. Should a quick return skip the reveal, and if
   so, after how long an absence does the reveal apply?
10. **Swipe-out source.** The firmware takes `p` from the page's offset, not the finger. So the
    fade also follows the page as it settles after release, back to full or on to the next page.
    That keeps it continuous with no second timer. Confirm this matches the intent.
11. **Fade steps.** Each 110 ms fade lands in about 6 to 8 frames at the measured 13 to 17 ms
    draw times, since the fade rides on full redraws. The owner will judge the steps on the
    panel. Is a coarser fade acceptable, or should the durations grow if it looks stepped?

## Behaviour the design depends on

12. **Cover release.** The firmware takes one cover per hand: a second cover counts only after
    a touch report without one in between. If the touch controller does not report the hand
    lifting, the next cover waits until the screen is touched. Is that acceptable from the
    user's side, or should the hint say more?
