# ui-faces review of `crates/octowhere-ui` at b8ab7ca

Reviewer: ui-faces. Worktree `/home/me/.claude/jobs/173b5f0c/tmp/review-b8ab7ca`; paths below are
relative to it, line numbers at `b8ab7ca`. No build was run; every count comes with its command.

Counts: Bugs 2, Structure 3, Duplication 11, Names and types 6, Unstated invariants and magic
numbers 5, Comments 4, Tests 4, Work and stack 2, Public surface 2. Total 39.

## Bugs

### The identity's title stays on the heap after a skipped start-up
- Where: `crates/octowhere-ui/src/ui/identity.rs:581-584`, `:625-635`, `:659-662`, `:897`;
  `crates/octowhere-ui/src/ui/startup.rs:311-315`, `:405-413`; `crates/octowhere-ui/src/ui/stage.rs:1883-1890`
- Category: Bugs
- Weight: high
- What: The title's `Recording` (36,660 bytes reserved, plus the `Title` box) lives in a static and
  is freed only by `forget_title()`, which only `draw_card` calls. A touch during the identity sets
  `skipped`, the phase goes straight to `Done`, no card frame is ever drawn, and the title is never
  freed. It then holds at least 36,660 bytes of the 192 KiB internal heap for the rest of the run. A replay from the device page that is
  skipped does the same. `forget_title`'s own doc says "once the identity is over", which a skip is.
- Evidence:
  ```rust
  static TITLE: embassy_sync::blocking_mutex::Mutex<..., core::cell::RefCell<Option<Box<Title>>>> = ...;
  /// Lets the title's coverage go, once the identity is over.
  fn forget_title() { TITLE.lock(|title| title.borrow_mut().take()); }
  pub fn draw_card<D: CoverageTarget<Color = Color>>(frame: u32, target: &mut D) -> ... {
      ...
      forget_title();
  ```
  `startup.rs:410-413`: `(false, true) => Phase::Done { entry: false, after_card: false }`, so
  `View::Card` (the only route to `draw_card`, `startup.rs:442`, `:531`) never comes.
  `grep -rn "forget_title" crates tools firmware/src` finds only the definition and the call in
  `draw_card`.
- Change: Own the title where the start-up's state lives (a field the stage drops with `Startup`
  at handover, `stage.rs:1886`), not in a static; failing that, have the stage call a public
  `identity::forget_title()` on every exit from `Phase::Identity`.

### The outline example labels outlines as hollow text
- Where: `crates/octowhere-ui/examples/outline.rs:1-2`, `:23`, `:66`, `:87-101`
- Category: Bugs
- Weight: low
- What: The sheets are documented and labelled as "the hollow ring and a halo", with tiles
  `HOLLOW, 1 PX` and `HOLLOW, 2 PX`, but both branches call `draw_outline_on_baseline`. The hollow
  path the identity's title uses (`draw_hollow_on_baseline`) is never rendered.
- Evidence:
  ```rust
  if i < 2 {
      style.text_color = chrome::WHITE;
      style.draw_outline_on_baseline(sample.text, pen, radius, &mut *fb).unwrap();
  ```
  `grep -rn "draw_hollow_on_baseline" crates tools firmware/src` finds only `chrome.rs` and
  `identity.rs:612`.
- Change: Call `draw_hollow_on_baseline` for the first two tiles.

## Structure

### Process-wide caches whose keys leave out an input
- Where: `crates/octowhere-ui/src/ui/clock_screen.rs:1043-1068`;
  `crates/octowhere-ui/src/ui/compass_screen.rs:349-377`; `crates/octowhere-ui/src/ui/identity.rs:581-635`
- Category: Structure
- Weight: medium
- What: Three statics hold per-screen work across calls: the clock's last two `Parts`, keyed by
  the `Face` tuple only; the compass's foreground boxes, built from whichever `font` arrives first;
  and the identity's title, also built from the first `font`. All three depend on the renderer,
  which is in none of the keys, so they are right only while every caller passes the same font
  set, and nothing says so. The crate runs several stages in one process (`tools/ui-sim --boards`,
  `octowhere-sim`'s `tests/screens.rs`), which share the two-entry clock cache and thrash it, and
  the title static is what makes the leak above possible.
- Evidence:
  ```rust
  static RECENT: Mutex<CriticalSectionRawMutex, core::cell::RefCell<heapless::Deque<(Face, Parts), 2>>> = ...;
  ...  .find(|(seen, _)| seen == face)
  ```
  ```rust
  fn foreground(font: &FontdueRenderer<'static, Color>) -> &'static [Rectangle; 9] {
      static FOREGROUND: embassy_sync::once_lock::OnceLock<[Rectangle; 9]> = ...;
      FOREGROUND.get_or_init(|| { ... caption_style(font, color) ... })
  ```
- Change: Keep the clock's recent parts beside `Stage::drawn`, which already holds the face before
  and after, and the title with the start-up state; the compass foreground could stay a `OnceLock`
  if its doc states the single-font assumption.

### `clock_screen::Parts::of` builds every part in one 146-line body
- Where: `crates/octowhere-ui/src/ui/clock_screen.rs:575-722`, `:548-573`
- Category: Structure
- Weight: low
- What: One function works out the time strings, the band lines, the gauge's length, exposure and
  frame, the token lines including the decision to split the zone onto two lines by width, the
  rail and the scatter. The split rule and the gauge's frame rule (`exposed == 0` or `NoData`
  keeps the slices still) are decisions a test could pin, but only reach tests through drawing.
