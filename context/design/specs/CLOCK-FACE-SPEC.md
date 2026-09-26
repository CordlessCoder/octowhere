# Clock face specification

The clock face that replaces the `CLOCK` page, between Motion and Touch in the ring. It follows
`SCREEN-DESIGN-BRIEF.md` and the doctrine. It is the industrial version of direction B. Hours sit
on the field beside the status symbol at 96 px. The minutes are knocked out of a white band that
crosses the whole circle. A ruled plate carries the zone.

The concept images are in `images/`, named `clock-*.png`, one per state, at 466 × 466. The panel
is drawn on pure black, and the dark grey outside the circle is off-panel, as in the compass
captures. `clock-overview.png` shows every state on one sheet with labels. The entry and exit
animations are `anim-B.gif` (real time), `anim-B-slow4x.gif`, `anim-B-entry-frames.png` (every
20 ms) and `anim-B-resync.gif`.

Every image uses the same fixture data: local time 13:07:42 on Thu 24 Sep 2026, zone
Europe/Dublin (IST, +01:00), automatic, clock set from GNSS since boot. UTC is 12:07. None of it
is a reading.

## Assumptions

- The time is 24-hour. There is no setting for a 12-hour clock, and none is proposed.
- The screens receive "no zone" as an absent zone, in automatic mode before the first fix the
  device has ever had. The brief describes the condition but not how it arrives.
- Setting the clock clears the stopped flag, so "stopped since last set" means the time is still
  unset.
- The zone counts and neighbours in the picker images come from the tz database in my build
  environment (447 zones), not the firmware's 444. `26` zones at +01:00 and the neighbour names
  are illustrations.
- The animation timings assume a frame every 20 ms. The compass draws a full frame in 13–23 ms,
  and this face has not been measured.

## States

Precedence runs top to bottom. The first state whose condition holds is shown.

| State | Condition | Icon | Band label | Hours | Band | Date row | Plate |
| --- | --- | --- | --- | --- | --- | --- | --- |
| NO DATA | The clock could not be read | `RED`, no data | `CLOCK` | none | `RED`, `NO DATA` | empty | as normal |
| STOPPED | The clock stopped since it was last set | `ORANGE`, stopped | `STOPPED` | `--` | `ORANGE`, `--` | `WAITING FOR GNSS` | as normal |
| NO ZONE | Automatic, and no zone has ever been found | `WHITE`, no zone | `NO ZONE` | `--` | `WHITE`, `--` | `UTC 12:07` | `AUTO` `NO FIX YET`, no name line |
| LOCAL, GNSS | Otherwise, and GNSS has set the clock since boot | `BLUE`, GNSS | `LOCAL` | `13` | `WHITE`, `07` `42` | `THU 24 SEP 2026` | `AUTO` `IST` `+01:00` / `EUROPE/DUBLIN` |
| LOCAL, RTC | Otherwise | `GRAY`, RTC | `LOCAL` | as above | as above | as above | as above |

- NO DATA turns the ring `RED` as well. It keeps the plate, because the zone does not come from
  the clock.
- STOPPED and NO ZONE withhold the local time, the seconds and the date. Local time is never
  shown without a zone, and UTC is never shown in the local time's place.
- NO ZONE shows UTC in the date row, prefixed `UTC`. This is an open choice (see the end).
- LOCAL, RTC is not a warning. It says only that GNSS has not confirmed the clock since boot.
  The time on a clock that has not stopped is normal.
- Manual mode changes only the plate's tag: `MANUAL`. Manual mode cannot reach NO ZONE.

Files: `clock-local-gnss.png`, `clock-local-rtc.png`, `clock-local-manual.png`,
`clock-stopped.png`, `clock-no-zone.png`, `clock-no-data.png`, `clock-swiping.png`.

## Layout

All positions are in panel pixels. "Ink" rows and columns were measured from the renders. Pens
and baselines are what the firmware should use. The ink follows from them, give or take a pixel
of rasterizer difference.

