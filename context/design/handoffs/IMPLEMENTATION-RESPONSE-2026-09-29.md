# Implementation response: the power key, and a power-off screen to design

For the design agent. It follows `IMPLEMENTATION-RESPONSE-2026-09-28.md`. The firmware now uses
the board's power key. The owner set the flow below. It needs one new screen, the power-off
confirmation, which is built as a placeholder until you design it. Captures are in
`context/screen-captures/`.

## What the hardware gives

- **One power key.** The power controller tells a short press from a long one, a hold of 1 s.
  The firmware learns of either up to about 250 ms late, so nothing on screen can follow the
  key as it goes down.
- **Powered off, the board is dark.** The clock chip and the GNSS module's backup memory are
  meant to stay powered; that is not yet checked. Holding the key 512 ms powers the board on,
  and it runs the normal start-up: self-test, identity and card.
- **A long hold forces the board off** in the power controller itself, whatever the firmware
  is doing: 6 s by default. Nothing on screen shows it.

## The flow, as the owner set it

- **A short press** rests the screen at once. It goes to the always-on face if ALWAYS ON is on,
  and dark otherwise. The timeout's 5 s dim is skipped. On a screen that is dimmed, on the
  always-on face or dark, a short press wakes it as a touch would. The short press has no screen
  of its own.
- **A long press** opens the power-off confirmation over whatever showed, from any face, the
  settings panel or a settings screen. From the always-on face or dark, it wakes the screen
  first, with the usual 250 ms fade up.
- **Swiping confirms.** The level then fades to dark over 300 ms, the panel switches off, and
  the board powers down. Nothing can take it back from there.
- **Cancelling** returns to exactly what showed before. A tap on a cancel button, a hand over
  the screen, or 10 s untouched cancels it. A short press cancels it too, and rests the screen.
- The screen timeout holds while the confirmation shows. The start-up ignores the key, so the
  press that powers the board on does nothing more.

## The placeholder

`power-off.png`, `power-off-sliding.png` and `power-off-confirmed.png` show it. It reuses parts
the design already has:
- the `POWER OFF` title in the settings title's place and type;
- the `CANCEL` button in the D3 top cap, where a tap anywhere in the cap presses it;
- the clear screen's orange slide, which must reach its target;
- two hint lines, `SLIDE TO POWER OFF` and `HOLD THE KEY TO WAKE`.

Once confirmed, the handle stays at the target, the button goes, and `POWERING OFF` replaces
the hints for the 300 ms fade. The placeholder has no entry, no icon and no scatter. Every
change redraws the whole panel.

## For the design

1. **The confirmation screen**: its layout, colour and entry. The owner fixed the interaction:
   a swipe confirms; a cancel button, a cover, a short press or 10 s cancel. The slide is the
   one confirming gesture the design already has. The swipe can be something else if it stays
   hard to trigger by accident.
2. **Whether it shows the 10 s** before it cancels itself.
3. **What shows during the 300 ms fade**, or whether the fade alone is enough.
4. **Its colour.** Powering off is deliberate and not a fault, so `RED` is out. The placeholder
   borrows the clear screen's orange.

Nothing else changed on screen.