- Evidence: `if one.width(font) > ZONE_LINE_MAX { for tokens in [ ... 1 ..., ... 2 ...] { ... } } else { ... }`
  (`:686-697`); `phase: match () { () if exposed == 0 => charging::OPEN, () if mode == Mode::NoData => charging::OPEN, () => accents.bands }` (`:624-628`).
- Change: Pull the token lines (`fn lines(mode, local, keys, accents, font)`) and the gauge
  (`fn gauge(...)`) out as plain functions, each with a test of its rule.

### `Stepper::draw` carries the replay chooser's look as branches
- Where: `crates/octowhere-ui/src/ui/second.rs:877-988`
- Category: Structure
- Weight: low
- What: The stepper is "the layout of round 3's timeout screen", but its draw branches on
  `replay` and `failure` for the slab colour, three font sizes, a `SUCCESSFUL BOOT` note, wrapping
  versus clamped neighbours, neighbour size, the `DEMO / nn OF nn` position and a numbered label.
  Timeout and always-on pass none of that; replay passes all of it through one enum.
- Evidence: `let replay = matches!(kind, StepperKind::Replay); let failure = replay && self.index > 0;`
  then `let size = if failure { 25 } else if replay { 36 } else { 43 };`, `let neighbours = if replay { [...] } else { [...] };`,
  `if failure { _ = write!(position, "DEMO / ...") } else if !replay { ... }`.
- Change: Have each chooser hand the stepper a small look (accent, size, note, wrap, position
  text) and keep `Stepper::draw` free of `StepperKind`.

## Duplication