| Element | Position | Treatment |
| --- | --- | --- |
| Perimeter ring | radius 230–232 | `GRAY`; `RED` in NO DATA. Drawn first |
| Hours | pens x 62 and 144, baseline 184; ink rows 90–185, x 71–217 | Fraktion Mono Bold 136 px, `WHITE` on `BLACK`. Tabular, two digits always |
| Icon, 96 × 96 at x 262 | rows 90–185 | Outlined, 16 px modules, 8 px padding, 4 px frame. It fills the hours' cap height, top and bottom |
| Band | rows 198–317, clipped to the page's circle of radius 232 | Solid fill across the page, ending where the ring's outer edge does. Drawn over the ring. `WHITE`, `ORANGE` in STOPPED, `RED` in NO DATA |
| Band label | pen x 261, baseline 225; ink rows 214–225, from x 262 | Mono Bold 16 px `BLACK` on the band: `LOCAL`, `STOPPED`, `NO ZONE`, `CLOCK` |
| Minutes | pens x 62 and 144, baseline 305; ink rows 211–306 | Mono Bold 136 px, `BLACK` on the band. The same columns as the hours |
| Seconds | pens x 260 and 284, baseline 305; ink rows 277–306, from x 262 | Mono Bold 40 px, `BLACK` on the band. Shares the minutes' baseline |
| Withheld digits | the same pens and baselines | `--` in the same face, size and colour as the digits they replace |
| `NO DATA` | ink left 71, centred by ink on row 257.5 (rows 246–267) | Marathon Shapiro Wide 65, 28 px, `BLACK`. Left-aligned with the digits |
| Date row | centred by ink on x 233, ink top 334 (rows 334–350) | Mono Regular 23 px `WHITE`. `WAITING FOR GNSS` 19 px `GRAY`; `UTC HH:MM` 23 px `GRAY` |
| Plate | rows 360–379, cells centred as a group on x 233 | See below |
| Zone name | centred by ink, baseline 398 (ink rows about 388–401) | Mono Regular 14 px `GRAY`. The IANA name in capitals, underscores kept |

The plate is a row of cells, 20 px tall, with 4 px between cells:

- Each cell is as wide as its text's advances plus 6 px on each side (8.4 px per character at
  14 px).
- The mode tag (`AUTO` or `MANUAL`) is filled `GRAY`, with Mono Bold 14 px `BLACK` text.
- The value cells (abbreviation, then offset) have a 1 px `GRAY` frame, a `BLACK` inside and
  Mono Regular 14 px `WHITE` text.
- Text pens start 6 px inside each cell, on baseline 375.
- For Europe/Dublin the cells are x 156–201, 206–242 and 247–308.
- The widest case, `MANUAL` `+0530` `+05:30`, spans x 140–325.

Rules:

- Hour and minute digits sit on fixed columns: the pen of the second digit is the first plus the
  advance, 81.6 px at 136 px. A changing digit never moves its neighbour.
- The date is always 15 characters: weekday, a two-digit day, month and year
  (`THU 01 OCT 2026`). It keeps one width, so centring it never moves it.
- Nothing moves between states. The band label, the date row and the plate keep their space.
  Plate cells change width with their text, since they are centred as a group.
- Every antialiased edge is on a known flat colour. The hours, icon, date and plate are on
  `BLACK`. The band label, minutes, seconds and `NO DATA` are on the band's colour. The plate's
  text is on its own cell colour.
- The face has no dial, no rotated text and no polygons. It uses solid rectangles, the ring and
  upright text only.

Worst cases, measured:

| String | Width | Room |
| --- | --- | --- |
| Zone name, `AMERICA/ARGENTINA/BUENOS_AIRES` (30 characters, the longest) | 251 px, x 108–358 | corners at radius 210, inside 226 |
| Plate, `MANUAL` `+0530` `+05:30` | 186 px | corners inside radius 226 |
| Date, `WED 30 SEP 2026` | 205 px | 386 px of chord at radius 226 on row 350 |

A script checked every element in every image. Apart from the ring and the band, all ink stays
inside radius 226.

## Symbols

The icon is the shared 5 × 5 system, with the same grid, bits and meanings as the compass's
icons. Each row below is five bits from left to right, and 1 is a module.

Every icon is drawn outlined, on this face, in the picker and on the compass. The owner chose
this on 24 Sep 2026 over the filled tiles. An outlined icon has three parts:

- a frame in the state colour
- `BLACK` inside the frame
- the modules in the state colour

