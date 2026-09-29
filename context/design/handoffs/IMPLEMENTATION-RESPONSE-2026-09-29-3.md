# Implementation response: the power-off confirmation is built

For the design agent. It answers `context/octowhere-poweroff-implementation-handoff-2026-09-29/`
and replaces the placeholder `IMPLEMENTATION-RESPONSE-2026-09-29.md` described. `DECISIONS.md`
entry 14 records it. Captures are `power-off.png`, `power-off-sliding.png`,
`power-off-confirmed.png` and `power-off.mp4` in `context/screen-captures/`.

## How it was checked

The firmware's renders were compared with `references/power-off-rest.png`, `-sliding.png` and
`-confirmed.png` by the ink box of every element. These match to a pixel:

- the title;
- the rules at y 83, 145 and 370;
- CANCEL at x 83–182, y 96–139;
- the icon tile at x 330–365;
- `SLIDE TO POWER OFF` at x 92, y 164;
- the slider band at x 83–382, y 225–288;
- both help lines at y 323 and 346;
- `POWERING OFF` at y 327.

The icon's glyph and orange match. The arrow's shaft and head follow the study's geometry,
with the head's edges antialiased.

## Built as specified

- **Entry.** A long press cuts to the confirmation from any face, the settings panel or a
  settings screen. From dim, the always-on face or dark, it fades up onto it over the 250 ms
  wake, with no clock frame first.
- **Slide.** It follows the finger, from the handle's start to the target, and confirms only
  when released with the handle's middle over the target, the CLEAR SETTINGS rule. Released
  short, the handle goes back to its start and the confirmation stays. A tap does nothing.
- **Confirmed.** The handle stays at the target with the track full. CANCEL, the action line,
  the hints and the footer go, and `POWERING OFF` shows in white. The level fades to black
  over 300 ms, then the panel goes off and the board powers down. Nothing reverses it.
- **Cancel.** A tap anywhere in the top cap, a cover, or 10 s untouched. Every touch restarts
  the 10 s. After a wake from rest, the 10 s start once the screen is fully up. The screen,
  page and scroll under the confirmation are kept.
- **Display state.** A cancel puts the screen back as it rested before the key woke it:
  dimmed, darkening, on the always-on face or dark. A short press cancels and then rests the
  screen. The screen timeout holds while the confirmation shows.
- **Start-up.** It ignores the key, so the press that powers the board on opens nothing.
- **Orange** is the shared `ORANGE` token. Nothing on the screen is red.

## Where the build interpreted the hand-off

1. **No perimeter ring.** The owner removed the ring from every screen on 2026-09-27
   (`DECISIONS.md` 4b). The study still draws it.
2. **The scatter** is the settings pages' scatter, unchanged, as the hand-off asks. The slider's
   rows are kept clear of it, so no mark shows between the handle and the target.
3. **A handle partly over the target** is drawn over the target's black. The study redraws
   the target on top until the handle arrives, which would hide the arrow in the last 60 px.
4. **Breadcrumb and footer size.** `SYSTEM / POWER` and `AUTO CANCEL / 10 S` use the settings
   pages' 14 px hint style, as every settings screen does, where the study sets them at 12 px.
   They come out 16 and 22 px wider.
5. **The fade** dims the panel's level rather than blending the pixels. It darkens everything
   uniformly, as the study's blend does, without redrawing.
6. **Cancelling a dim or darkening screen** restarts that step, so the screen dims or darkens
   again from its start rather than from where it was.

## On the device

Not yet checked. The board was not connected when this was built. These are still open:

- the contrast of the gray hints and the orange at the chosen brightness;
- the release hitbox, and whether a cover or the cap wins over a drag;
- whether the fade reaches a dark panel before power is removed.

## For the design

1. Review `power-off.mp4`: a slide released short, a cancel, a short press to dark, a long press
   fading up onto the confirmation, and a slide that powers off.
2. Say whether the breadcrumb and footer should drop to 12 px here, and on the settings screens
   with them.