### Two slide confirmations, both named `Slide`
- Where: `crates/octowhere-ui/src/ui/second.rs:1169-1282`; `crates/octowhere-ui/src/ui/power_off.rs:279-286`, `:421-461`;
  `crates/octowhere-ui/src/ui/slide.rs:12-80` (ui-runtime's file)
- Category: Duplication
- Weight: medium
- What: `second::Slide` with `Rail` (CLEAR SETTINGS, power-off) and `slide::Slide` with `Track`
  (leave, join, remove, decline) are both public, both follow a drag that starts in a grab area
  and both confirm near the end, with different commit rules (half the handle over the target
  against 90 %) and different release behaviour (snap back against ease back). `second::Slide`
  is only half parameterised: `handle` takes a `&Rail`, but `draw` reads `CLEAR_RAIL` directly, so
  power-off draws its own slider.
- Evidence: `second.rs:1171` `pub struct Slide`, `slide.rs:26` `pub struct Slide`;
  `second.rs:1211` `pub fn handle(&mut self, rail: &Rail, event: &GestureEvent) -> bool` but
  `second.rs:1246` `let top = CLEAR_RAIL.handle.top_left.y;`; `power_off.rs:422` `fn draw_slider`.
- Change: Move CLEAR and power-off onto `slide::Slide` (its commit point is a `Track` field away
  from either rule), or at least rename `second::Slide` and have its `draw` take the rail.

### The charging build is written twice, once for slices and once for bars
- Where: `crates/octowhere-ui/src/ui/charging.rs:310-358`, `:378-434`
- Category: Duplication
- Weight: medium
- What: `marks` (the gauge, `u32` columns) and `build_in` (the identity's barcode, `f32` units)
  each implement the seed, the end slices' flight from the middle and the inner slices popping on
  the logo frame nearest their place. The nearest-pop search is copied verbatim; the seed and the
  flight differ only in type and in how the end slices are clamped. A change to the build's
  timing has to be made in both.
- Evidence: both contain
  ```rust
  let nearest = POPS
      .iter()
      .min_by(|a, b| (a.0 - middle).abs().total_cmp(&(b.0 - middle).abs()))
      .map_or(0, |&(_, frame)| frame);
  if bands >= nearest + SEED_FROM - LOGO_SEED ...
  ```
  (`:348-352`, `:426-430`, with `place` for `middle`).
- Change: One generic build over `(left, width)` pairs in `f32`, with the gauge rounding its
  result to columns.

### Pager and Sheet share their settle machine and constants
- Where: `crates/octowhere-ui/src/ui/pager.rs:6-26`, `:149-158`; `crates/octowhere-ui/src/ui/sheet.rs:6-13`, `:301-313`, `:449-458`
- Category: Duplication
- Weight: low
- What: Both define `COMMIT_FRACTION = 4`, `FLICK_VELOCITY = 600.0`, a `Motion` enum with
  `Rest`, `Dragging` and `Settling { ease, offset }`, and the same `step`. Tuning one gesture's feel
  leaves the other behind.
- Evidence: `const COMMIT_FRACTION: i32 = 4; const FLICK_VELOCITY: f32 = 600.0;` in both;
  `let Motion::Settling { ease, .. } = self.motion else { return; }; ... ease.at(now) ...` in both.
- Change: Share the constants from `ease` and a `Settle { ease, offset }` with its `step`.

### The drag-stepped lists repeat their step and their tap regions
- Where: `crates/octowhere-ui/src/ui/second.rs:30`, `:38`, `:836-839`; `crates/octowhere-ui/src/ui/picker.rs:30-31`, `:335-338`
- Category: Duplication
- Weight: low
- What: `Stepper::stepped` and `Picker::stepped` are the same formula; the picker keeps its own
  `TOP_CAP = 150` and `BAND = 186..330` beside `second`'s `TOP_CAP = 150`, `in_top_cap` and
  `FIELD_TAPS = 186..330`.
- Evidence: `let steps = libm::truncf(-travel as f32 / per_step) as isize; (from as isize + steps).clamp(0, len as isize - 1) as usize`
  and `let steps = libm::truncf(-travel / STEP_TRAVEL) as isize; (from as isize + steps).clamp(0, self.len() as isize - 1) as usize`.
- Change: Make `Stepper::stepped` and `second::in_top_cap`/`FIELD_TAPS` `pub(super)` and use them
  from the picker.

### The style helper is written four ways
- Where: `crates/octowhere-ui/src/ui/text.rs:7-19`; `crates/octowhere-ui/src/ui/clock_screen.rs:769-780`;
  `crates/octowhere-ui/src/ui/identity.rs:289-296`; `crates/octowhere-ui/src/ui/startup.rs:617-624`
- Category: Duplication
- Weight: low
- What: `clock_screen::style` is a copy of `text::style`, body for body, and `always_on` uses the
  copy (`grep -rn "clock_screen::style" crates` gives 6 sites, all in `always_on.rs`).
  `identity::style` and `startup::small` are one-line wrappers around `text::style`; `small` is
  used for a 27 px title and a 100 px name.
- Evidence: `let mut style = font.clone(); style.text_color = color; style.font_size = size; style.font_index = index; style`
  in both `text.rs` and `clock_screen.rs`.
- Change: Delete the three and call `text::style`.

### One symbol, two definitions
- Where: `crates/octowhere-ui/src/ui/clock_screen.rs:116-117`; `crates/octowhere-ui/src/ui/startup.rs:79`, `:500`;
  `crates/octowhere-ui/src/ui/panel.rs:89`; `crates/octowhere-ui/src/ui/compass_screen.rs:640`; `crates/octowhere-ui/src/ui/icon.rs:1-6`
- Category: Duplication
- Weight: low
- What: The GNSS symbol, the clock symbol and the compass calibration symbol are each defined in
  two modules with the same meaning, while `icon.rs`, "the status symbols the screens share",
  holds only `NO_DATA`. (Other identical bit patterns, such as `ZONE`/`CODE`, mean different
  things and are left out.)
- Evidence: `clock_screen.rs:116` `const GNSS: Glyph = [0b00100, 0b01010, 0b10101, 0b01010, 0b00100];`
  and `startup.rs:500` the same; `clock_screen.rs:117` `const RTC: Glyph = [0b11111, 0b10001, 0b10101, 0b10001, 0b11111];`
  and `startup.rs:79` `Self::Clock => &[0b11111, 0b10001, 0b10101, 0b10001, 0b11111]`;
  `panel.rs:89` `CALIBRATING` and `compass_screen.rs:640` `OPEN_LOOP`, both `[0b01110, 0b10001, 0b10000, 0b10001, 0b01110]`.
- Change: Move the three to `icon.rs`.

### Zone and battery text formatted in two places each
- Where: `crates/octowhere-ui/src/ui/clock_screen.rs:75`, `:314-317`, `:322-326`; `crates/octowhere-ui/src/ui/picker.rs:285-291`;
  `crates/octowhere-ui/src/ui/panel.rs:266-269`; `crates/octowhere-ui/src/ui/always_on.rs:41`
- Category: Duplication
- Weight: low
- What: The `±hh:mm` offset is formatted in the clock face's plate and again in the picker's
  `signed`; `ZoneMode` maps to `AUTO`/`MANUAL` in the clock face and again in the panel; the low
  battery threshold `LOW: u8 = 15` is declared in the clock face and the always-on face.
- Evidence: `write!(offset, "{sign}{:02}:{:02}", minutes / 60, minutes % 60)` (clock) and
  `write!(text, "{sign}{:02}:{:02}", minutes / 60, minutes % 60)` (picker);
  `ZoneMode::Automatic => "AUTO", ZoneMode::Manual => "MANUAL"` in both; `const LOW: u8 = 15;` in both.
- Change: One `signed(offset)` and one `ZoneMode::tag()` in `ui::clock`, one battery threshold
  beside `screens::Battery`.

### The same integer hash twice
- Where: `crates/octowhere-ui/src/ui/scatter.rs:498-506`; `crates/octowhere-ui/src/ui/compass_screen.rs:404-410`
- Category: Duplication
- Weight: low
- What: `scatter::number` and `compass_screen::texture_hash` are the same xorshift-multiply mix
  with the same constants.
- Evidence: `grep -rn "0x7feb_352d" crates/octowhere-ui/src` gives the two lines; the steps around
  them (`>> 16`, `* 0x7feb_352d`, `>> 15`, `* 0x846c_a68b`, `>> 16`) match.
- Change: Keep one `pub(crate) fn hash(u32) -> u32` and build `number` on it.

### Two antialiased polygon fills and two segment distances
- Where: `crates/octowhere-ui/src/ui/smooth.rs:161-253`; `crates/octowhere-ui/src/ui/stroke.rs:57-88`, `:91-127`, `:302-312`
- Category: Duplication
- Weight: low
- What: `smooth::polygon_quarters` fills through fontdue's rasterizer and `stroke::fill_polygon`
  by an even-odd test and a distance per pixel; their edges will not match where both draw.
  Inside `stroke`, `Segment::distance` and `to_segment` compute the same distance, one with the
  direction precomputed.
- Evidence: `rasterize_path_clipped(raster, path, ...)` against
  `if (a.1 > y) != (b.1 > y) && x < ... { inside = !inside; } nearest = nearest.min(to_segment((x, y), a, b));`.
- Change: Decide which coverage the screens use and keep one fill (the quarter-turn reuse can
  wrap either); have `fill_polygon` build `Segment`s as `draw_path` does.

### The device page's POWER row in four arms
- Where: `crates/octowhere-ui/src/ui/second.rs:641-665`
- Category: Duplication
- Weight: low
- What: Four arms format the same voltage; two of them (`(true, true)` and `(false, true)`) are
  identical. The rule is one line: `CHG` while charging, else `USB` on USB, else `BAT`.
- Evidence: `(true, true) => text(format_args!("CHG  {}.{:02} V", ...))`, ...,
  `(false, true) => text(format_args!("CHG  {}.{:02} V", ...))`.
- Change: Pick the label in one `match`, format once.

### Ink placement and PNG writing re-derived
- Where: `crates/octowhere-ui/src/ui/always_on.rs:178-188`; `crates/octowhere-ui/src/ui/startup.rs:626-638`, `:1053-1066`;
  `crates/octowhere-ui/examples/outline.rs:113-128`; `crates/octowhere-ui/examples/render.rs:688-703`
- Category: Duplication
- Weight: low
- What: `text.rs` exists to place text by its ink, yet `always_on::centred` re-derives centring
  with integer halving (the `text` helpers round), and `startup` adds `centred` and `at_ink` on
  top of them. The two examples carry their own `write_png`.
- Evidence: `let left = CENTER.x - ink.size.width as i32 / 2 - ink.top_left.x;` (always-on) against
  `libm::roundf(x - ink.size.width as f32 / 2.0) as i32 - ink.top_left.x` (`text.rs:44`).
- Change: Add `text::draw_at_ink(style, text, (x, align), (y, align), target)` and use it; share
  `write_png` from the render example's module.

## Names and types

### A colour compared to recover the state that chose it
- Where: `crates/octowhere-ui/src/ui/clock_screen.rs:548-562`, `:626`; `crates/octowhere-ui/src/ui/panel.rs:449-457`
- Category: Names and types
- Weight: low
- What: `battery()` decides "a clock fault is not a battery fault" by testing whether the band
  is `RED`, while the same function's caller tests `mode == Mode::NoData` for the gauge's frame.
  The panel's row marker turns orange by testing whether the compass icon came out `ORANGE`. A
  change of a mode's colour silently changes these rules.
- Evidence: `() if band == chrome::RED => chrome::WHITE,`;
  `if cell == Cell::Compass && content.icon == chrome::ORANGE {`.
- Change: Pass the mode (or a `faulted: bool`) to `battery`, and give `Content` a
  `calibrating: bool`.

### Two public `Part` types, one imported beside the other
- Where: `crates/octowhere-ui/src/chrome.rs:638-644`; `crates/octowhere-ui/src/ui/startup.rs:35`;
  `crates/octowhere-ui/src/ui/identity.rs:26-31`, `:570`
- Category: Names and types
- Weight: low
- What: `chrome::Part` is a slice of a `Recording`; `startup::Part` is a self-test part. The
  identity, part of the start-up, imports `chrome::Part` and stores `heapless::Vec<Part, TITLE_PIECES>`
  next to `startup::{self, ...}`.
- Evidence: `pub struct Part { bytes: ..., columns: ..., rows: ... }` and `pub enum Part { Power, Clock, ... }`.
- Change: Rename the recording's to `Span` or `Take`.

### `hint_style` means 12 px in one module and 14 px in another
- Where: `crates/octowhere-ui/src/ui/panel.rs:363-365`; `crates/octowhere-ui/src/ui/second.rs:212-214`
- Category: Names and types
- Weight: low
- What: Both are public; `picker` uses `second::hint_style`. A caller choosing by name gets a
  different size depending on the import.
- Evidence: `style(font, chrome::GRAY, 12, FRAKTION)` and `style(font, chrome::GRAY, 14, FRAKTION)`.
- Change: Name them by role (`panel::hint_style` to `overview_hint_style`, or keep one public).

### The picker's fling uses 0 for "not yet advanced"
- Where: `crates/octowhere-ui/src/ui/picker.rs:75-78`, `:365`, `:473-480`
- Category: Names and types
- Weight: low
- What: `fling: Option<(f32, f32, Micros)>` is a tuple of travel, speed and last advance, and a
  last advance of 0 is a sentinel for the first step; `flung_from` lives outside it.
- Evidence: `self.fling = Some((travel, speed, 0));` then `if last == 0 { self.fling = Some((travel, speed, now)); return true; }`.
- Change: `struct Fling { from: usize, travel: f32, speed: f32, last: Option<Micros> }`.

### `clock_screen::Face` is a tuple; `draw` takes the same three as arguments
- Where: `crates/octowhere-ui/src/ui/clock_screen.rs:1038-1039`, `:840-846`, `:850`
- Category: Names and types
- Weight: low
- What: `damage` takes `&Face = (ClockView, Option<Battery>, Accents)`, `draw` takes the same
  values as three parameters and rebuilds the tuple to look up its parts.
- Evidence: `pub type Face = (ClockView, Option<Battery>, Accents);` and
  `let parts = parts(&(*view, supply, accents), font);`.
- Change: A `struct Face { view, supply, accents }` taken by both.

### `draw_button`'s `middle` is the far corner
- Where: `crates/octowhere-ui/src/ui/second.rs:318-322`
- Category: Names and types
- Weight: low
- What: `middle` is top-left plus the size, the exclusive bottom-right; the centre is then
  averaged from it. `draw_action_button` beside it uses `middle` for the real middle.
- Evidence: `let middle = bounds.top_left + Point::new(bounds.size.width as i32, bounds.size.height as i32);`
  then `(bounds.top_left.x + middle.x) as f32 / 2.0`.
- Change: Use `bounds.center()` (or name it `end`).

## Unstated invariants and magic numbers

### The compass field's entry steps are bare integers split over two files, with a dead arm
- Where: `crates/octowhere-ui/src/ui/compass_screen.rs:186-239`, `:516-517`; `crates/octowhere-ui/src/ui/stage.rs:2777-2789` (ui-runtime's file)
- Category: Unstated invariants and magic numbers
- Weight: medium
- What: The stage turns time into a step 0 to 5; the compass draws each step by literal: 2 draws
  nothing, 3 and 5 draw tiles (5 the settled field unless HOLD LEVEL), 1 and 4 draw blocks. Only
  "0 to 5, settled at 5" is written down. The match's `_ => (170, 0x2d49_1804)` arm is unreachable,
  since 0 never reaches `draw` (`Texture::of` returns `None`), and 2, 3 and 5 return earlier.
- Evidence: `if self.phase == 2 { return Ok(()); } ... if self.phase == 3 || self.phase == 5 { return self.draw_tiles(target); }`
  then `match self.phase { 1 => (42, 0x2d49_1801), 4 => (130, 0x2d49_1803), _ => (170, 0x2d49_1804) }`;
  `texture_step` returns only `0..=5`.
- Change: An enum of the steps (`Sparse`, `Gap`, `Tiles`, `Small`, `Settled`) produced by the stage
  and matched exhaustively by the compass; drop the dead arm and its seed.

### `set_pixel`'s unchecked write rests on every constructor forcing `CHECK_N`
- Where: `crates/octowhere-ui/src/framebuffer.rs:106-149`
- Category: Unstated invariants and magic numbers
- Weight: medium
- What: `set_pixel` writes with `get_unchecked_mut` and no `// SAFETY:` comment. It is sound only
  because `N == WIDTH * HEIGHT * BYTES_PER_PIXEL`, which is checked by a const assert that only
  `alloc` and `boxed` force. A third constructor that forgets `let _: () = Self::CHECK_N;` makes
  the write out of bounds without a compile error. The method also carries a stale
  `PERF: no panic for speed?`.
- Evidence:
  ```rust
  /// PERF: no panic for speed?
  #[inline]
  pub fn set_pixel(&mut self, x: usize, y: usize, color: C) {
      if x < WIDTH && y < HEIGHT {
          let idx = y * WIDTH + x;
          unsafe { self.buf.get_unchecked_mut(idx * C::BYTES_PER_PIXEL..) ...
  ```
- Change: Write the `// SAFETY:` line naming `CHECK_N`, and force the assert in an associated
  const used by `set_pixel` itself (or index checked, since nothing outside the module calls it).

### `FONTS` order and the index constants are kept in step by hand
- Where: `crates/octowhere-ui/src/chrome.rs:881-898`
- Category: Unstated invariants and magic numbers
- Weight: low
- What: Faces are picked by `usize` indices declared apart from the slice they index. Reordering
  or inserting a font silently re-maps every screen's face. Every caller passes `chrome::FONTS` as
  the renderer's `fonts` (`grep -rn -A5 "FontdueRenderer::new(" crates tools firmware/src` shows no
  other slice), so the parameter buys nothing.
- Evidence: `pub const FONTS: &[&dyn FontRepr] = &[&MarathonShapiroFont, &FraktionMonoRegularFont, ...];`
  then `pub const SHAPIRO: usize = 0; pub const FRAKTION: usize = 1; ...`.
- Change: Build `FONTS` from the constants (`const FONTS: [&dyn FontRepr; 7] = { let mut f = ...; f[SHAPIRO] = &...; ... }`)
  or use an enum, and drop the `fonts` parameter.

### Bounded collections whose bounds the types do not carry
- Where: `crates/octowhere-ui/src/ui/stroke.rs:42-46`; `crates/octowhere-ui/src/ui/dirty.rs:236-249`;
  `crates/octowhere-ui/src/ui/scatter.rs:47-52`, `:207-211`, `:240-242`
- Category: Unstated invariants and magic numbers
- Weight: low
- What: `draw_path` collects its segments into `heapless::Vec<Segment, 8>`, whose `FromIterator`
  panics past 8 (heapless 0.9.3 `src/vec/mod.rs:1459`, `expect("Vec::from_iter overflow")`), so a
  path of more than nine points panics on the frame loop's path; the comment says six. 
  `add_polygon` silently drops corners past eight and closes the polygon on the eighth. `Tones`
  keeps a mark's tone in two bits; a fifth colour would be drawn but compare equal to the first in
  `Shown`, so its damage would be missed.
- Evidence: `// A list's paths hold at most six points. let segments: heapless::Vec<Segment, 8> = points.windows(2)...collect();`;
  `let count = corners.len().min(edges.len());`; `/// At most four. pub colors: &'static [Color],` against
  `for place in 0..2 { if tone >> place & 1 != 0 { ... } }`.
- Change: `debug_assert!` each bound at its entry (`points.len() <= 9`, `corners.len() <= 8`,
  `colors.len() <= 4`), as `each_point` already does for `FIELDS`.

### The panel's geometry is restated as literals
- Where: `crates/octowhere-ui/src/ui/startup.rs:568`, `:1084-1095`, `:1149`, `:1235`, `:1254`, `:1270-1284`, `:1310`, `:1329`, `:1404-1408`;
  `CENTER` in six of this partition's modules; `startup.rs:1227` and `scatter.rs:107` (`GLASS`)
- Category: Unstated invariants and magic numbers
- Weight: low
- What: `startup.rs` uses the literal `466` on 17 lines of non-test code where `board::LCD_WIDTH`,
  `LCD_HEIGHT` or `chrome::DISPLAY_SIZE` exist; `CENTER: Point = Point::new(233, 233)` is declared
  in seven modules; the glass radius is declared twice.
- Evidence: `awk '/#\[cfg\(test\)\]/{exit} {print}' crates/octowhere-ui/src/ui/startup.rs | grep -c "\b466\b"` gives 17;
  `grep -rln "CENTER: Point = Point::new(233, 233)" crates/octowhere-ui/src` gives always_on, members,
  second, clock_screen, panel, compass_screen, startup.
- Change: One `board::CENTER` and `board::GLASS_RADIUS`, and the panel size from `board`.

## Comments

### The colour format reads as switchable but is fixed to RGB565
- Where: `crates/octowhere-ui/src/chrome.rs:18-20`, `:175-228`; `crates/octowhere-ui/src/framebuffer.rs:23-33`; `AGENTS.md` ("Rendering")
- Category: Comments
- Weight: low
- What: The comment beside `pub type Color = Rgb565` weighs Rgb888 and Gray8, and `AGENTS.md` says
  `chrome::Color` selects the format. But `impl CoverageTarget for FB` and `FB::pixel` decode
  two big-endian bytes as `Rgb565` and `lerp` them against `color: Color`, so any other `Color`
  fails to compile there. (The firmware's `co5300.rs` does use the `Rgb888` and `Gray8`
  `PixelFormat` impls for its own modes, so those stay.)
- Evidence: `// Rgb888 is higher quality, Rgb565 cuts the size of the framebuffer by a third. // Gray8 is 3x smaller than Rgb888... but I'm not sure we love monochrome.`;
  `let under = Rgb565::from(RawU16::new(u16::from_be_bytes([pixel[0], pixel[1]]))); under.lerp(&color, covered)`.
- Change: Say the UI's framebuffer is RGB565 only, in the comment and in `AGENTS.md`, or write the
  FB paths against `Color`'s raw type.

### Stale docs on `Scatter::IDENTITY` and `Scatter::draw`
- Where: `crates/octowhere-ui/src/ui/scatter.rs:110-127`
- Category: Comments
- Weight: low
- What: `IDENTITY` is documented as the identity's scatter and `draw` says "The identity runs at
  a density of 1.15", but the identity builds its own two-field scatter (`identity.rs:225-236`,
  densities 0.65 and 0.40). `IDENTITY` is used only by this module's tests.
- Evidence: `/// The identity's: the whole panel to radius 228, stopping short of the band on rows 198–317.`;
  `grep -rn "Scatter::IDENTITY" crates tools firmware/src` gives three lines, all in `scatter.rs`'s tests.
- Change: Move the constant into the test module as the reference fixture and drop the density
  sentence.

### Stale comments in the render example
- Where: `crates/octowhere-ui/examples/render.rs:212`, `:229`; `crates/octowhere-ui/examples/render/atlas.rs:64`
- Category: Comments
- Weight: low
- What: `clock-marking` is described as "the wordmark holding", and its atlas label is
  `ENTERING / MARK`; the wordmark was removed (`DECISIONS.md` 13), and no wordmark code is left.
  `compass-entering` says "the ring is in"; the ring went with `DECISIONS.md` 4b.
- Evidence: `// 380 ms in: the wordmark holding, its first block standing, before it snaps in.`;
  `// Partway through the entry fades: the ring is in, the icon arriving, the dial not yet.`;
  `grep -rni wordmark crates/octowhere-ui/src` finds nothing.
- Change: Say what those frames show now, or drop `clock-marking` if it duplicates `clock-entering`.

### Smaller stale or misplaced comments
- Where: `crates/octowhere-ui/src/ui/compass_screen.rs:1213-1215`; `crates/octowhere-ui/src/ui/pager.rs:21`;
  `crates/octowhere-ui/src/ui/picker.rs:73`; `crates/octowhere-ui/src/ui/smooth.rs:3-5`; `crates/octowhere-ui/src/framebuffer.rs:172`;
  `crates/octowhere-ui/src/ui/second.rs:345`; `crates/octowhere-ui/src/chrome.rs:1195-1199`, `:1075-1079`; `crates/octowhere-ui/src/ui/script.rs:22-23`
- Category: Comments
- Weight: low
- What: Each says something the code no longer does.
- Evidence:
  - `compass_screen.rs:1213`: `/// The slab's black knockout content, over a target that knows the slab's colour.`
    sits on `DASHES`, stacked over its real doc; it belongs to `draw_readout`.
  - `pager.rs:21`: `Settling` is "Moving toward `target`", but it has no `target` field.
  - `picker.rs:73`: `/// The index and travel when the current drag started.` on `grabbed: Option<usize>`.
  - `smooth.rs:3`: "Both primitives are symmetric about `center`"; the module has four, and `rect`
    has no centre.
  - `framebuffer.rs:172`: `buffer_mut` is "(snapshot restore)"; it serves the blend paths and the
    flush.
  - `second.rs:345`: "The former editor field, retained for the orange clear confirmation."
  - `chrome.rs:1195-1199`: commented-out `underline_color` and `strikethrough_color` fields.
  - `chrome.rs:1078`: `dilate` says the outermost pixels "must stay zero"; the hollow path feeds it
    an inverted border of 255 (`:1760`), which is right. The real precondition is that the border
    already holds its grown value.
  - `script.rs:22`: `SENSOR_PERIOD` is "How often the sensor task publishes"; the sensor task
    publishes every 250 ms (`firmware/src/main.rs`, `sensor_task`'s `Timer::after(Duration::from_millis(250))`).
- Change: Fix or drop each.

## Tests

### The giant-glyph test checks a looser bound than the raster reserved at boot
- Where: `crates/octowhere-ui/src/ui/startup.rs:1659-1669`; `crates/octowhere-ui/src/chrome.rs:1025-1029`, `:1046-1053`
- Category: Tests
- Weight: medium
- What: The fault screen's doubled name is the largest raster any screen draws, and the raster
  made at boot holds `RASTER_CELLS = 8_859` cells, never to grow (a growth panicked both boards on
  2026-10-03, per `AGENTS.md`). The test asserts `width * height * 4 < 40_000`, that is 10,000
  cells, so a glyph between 8,857 and 9,999 cells passes it and grows the raster on the board,
  where `chrome::fits` is a `debug_assert`. Its doc still describes "a raster of its own".
- Evidence: `assert!(metrics.width * metrics.height * 4 < 40_000, "{c}");` against
  `debug_assert!(width * height + 3 <= RASTER_CELLS, ...)`.
- Change: Assert `metrics.width * metrics.height + 3 <= chrome::RASTER_CELLS` for every name's
  glyphs at `NAME_PX / 2`, and fix the doc.

### The hollow text the identity draws is untested; the unused outline is tested
- Where: `crates/octowhere-ui/src/chrome.rs:1704-1780`; `crates/octowhere-ui/tests/outline.rs`
- Category: Tests
- Weight: low
- What: `draw_hollow_on_baseline` (the title's outline, `identity.rs:612`) runs the inverted
  dilation path; `tests/outline.rs` covers only `draw_outline_on_baseline`, which `AGENTS.md`
  says no screen uses. The example that should show the hollow ring draws outlines (Bugs).
- Evidence: `grep -rn "hollow_on_baseline" crates/octowhere-ui/tests` finds nothing.
- Change: A sibling test: the hollow ring stays inside the filled glyph's ink and covers its edge.

### `smooth` and `reveal` have no tests
- Where: `crates/octowhere-ui/src/ui/smooth.rs`; `crates/octowhere-ui/src/ui/reveal.rs`
- Category: Tests
- Weight: low
- What: `polygon_quarters` maps each quarter turn to rows and columns by hand, and draws the
  compass dial; `DiscRows::solid` decides what the clear may skip under the clock's band;
  `Reveal::of` and `Reveal::part` time every typed line. None has a direct test, and the stage's
  clipped-against-full comparisons cannot catch a misplaced quarter.
- Evidence: `grep -rln "polygon_quarters\|DiscRows\|Reveal::of" crates/*/tests` finds nothing, and
  neither module has a `#[cfg(test)]`.
- Change: Test that `polygon_quarters` with all four bits equals four separately turned fills, that
  `solid()` lies inside every row's solid span, and `Reveal::of`'s ends and block.

### The test renderer is built inline eight times
- Where: `crates/octowhere-ui/src/ui/clock_screen.rs:1213`, `:1291`; `compass_screen.rs:1315`; `text.rs:136`; `startup.rs:1698`
  (and three in ui-runtime's `drawer/messages.rs` and `group/keyboard.rs`)
- Category: Tests
- Weight: low
- What: Each test spells out `FontdueRenderer::new(FontdueRendererCtx::new_rc(), 20, chrome::WHITE, chrome::FONTS)`.
- Evidence: `grep -rn "FontdueRenderer::new(" crates/octowhere-ui/src | grep -v chrome.rs` gives nine,
  eight of them in tests.
- Change: `impl Default for FontdueRenderer` (or a `cfg(test)` helper) once the `fonts` parameter
  goes.

## Work and stack

### `PeripheralState::default` builds a whole `MeshView` to read its name
- Where: `crates/octowhere-ui/src/ui/screens.rs:112-127`; `firmware/src/main.rs:2023-2029`
- Category: Work and stack
- Weight: low
- What: The default name comes from `MeshView::default().name`, which builds the view (32
  `Option<MemberView>` and the rest, "kilobytes" per `AGENTS.md`) as a temporary. The firmware
  calls it in `async_main` at boot. `AGENTS.md` says the view is filled where it lives on the heap
  rather than built on the stack.
- Evidence: `name: super::group::view::MeshView::default().name,`; `MeshView::default` sets
  `name: Name::from_mac(&mac)` with `let mac = [0; MAC_LEN];` (`crates/octowhere-node/src/view.rs:278-284`).
- Change: `name: Name::from_mac(&[0; MAC_LEN])`.

### The framebuffer derives `Clone`
- Where: `crates/octowhere-ui/src/framebuffer.rs:59-68`
- Category: Work and stack
- Weight: low
- What: `Clone` on a 434,312-byte array type returns it by value; one call on the board would put
  it on core 0's 107 KB stack. Nothing clones a framebuffer, and no bound needs it.
- Evidence: `#[derive(Clone)] pub struct Framebuffer<...> { buf: [u8; N], ... }`;
  `grep -rn "fb[a-z_]*\.clone()\|panel\.clone()" tools/ui-sim/src/main.rs tools/ui-web/src/lib.rs crates/octowhere-ui/tests firmware/src`
  and `grep -n Clone firmware/src/util.rs firmware/src/drivers/*.rs` find nothing.
- Change: Drop the derive.

## Public surface

### The framebuffer's raw writers are public, and one panics on its own input
- Where: `crates/octowhere-ui/src/framebuffer.rs:129-165`
- Category: Public surface
- Weight: low
- What: `clear_color` has no caller anywhere; `set_pixel` and `fill_rect` are called only inside
  the module. `fill_rect` clips the right and bottom but not `x`, so `x > WIDTH` gives `start > end`
  and a slice panic, and it trusts `raw.len()` to be the pixel size.
- Evidence: `grep -rn "\.fill_rect(\|set_pixel(\|clear_color(" crates tools firmware/src host-tests | grep -v src/framebuffer.rs`
  finds nothing; `let x_end = (x + w).min(WIDTH); ... let bytes = &mut self.buf[start * raw.len()..end * raw.len()];`.
- Change: Make `set_pixel` and `fill_rect` private (they are reached through `DrawTarget`) and drop
  `clear_color`.

### Public items with no caller, or none outside tests
- Where: `crates/octowhere-ui/src/ui/panel.rs:623-630`; `crates/octowhere-ui/src/chrome.rs:1401-1445`, `:988-993`;
  `crates/octowhere-ui/src/ui/startup.rs:1150`; `crates/octowhere-ui/src/ui/geometry.rs:3-55`; `crates/octowhere-ui/src/ui/compass_screen.rs:100-104`
- Category: Public surface
- Weight: low
- What: `panel::text_damage` has no caller. `FontdueRenderer::stretched_bounds` has none, and
  `draw_stretched` is called once, at scale `1.0`, for the fault ticker. `geometry::FillRegion`
  and `clipped_fill_region` are public for `tests/geometry.rs` only. `chrome::lerp_u8` is used only
  in `chrome.rs`. `compass_screen::Shape` is public with public tuple variants for the private
  `compass_texture` module.
- Evidence: `grep -rn "text_damage" crates tools firmware/src` and `grep -rn "stretched_bounds" crates tools firmware/src`
  find only the definitions; `grep -rn "draw_stretched" crates tools firmware/src` finds
  `startup.rs:1150` `style.draw_stretched(&line, Point::new(pen, baseline), 1.0, &mut strip)?;`.
- Change: Delete `text_damage` and `stretched_bounds`; draw the ticker with `draw_on_baseline` once a
  render shows its pixels unchanged, and drop `draw_stretched` if nothing needs a stretch;
  `pub(crate)` the rest.

## Files read

Read in full: `AGENTS.md`, `context/UI-FIRMWARE-REVIEW.md`, `context/BACKLOG.md`,
`context/design/DECISIONS.md`, `context/WORKING-NOTES.md`; `crates/octowhere-ui/Cargo.toml`,
`src/lib.rs`, `src/board.rs`, `src/chrome.rs`, `src/framebuffer.rs`, and under `src/ui/`:
`mod.rs`, `always_on.rs`, `charging.rs`, `clock.rs`, `clock_screen.rs`, `compass_screen.rs`,
`dirty.rs`, `ease.rs`, `geometry.rs`, `gesture.rs`, `icon.rs`, `identity.rs`, `pager.rs`,
`panel.rs`, `picker.rs`, `power_off.rs`, `rest.rs`, `reveal.rs`, `scatter.rs`, `screens.rs`,
`script.rs`, `second.rs`, `sheet.rs`, `shift.rs`, `smooth.rs`, `startup.rs`, `stroke.rs`,
`text.rs`; tests `clock.rs`, `geometry.rs`, `knockout.rs`, `outline.rs`, `recording.rs`; examples
`outline.rs`, `render.rs`, `render/atlas.rs`.

Skimmed: `src/ui/compass_texture.rs` (generated; read its header and how `compass_screen` uses its
three tables); `context/SCREEN-DESIGN-BRIEF.md` (the settings and power-off sections only, for the
hint sizes); the parts of `src/ui/stage.rs`, `src/ui/slide.rs`, `crates/octowhere-node/src/view.rs`
and `firmware/src/main.rs` that the findings above cite, to confirm the callers and not to review them.