The frame is a quarter of the module, rounded half up, and never below 2 px. That gives 2 px at
5 and 8 px modules, 3 px at 10 px and 4 px at 16 px. `study-outlined-icons.png` compares filled
and outlined on every state.

| Symbol | Colour | Rows | Meaning |
| --- | --- | --- | --- |
| GNSS | `BLUE` | `00100 01010 10101 01010 00100` | Set from GNSS since boot |
| RTC | `GRAY` | `11111 10001 10101 10001 11111` | Running on the RTC, not confirmed since boot |
| Stopped | `ORANGE` | `01010 01010 01010 01010 01010` | The clock stopped since it was last set |
| No zone | `WHITE` | `01110 10001 00110 00000 00100` | Automatic, no zone yet |
| No data | `RED` | `11100 11000 00100 00011 00111` | The compass's symbol, reused unchanged |
| Offset | `WHITE` | `11011 10001 00100 10001 11011` | The zone setting, in the picker |

- On this face the icon is 96 × 96: 16 px modules, 8 px padding from the tile's edge, a 4 px
  frame. The module is 16 px, not 15, so that the tile fills the hours' 96-row cap height
  exactly.
- The offset glyph is a bracket closing on a point. `study-zone-glyph.png` compares it with a
  pin, two globes and meridian stripes. The stripes were too close to STOPPED's pause bars.
- `BLUE` keeps the compass's meaning of a valid live reading.
- `GRAY` as an icon colour is new. It marks a value that is valid but unconfirmed, which the
  compass has no case for.
- The size and the frame are parameters of one drawing: module, padding and frame. The icon is
  never redrawn per size.

## What changes, and how often

This table is the input for the face's change tracking. Until that tracking exists, every change
redraws the whole face, as the placeholder screens do.

| Element | Changes | Region |
| --- | --- | --- |
| Seconds, units digit | every second | one glyph cell, 24 × 30, on the band |
| Seconds, tens digit | every 10 s | one more cell |
| Minutes, units digit | every minute | one glyph cell, about 82 × 96, on the band |
| Minutes, tens digit | every 10 min | one more cell |
| Hours | every hour | one or two cells on `BLACK` |
| Date row | at local midnight | the row |
| Time and date row, typing in | during the reveal after the time is replaced | each part's cells, the row |
| `UTC HH:MM` (NO ZONE) | every minute | the row |
| Icon | on a state change; during its build | the 96 × 96 tile, or one row of it per build step |
| Band label, plate, zone name | when their text changes; during their reveal | one character cell or plate cell per step |
| Everything | on a state change that recolours the band, a page swipe, or the ring fade | full frame |

In steady state a second costs one small region. On the compass, the change nearest in size is
a tilt change, measured at 1.7 ms. Nothing on this face has been measured.

The screens read the clock about every 250 ms, so a seconds digit lands up to about a quarter
second late and unevenly. The face shows whole seconds as digits, which the data supports. It
has no sweep.

## Motion

The time, the band, the icon's frame and the date are centre content. They arrive and leave with
the page, so the face reads when a swipe's edge cuts through it. The accents are the ring, the
icon's black modules, the band label, the plate and the zone name. They build after the page
settles and unbuild as it leaves.

The band is clipped to the page's own circle, radius 232 about the page's centre, so it ends
exactly where the ring does. At rest the glass crops the band at nearly the same place, so the
clip only shows during a swipe. There the band's ends become arcs that travel with the page,
instead of straight cuts running past the page's silhouette. `study-swipe-band.png` shows
frames without the clip (top) and with it (bottom). In page coordinates the clip never
changes, so the band can always be drawn clipped, with one code path.

Two primitives do all of it. Both are rectangles and upright text the firmware already draws.

- Icon build. The frame and its black inside are drawn whole. The modules are drawn one row
  at a time, top down, every 30 ms. Any glyph takes five steps, 150 ms.
  `study-icon-build-orders.png` compares this with module-by-module order (up to 340 ms, and
  the length depends on the glyph) and centre-out order (three steps, the first often empty).
- Cell reveal. Mono text is revealed left to right over a fixed duration. At progress k (0 to
  1) of a string of n characters, characters with index below k(n + 1) − 1 show their glyph.
  The next one shows a solid block in its colour: one advance wide less 1 px on each side, from
  the baseline up to cap height. Later characters show nothing. A block stands only where a
  character will be, so no frame shows a wrong character. Plate cells appear whole, one at a
  time, as their own k passes each cell's share.

Digits animate only when the time is replaced, below; a tick never animates them. The face
never shows a time, date or zone value that is not the current one.

### Entry

Once the page settles, times in ms from settling:

| Accent | Start | Duration | How |
| --- | --- | --- | --- |
| Ring | 0 | 110 | Fade from `BLACK`, stepped, as on the compass |
| Icon modules | 60 | 150 | Row build; rows land at 60, 90, 120, 150 and 180 |
| Band label | 100 | 120 | Cell reveal |
| Plate | 160 | 120 | Cells land at 160, 200 and 240; their text appears with them |
| Zone name | 240 | 160 | Cell reveal |

The sequence is complete at 400 ms. During the drag in, the entering page shows only the centre
content. The icon is an empty frame.

### Exit

Going out, the accents are driven by the page's offset, not by time. p runs from 0 at rest to 1
at 35 % of the width, the compass's fade distance. Reversing a drag restores them continuously.

| Accent | Over p | How |
| --- | --- | --- |
| Zone name | 0–0.2 | Reveal run backwards, right to left |
| Plate | 0.1–0.4 | Cells removed, last first |
| Band label | 0.2–0.5 | Reveal run backwards |
| Icon modules | 0.3–0.8 | Rows removed, bottom first, leaving the empty frame |
| Ring | 0.6–1.0 | Fade to `BLACK` |

If a swipe starts before the entry has finished, each accent shows the lesser of its entry
progress and its exit progress, so nothing jumps forward.

### Changes while the page shows

- A state change that keeps the band's colour, such as RTC to GNSS when GNSS sets the clock,
  recolours the frame in one frame and rebuilds the new glyph by rows (`anim-B-resync.gif`). The
  band label and plate re-reveal only if their text changed.
- Entering NO DATA is instant, with no build and no reveal. Leaving NO DATA runs the normal
  build.
- A zone change, such as a new automatic lookup or a DST transition, re-reveals the plate and
  the zone name.
- A time that is replaced rather than ticked types in again by cell reveal. That covers a first
  fix out of NO ZONE, a fix that corrects the clock, a zone change that moves the offset, and
  leaving STOPPED or NO DATA. The hours, minutes and seconds reveal as one string of six cells
  over 180 ms, and the date line over 160 ms from 120 ms. A shown time that moves more than 2 s
  from the time elapsed since it last changed counts as replaced. A fix or zone change that
  leaves the time as it was changes no digit. This replaces hard replacement of the hours, by
  the owner's decision of 2026-09-24.

### Rejected option

`anim-A.gif` is the compass's rule applied unchanged: ring, icon, plate and zone name fade in
after settling, staggered (0, 95, 190 ms), and fade with the offset on the way out. It works,
but a 96 px tile fading in reads as a flash of dim blue. And fading the black band label on
white would need an interpolation toward the band colour, which the firmware does not have.

## Gestures on the face

- Horizontal drags change page, as everywhere.
- Nothing else changes the face: taps, vertical drags other than the settings panel's opening
  drag, and two-finger contacts. The zone is chosen from a settings panel, so an accidental touch
  never starts changing it.
- Covering the screen goes to the clock face from any screen, and does nothing on the face
  itself (owner's decision, 2026-09-24).

## Zone picker

The design owns how a zone is chosen. The picker opens from a settings panel, which does not
exist yet and is not designed here. The picker is not a page in the ring and is not reached from
the face. It takes two steps.

1. Offset, chosen by local time. The user drags until the time in the band matches the clocks
   around them. That picks one of the 38 or so distinct UTC offsets in force today, not one of
   444 names.
2. Zone within that offset, nearest first. The first row is almost always right. Any zone at
   that offset is reachable by dragging.

Returning to automatic is one tap on the first step.

Files: `clock-picker-offset.png` (automatic in force), `clock-picker-offset-manual.png`,
`clock-picker-offset-stopped.png` (time unknown), `clock-picker-zone.png`,
`clock-picker-zone-sea.png` (end of the list).

The picker follows one rule: filled means reading, outlined means editing. Where the face has a
filled band, the picker has an open field between two white rules, with white text on black.
Its symbol is the offset glyph, as a 96 px icon in the face's icon slot. The picker needs no
colour beyond the existing tokens.

### Shared layout, both steps

| Element | Position | Treatment |
| --- | --- | --- |
| Ring | radius 230–232 | `GRAY` |
| Hint | centred by ink, ink top 48 | Regular 14 px `GRAY` |
| `CANCEL` / `BACK` | box x 72–181, rows 104–142 | 2 px `GRAY` border, `BLACK` inside, label Bold 16 px `WHITE` centred by ink |
| Icon, 96 × 96 at x 262 | rows 90–185 | As on the face, in `WHITE`, with the offset glyph |
| Field rules | rows 198–201 and 314–317, clipped to the page's circle of radius 232 | `WHITE`. The face's band rows, opened |
| Field locators | 3 × 12 px at x 48 and 400; rows 202–213 and 302–313 | `WHITE`. They mark where the editable text sits |
| Neighbour rows | ink left 62, ink centred on rows 174 and 341 | Regular 23 px `GRAY`. Empty past either end of the list |

### Step 1: offset

| Element | Position | Treatment |
| --- | --- | --- |
| Hint | as above | `DRAG TO YOUR LOCAL TIME` |
| Button | as above | `CANCEL` |
| Neighbour rows | as above | `HH:MM  ±HH:MM` |
| Selected time | pens x 57, 109, 148, 188, 240; baseline 289 | `HH:MM` in Mono Bold 86 px `WHITE`. The colon has a 28 px cell, centred on the tabular column |
| `UTC` | ink left 311, ink top 228 | Bold 16 px `GRAY` |
| Selected offset | ink left 311, ink bottom 288 | `±HH:MM`, Bold 24 px `WHITE` |
| `AUTO` | box x 183–281, rows 366–404 | Filled `GRAY` with a `BLACK` label while automatic is in force, as the face's plate tag. Otherwise outlined like `CANCEL` |

- The rows are the distinct UTC offsets currently in force across the zone set, in ascending
  order. There are 38 today.
- Each row's time is UTC plus that offset, updated every minute. With the time unknown (STOPPED
  or NO DATA), rows show `--:--` and only the offset tells them apart.
- It opens on the offset in force. With no zone, it opens on +00:00.

### Step 2: zone

| Element | Position | Treatment |
| --- | --- | --- |
| Hint | as above | `+01:00  NEAREST FIRST`, or `+01:00  A-Z` with no position |
| Button | as above | `BACK` |
| Neighbour rows | as above | City |
| Selected city | ink left 62, ink bottom 257 | Mono Bold 40 px `WHITE`. The last part of the IANA name, capitals, `_` shown as a space |
| Selected name | ink left 62, ink top 276 | IANA name, two spaces, abbreviation. Bold 16 px `GRAY` |
| Position | centred by ink, ink top 381 | `n/N`, Regular 14 px `GRAY` |

- The rows are the zones whose current offset is the one chosen in step 1.
- Nearest first means by distance from the last GNSS position to each zone's reference point.
  Zones without one go last: the `Etc` zones, and any others the table lacks. With no position,
  the order is A to Z by city.
- An `Etc` zone shows `AT SEA` as its city. Its second line keeps the IANA name, whose sign is the
  reverse of the offset: `ETC/GMT-1  +01` is UTC+01:00. The abbreviation shows the true offset.
- It opens on the zone in force if that zone is at this offset, and on the first row otherwise.
- Worst cases fit. A 14-character city at 40 px is 331 px wide (ink x 62–393). The longest name
  line, 30 characters plus an abbreviation at 16 px, is 335 px (x 62–397). The field is about
  450 px wide on those rows.

### Picker gestures

The targets are regions across the whole chord, larger than the boxes drawn in them. Nothing is
smaller than about 10 mm.

| Region | Rows | Tap |
| --- | --- | --- |
| Top cap | 0–150 | `CANCEL` on step 1: back to the settings panel, nothing stored. `BACK` on step 2: back to step 1 |
| Band | 186–329 | Step 1: go to step 2 with this offset. Step 2: store this zone as the manual zone and go back to the settings panel |
| Bottom cap | 356–465, step 1 only | Store automatic and go back to the settings panel |
| Anything else | | Nothing. Taps on a neighbour row do not select it |

- A vertical drag, starting anywhere, steps the list. A drag up brings the next row into the
  band: one step per 40 px of travel, as the finger passes each threshold. On release faster than
  0.5 px/ms, stepping continues and slows to a stop. Both numbers are starting values to tune on
  the panel.
- Steps are hard replacements. Rows do not slide.
- Covering the screen, on either step, discards the choice and goes to the clock face, as on
  every screen (owner's decision, 2026-09-24). It counts once per hand: a second cover counts
  after 260 ms without a report, or after a finger touches. This gives an exit that does not
  depend on finding the top cap.
- A horizontal drag does whatever it does on the settings panel. If it leaves, the picker is
  discarded without storing anything.
- Two-finger contacts are unused in the picker. A two-finger drag that steps ten rows at a time
  is possible, but with a fling it adds a gesture without adding reach.
- Storing automatic when the device has never had a fix puts the face into NO ZONE.
- Each picker step is a hard cut in one frame. The field sits on the face's band rows, so the
  picker reads as the clock's own setting opened for editing.
- On entering the picker, the icon builds by rows as on the face (150 ms), and the hint
  reveals by cells. The field and its contents arrive at once.

## Firmware work this design needs

Each item is firmware work, and each is the owner's decision.

1. Zone enumeration for the screens: the distinct offsets in force now; the zones at a given
   offset; each zone's IANA name and current abbreviation.
2. A write path from the screen to the stored zone setting: set a manual zone by name, and set
   automatic.
3. For nearest first, two things. First, the last GNSS position passed to the screens, which is
   grade 2 today. Second, a reference coordinate per zone. `zone1970.tab` gives one per zone, and
   two `i16` values at 0.01° for 444 zones take under 2 KB. Without these, step 2 falls back to
   A to Z and still works.
4. Vertical drags that step by distance, with a fling. The axis check steps on whole swipes,
   which would be too slow for 38 or more rows.
5. A settings panel that opens this picker. How it is reached is not designed here.
6. Change tracking for the face, built from the table above.
7. An accent timeline that runs after the page settles and one driven by the swipe offset,
   generalised from the compass's fades to per-step builds and reveals. The steps themselves are
   rectangles and upright glyphs.
8. A band clipped to a circle. Each row is one span, whose ends are where the row meets
   radius 232, with one antialiased pixel at each end. The ring's analytic coverage gives the
   end pixels. It costs a rectangle fill plus two edge pixels per row.
9. Mono Bold at 136 px, the largest text on the device so far (the compass readout is 86 px).
   What a 136 px glyph costs to rasterize and cache is unknown. I'd ask for a measurement of one
   minute change before relying on the cost estimate above.

The face needs no new glyphs, because everything is printable ASCII. It needs no arcs, rotated
text or polygons.

## Compass changes

This is a proposal for the approved compass, following decisions 8 and 10. Only the centre stack
changes. The ring, dial, letters and every behaviour stay as they are. The images are in
`images/compass-icon66-*.png`, one per state, and `compass-proposed.png` shows all five.

N E S W orbit on radius 172 at 40 px, so their inner edge is near radius 157. The stack must
keep its corners inside that. A 66 px icon centred at the top of the stack is the largest that
fits.

| Element | Approved | Proposed |
| --- | --- | --- |
| Icon | 33 × 33 filled at x 217, rows 102–134 | 66 × 66 outlined at x 200, rows 87–152: 10 px modules, 8 px padding, 3 px frame |
| Caption | ink top 142 | ink top 160 |
| State line band | rows 161–178 | centred on row 187.5 |
| Slab, 196 × 91 at x 135 | rows 186–276 | rows 204–294 |
| Tilt | ink top 293 | ink top 311 |
| Divider, x 138–328 | row 321 | row 336 |
| Hint | ink top 328 | ink top 342 |

- Everything below the icon moves down 18 px, except the divider and hint, which move 15 and
  14 px. The divider's gap closes by 3 px so that the hint stays inside radius 150.
- The slab still never moves or resizes between states, and the icon and caption still never
  move. The compass's own rules hold. Only the numbers change.
- The stack's corners reach radius 149, at the icon. A sweep turned the heading through a
  full circle in 3° steps and found no letter within 3 px of the stack at any heading. A block
  planted at radius 150–165 was caught at heading 0, so the sweep does detect contact.
- A 52 px icon also fits (`study-compass-icon.png`), reaching radius 147 with the stack moved 5
  px. At 66 px the compass's icon matches the clock's in weight, and it still fits.
- The icon's entry fade becomes the row build, to match the clock. The compass's other accents
  keep their fades.

## Decisions

The owner settled these on 24 Sep 2026.

1. The zone is chosen from a settings panel, not from the face. A tap on the face was too easy
   to trigger by accident. The face has no touch actions besides changing page.
2. The picker uses the outlined field and an outlined 96 px icon, with no new colour token. The
   alternatives were six colours from the reference board and a white band with a symbol
   (`study-picker-treatment.png`). The outlined field extends to every future setting: filled
   means reading, outlined means editing.
3. NO ZONE shows UTC in the date row, labelled `UTC` in `GRAY`. It is the only real data the
   face has in that state, and it cannot be confused with the 136 px local time.
4. The `GRAY` RTC icon stays. Without it, `BLUE` would show in nearly every state and carry no
   meaning.
5. The list steps instead of scrolling continuously. It is cheaper and fits hard replacement.
   Revisit on the panel if it feels crude with the fling.
6. There is no DST indicator. The offset and abbreviation already carry it. For Europe/Dublin
   the flag is unreliable: in the tz database's main data, Irish summer time (IST) is standard
   time, and winter GMT is the daylight-saving period with a negative offset. How the zone data
   was compiled decides whether the flag reads "on" in summer or in winter.
7. A mode in force is filled `GRAY` everywhere: the face's plate tag and the picker's `AUTO`.
8. Every icon is outlined, the compass's included. At 96 px the outlined icon carries more
   weight than the filled tile. The cost is less saturated area. The bands carry the colour
   where a state needs to be loud.
9. The picker's glyph is the offset bracket.
10. The compass gets a larger icon. The proposed layout is in the section above.
11. The band, and the picker's field rules, are clipped to the page's circle, so a swipe
    carries a round page.

Still open, outside this spec: how the settings panel is reached, and what else it holds.

## Prototypes, not specified

More expressive variants in the same family, for comparison only. None of these covers states
other than normal.

- `ind-v2-crop.png`: digits at 230 px, cropped by the edge, with a metadata rail between hours
  and band.
- `ind-v3-wipe.png`: the band fills with the seconds, with a 5 s scale on its top edge.
- `ind-v4-display.png`: `13:07` in Shapiro in the band, over a 60-cell seconds rail.
- `proto-*.png`: the first, rougher set.
- `direction-a-dial.png` and `direction-c-analogue.png`: the directions not taken.
- `study-icon-size.png`: the 5 × 5 icon at 33, 52, 66 and 96 px on this face.
- `study-picker-icon.png`: the picker's icon filled and outlined, with the first zone glyph.
- `study-outlined-icons.png`: every state's icon filled and outlined, on both screens.
- `study-compass-icon.png`: the compass at 33, 52 and 66 px.

## Verification record

What I checked:

- The renderer was calibrated against `compass-heading.png` by redrawing the compass from the
  brief's table. The caption, tilt and hint ink land on the brief's rows: 142–153, 293–309 and
  328–337. With the supplied Fraktion Mono Bold, the readout digits fill exactly columns
  149–291. They sit 3 px higher, only because the calibration centred them by ink rather than by
  the firmware's baseline.
- A script checked all 12 state and picker images. The ink of every measured element except the
  ring, band and field rules is inside radius 226. `RED` appears only in NO DATA. `YELLOW`
  appears nowhere. Rigged cases confirmed the checks fire.
- The plate's cells and the zone name are not measured by that script. I computed their boxes
  separately for the fixture and the worst cases, and checked them against the same radius.
- I looked at the animation frames every 20 ms, for entry and for a swipe out, and at swipe
  frames with the band clipped and unclipped.

What I did not check:

- Drawing by the firmware's own code, or viewing on the panel. The concept renderer is Pillow at
  4× oversampling, box-filtered down. Its antialiasing and hinting are not the firmware's, and
  ink edges may differ by about a pixel.
- Any draw or transfer cost, or the frame rate the animations assume.
- Whether the firmware's 444 zones give the same 38 offsets.
