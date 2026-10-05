# ui-runtime review, base b8ab7ca

Partition: `crates/octowhere-ui/src/ui/stage.rs`, `ui/group/` (all), `ui/drawer/` (all), `ui/events.rs`,
`ui/members.rs`, `ui/slide.rs`, the tests `stage.rs`, `group.rs`, `drawer.rs`, `members.rs`,
`messages.rs`, `removals.rs`, `common/mod.rs`, and the render examples `events.rs`, `group.rs`,
`members.rs`, `messages.rs`, `removals.rs`. Paths below are relative to the worktree
`/home/me/.claude/jobs/173b5f0c/tmp/review-b8ab7ca`; `ui/` means `crates/octowhere-ui/src/ui/`.

The four bugs were reproduced on the host. The reproducers live outside both checkouts, in a
scratch crate that depends on the worktree's `octowhere-ui` by path:
`/home/me/.claude/jobs/173b5f0c/tmp/ui-runtime-scratch/tests/verify.rs`. Rerun from that directory with
`CARGO_TARGET_DIR=/home/me/.claude/jobs/173b5f0c/tmp/review-target-ui-runtime cargo test --offline --test verify -- --nocapture`.

Counts: Bugs 4, Structure 3, Duplication 8, Names and types 5, Unstated invariants and magic
numbers 4, Comments 3, Tests 4, Work and stack 3, Public surface 2. Total 36.

## Bugs

### A short press of the power key on a dimming or darkening screen re-dims it instead of waking it
- Where: ui/stage.rs:1579-1588 (`wake_by_key`), ui/stage.rs:1176-1181 (the timeout check at the end of `advance`)
- Category: Bugs
- Weight: high
- What: `wake_by_key` sets `Rest::Awake` and starts the fade up, but never restarts the timeout timer. Later in the same step the timeout check finds `now - active_since` still past the timeout and sets `Rest::Dimmed` again with a fresh `since`. A dimming screen keeps dimming. A darkening screen jumps back to dim and holds there for another `DIM_HOLD`. Owner decision 12 says a short press wakes a resting screen, and the function's own doc says it wakes "as a contact would". A contact does restart the timer, at line 1173.
- Evidence:
  ```rust
  Rest::Dimmed { .. } | Rest::Darkening { .. } => {
      self.rest = Rest::Awake;
      self.fade_to(self.level, rest::WAKE_FADE, now);
  }
  ```
  `wake` (1817) and `wake_for_toast` (1225) both call `self.restart(now)`. This arm does not. Reproducer `a_short_press_on_a_dimmed_screen`: before the press `Dimmed { since: 15000300 }`, level 87. After it `Dimmed { since: 15216971 }`, and the next 1.5 s of levels run 86, 84 … 36. From darkening: `Darkening { since: 20000400 }` becomes `Dimmed { since: 20017067 }` at level 36. No test gives a short press to a dimmed or darkening screen. `a_short_press_rests_the_screen_at_once_and_another_wakes_it` covers Awake to Off and back only, and the dimmed cases in the power-off tests use the long press, whose confirmation restarts the timer itself (`step_power_off`, 1611).
- Change: Call `self.restart(now)` in the dim arm of `wake_by_key`. Add the case to the stage tests. The structural finding "The rest state machine is written in seven places" below is the underlying cause.

### A drawer left open stays on the panel when the timeout rests the screen on the always-on face
- Where: ui/stage.rs:1740-1751 (`step_rest`, Dimmed to AlwaysOn), ui/stage.rs:2690-2699 (`draw`), ui/stage.rs:1591-1606 (`sleep`)
- Category: Bugs
- Weight: high
- What: The timeout's move to `Rest::AlwaysOn` does not close the drawer. Only `sleep()`, the power key's path, calls `close_drawer()`. `draw()` checks for an open drawer before `draw_under` checks for the always-on face, so the drawer keeps being drawn at the always-on level. The always-on face never shows. That is the burn-in case the face exists to avoid.
- Evidence:
  ```rust
  if self.drawer.is_some()
      && self.drawer_sheet.is_open()
      && let Some(list) = &self.drawer_list
  {
      screens::clear(target).expect("clearing the panel failed");
      list.draw(&self.renderer, target)
  ```
  This comes before `self.draw_under(target)`, whose first branch is `if let (Rest::AlwaysOn, Some(view)) = (self.rest, &self.drawn_always_on)`. Reproducer `the_drawer_left_open_through_the_timeout` uses Seconds15 and AlwaysOn::Dim. At `Rest::AlwaysOn` the drawer is still held. The frame is identical to the last dimmed frame, 0 pixels differ. It differs from the always-on face drawn without the drawer by 14,848 pixels.
- Change: Make resting one function that both `sleep` and `step_rest` use, so the timeout also closes the drawer (keeping the draft) and clears the toast. Or check `Rest::AlwaysOn` first in `draw`. The first fixes the cause.

### Refresh events and the refresh strip assume session numbers only grow, and the node numbers them from 1 again after leaving or joining
- Where: ui/events.rs:319-337 (`Events::refresh`, line 326), ui/group/mod.rs:2445-2463 (`refresh_screen`, line 2457) and 534-544 (`RefreshEntry`'s `last + 1`); the other side is `crates/octowhere-node/src/node.rs:1048`, `:2152` and `:3099`, and `crates/octowhere-node/src/view.rs:122`; the scripted mesh copies it at ui/group/sim.rs:563 and :579
- Category: Bugs
- Weight: medium
- What: The view documents the refresh session as counting up "since boot". The UI relies on that in three places: `Events::refresh_session`, `group::Memory::refresh_seen`, and the guess that the next refresh is `last + 1`. The node breaks the promise. It sets `self.refresh = None` when the device leaves (1048) and when it joins or founds a group (2152), then numbers the next refresh from 1. After that, a new refresh makes no event, and its end is never told, unless an old event with the same number is still listed. In that case the new refresh silently takes over the old event. The strip also stops showing ended results "once", because the new numbers are not above `refresh_seen`.
- Evidence: view.rs:122 `/// Counts up from 1 with each refresh started since boot.` node.rs:3099 `session: self.refresh.map_or(1, |refresh| refresh.session + 1),`. events.rs:326 `None if refresh.is_listening() && refresh.session > self.refresh_session => {`. group/mod.rs:2457 `Some(refresh) if refresh.session > memory.refresh_seen => {`. Reproducer `a_refresh_numbered_again_from_one`: refresh 1 runs and ends (told), is dismissed, the view's refresh goes to `None` as a leave does, then a new refresh 1 starts. Output `second refresh: ongoing events 0, events 0` and `second refresh ended: toast None, events 0`.
- Change: Keep a refresh counter in the node that survives `self.refresh = None`, as the view's doc says, and make `group::sim` match (sim.rs:563 and :579 copy the reset). Add a scenario across a leave and a join. The node is outside all three partitions and is being edited in the main checkout, so this one needs routing.

### The refresh screen shows 00:00 until the mesh takes the request up
- Where: ui/group/mod.rs:2506-2538 (`refresh`)
- Category: Bugs
- Weight: low
- What: Before the mesh publishes the new session, the screen computes `until = now + REFRESH_US` and then calls `time_left(until, until as Micros, list)`, which counts down from `until` to itself and gives `00:00`. The comment beside it says "all its time is still to come". In the script driver this lasts one frame. On the board it lasts until the node publishes its next view.
- Evidence:
  ```rust
  // Asked for, and not yet taken up: all its time is still to come.
  None => (None, now as At + REFRESH_US),
  ...
  let left = if refresh.is_some() {
      time_left(until, now, list)
  } else {
      time_left(until, until as Micros, list)
  };
  ```
  Reproducer `the_refresh_screen_before_the_mesh_takes_it_up` records each step's group text after START REFRESH: `step 3: ["00:00"]`, then `step 4: ["02:15"]`.
- Change: Show the whole duration without scheduling a change, for example `words::countdown(REFRESH_US, 0).0`.

## Structure

### The rest state machine is written in seven places, each resetting a different subset of fields
- Where: ui/stage.rs:1723-1772 (`step_rest`), 1816-1836 (`wake`), 1579-1588 (`wake_by_key`), 1212-1228 (`wake_for_toast`), 1591-1606 (`sleep`), 1664-1689 (`rest_again`), 1176-1181 (timeout in `advance`)
- Category: Structure
- Weight: high
- What: Moving between Awake, Dimmed, Darkening, AlwaysOn and Off is spread over seven code paths. Each one touches its own subset of `rest`, `fade`, `level`, `active_since`, `swallowed`, `entry_from`, `drawn_always_on`, `shift`, `changed`, `route`, the toast and the drawer. The first two bugs above come straight from that: one path does not restart the timer, and one does not close the drawer.
- Evidence: Entering AlwaysOn is written twice. `sleep` (1591-1606) closes the drawer, clears the toast, sets `drawn_always_on = None` and sends the level. `step_rest` (1741-1745) does the last two only. Waking is written four times. `wake` restarts the timer, sets `entry_from`, `swallowed`, `shift.advance` and re-enters the face. `wake_for_toast` restarts the timer only, and starts its fade at `now` rather than after `PANEL_WAKE`. `wake_by_key` (dim arm) sets `rest` and the fade only. `step_rest`'s contact arm sets `swallowed` and relies on line 1173 to restart the timer.
- Change: One `enter(rest, now, update)` that owns every transition, with the per-target resets in one place, called from the key, the timeout, the toast and the double tap.

### stage.rs carries the faces' entry choreography alongside the input routing
- Where: ui/stage.rs:90-201 (about 30 timing constants and the `CompassSettled`/`ClockSettled`/`CompassTimes`/`ClockTimes` types), 2366-2411, 2415-2511, 2514-2666, 2777-2822
- Category: Structure
- Weight: medium
- What: `Stage` has 73 fields and stage.rs is 2,822 lines. About 450 of those lines are the clock's, the compass's and the panel's entry and exit timing, written as methods on `Stage` that both mutate settled state and compute accents. The faces' modules already own drawing and damage. Their timing could be plain types that step and return accents, each tested on its own.
- Evidence: `sed -n 325,456p ui/stage.rs | grep -cE "^    [a-z_]+: "` gives 73. `compass_accents` spans 2415-2511 and `advance_clock_accents` spans 2514-2666. Both mix inserting the settled state, tracking changes and computing the exit from `swipe_progress`.
- Change: Move each face's entry state and its `advance` into a `ClockEntry`, `CompassEntry` and `PanelEntry` beside the face, with `Stage` holding one of each. The faces' files are ui-faces'.

### Opening and closing the drawer, and leaving a group page, are each written out several times
- Where: ui/stage.rs:1277-1282, 1358-1364, 2181-2186 (open); 1061-1065, 1370-1377 (close); 1338-1343, 2349-2355 (interrupt and discard a group page); 1440-1441, 1444-1445, 1344-1345 (open a group page)
- Category: Structure
- Weight: low
- What: Handing the kept draft over, setting `self.drawer` and moving `drawer_sheet` are repeated at every entry point. The close path in `advance` duplicates `close_drawer` without its `make_full`. Interrupting a pairing and discarding the page is copied between `open_group` and `go_home`.
- Evidence:
  ```rust
  let mut drawer = Drawer::at_event(toast.event, &self.events);
  drawer.keep_draft(self.kept_draft.take());
  self.drawer = Some(drawer);
  self.drawer_sheet.go(true, now);
  ```
  This appears at 1279-1282 and again at 1361-1364. 2182-2185 is the same with `Drawer::new()` and `grab`.
- Change: `open_drawer(drawer, now)`, `close_drawer()`, `open_group(flow, now)` and `discard_page(effects)` helpers used everywhere.

## Duplication

### Time is formatted by six functions with two different APIs
- Where: ui/group/words.rs:34-41 (`countdown`) and 46-70 (`age`); ui/drawer/rows.rs:257-282 (`age`, `age_due`, `clock`); ui/drawer/removals.rs:120-130 (`left`); ui/group/mod.rs:2397-2403 (`time_left`), plus inline copies at 2158-2167 and 2085-2094
- Category: Duplication
- Weight: medium
- What: The group screens' formatters take `(since, now)` and return the text with when it next changes. The drawer's take an elapsed time, with `age_due` called separately. `removals::left` is a third MM:SS countdown, and `rows::clock` a fourth. `discovery` and the code screen reimplement `time_left` with a `--:--` fallback. Some formats differ by design (`05M 20S` against `05M`), but the six APIs do not need to.
- Evidence: `words::countdown(deadline, now) -> (String<5>, Option<Micros>)`, `rows::age(elapsed) -> Line` with `rows::age_due(elapsed) -> Micros`, `removals::left(at, now, unit) -> (Line, Micros)`. group/mod.rs:2158-2167 is the body of `time_left` plus a `None => "--:--"` arm.
- Change: One module of formatters, each returning `(text, next)` and taking `(at, now)`, with the design's two age styles as two functions. `time_left` gains the `--:--` case.

### Fitting text to a width is written six ways
- Where: ui/drawer/parts.rs:96-115 (`title`), ui/members.rs:828-845 (`fit`), ui/drawer/removals.rs:290-299 (`target`), ui/group/mod.rs:1273-1276 (`big`), 955-959 (own device), 1490-1501 (`member_detail`); wrappers ui/drawer/rows.rs:247-253 (`fit`), ui/drawer/removals.rs:71-88 (`measure`, `fitted`), inline at ui/group/mod.rs:1491 and 1584
- Category: Duplication
- Weight: medium
- What: `parts::title` and `members::fit` are the same algorithm: the largest size in a range whose ink fits a width. The others pick between two sizes by three different rules. `big` and the own-device name compare byte length with 12, `member_detail` measures advance against 270, and `target` measures against `CENTRED`. `rows::fit` and `removals::fitted` are the same wrapper of `parts::fitted`.
- Evidence: group/mod.rs:1274 `let size = if text.len() < 12 { 35 } else { 28 };`. group/mod.rs:955-959 repeats it for the name: `if mesh.name.as_bytes().len() < 12 { 35 } else { 28 }`. members.rs:835-844 and parts.rs:99-106 both step a size down until `baseline_bounds(..).size.width <= width`.
- Change: One `largest_fitting(font, face, sizes, width, text)` in `parts`, and one `fitted(font, face, size, width, text)`. A name chooses its size by measuring, not by byte count.

### The toast overlay, the drawer, the member face and the group screens each double-buffer a `List` by hand
- Where: ui/stage.rs:351-353, 429-432, 437-442 (fields); 1142, 2053 (group); 1381-1401, 2090 (drawer); 1405-1423, 1158-1161, 2091 (members); 1453-1508 (overlay); 311-323 (`Drawn`)
- Category: Duplication
- Weight: medium
- What: Four pairs of `Option<Box<List>>` are taken, filled, swapped and dropped by hand. Each pair has its own ownership: the group's list lives inside `Drawn::Group`, while the drawer's and members' live beside the unit markers `Drawn::Drawer` and `Drawn::Members`. Each pair also keeps its spare by its own rule. The cost is in "Work and stack" below.
- Evidence: `members_spare` is dropped while the face is hidden ("Lists are kilobytes; a hidden face keeps none", 1158-1161). `spare_list` is never cleared after a group page closes (`grep -n spare_list ui/stage.rs` shows only `take` at 1142 and `Some(before)` at 2053). `overlay` and `overlay_spare` are both refilled on every step that reaches `track_overlay`.
- Change: One `Lists { shown, spare }` type with `build(|list| ..) -> before` and one rule for releasing the spare, used by all four.

### Three settle-and-ease implementations, two storing a tuple and one storing the `Ease`
- Where: ui/stage.rs:271-301 (`Grid`), ui/group/mod.rs:113-214 (`Scroll`), ui/drawer/mod.rs:161, 453-468, 707-713 (`Drawer::settle`)
- Category: Duplication
- Weight: low
- What: `Grid::snap` and `group::Scroll::settle` both keep `Option<(i32, i32, Micros)>` and rebuild `Ease::new(from as f32, to, start)` on every step. The drawer keeps `Option<Ease>` directly. Grab, follow, clamp and release are written once for each, and once more in the drawer's child scroll (drawer/mod.rs:501-522).
- Evidence: stage.rs:295 `let (scroll, arrived) = Ease::new(from as f32, to, start).at(now);`, group/mod.rs:200 `let (offset, arrived) = Ease::new(from as f32, to, start).at(now);`, drawer/mod.rs:707-708 `if let Some(ease) = self.settle { let (slide, arrived) = ease.at(now);`.
- Change: Store `Option<Ease>` everywhere, and share one scroll type with a snap rule for rows (group) or none (drawer).

### The refresh's length is stated five ways
- Where: ui/group/sim.rs:19, ui/group/mod.rs:77, ui/events.rs:523, crates/octowhere-ui/examples/render/events.rs:18, crates/octowhere-ui/tests/drawer.rs:188, 205, 229
- Category: Duplication
- Weight: low
- What: The screens take it from `SWEEP_US` in two places. The scripted mesh hard-codes 135 s in a `pub const REFRESH` that nothing outside sim.rs reads, and the example and the tests write `135 * SECOND` again.
- Evidence: `grep -rn "135 \* SECOND\|SWEEP_US\|REFRESH_US" crates/octowhere-ui/{src,tests,examples}` gives sim.rs:19 `pub const REFRESH: Micros = 135 * SECOND;`, group/mod.rs:77 `const REFRESH_US: i64 = octowhere_mesh::clock::SWEEP_US;`, events.rs:523, examples/render/events.rs:18, and tests/drawer.rs three times.
- Change: `sim::REFRESH = SWEEP_US as Micros`, used by the tests and the example. `events::elapsed` and `group` share one constant.

### `upper` is written twice and a literal-to-`Line` helper three times
- Where: ui/drawer/messages.rs:154-162, ui/drawer/removals.rs:92-100, ui/group/layout.rs:440-448 and 453-464, ui/members.rs:950-952
- Category: Duplication
- Weight: low
- What: `messages::upper` (private) and `removals::upper` (pub) are identical. `layout::line`, `members::line` and `Cut::write_str` each push characters until the `Line` is full. Constant strings become a `Line` through `format(format_args!("NO RADIO"))` across the group and drawer files.
- Evidence: both `upper` bodies are `for c in text.chars() { if line.push(c.to_ascii_uppercase()).is_err() { break; } }`.
- Change: Make `layout::line` and an `upper` public in `layout`, delete the copies, and use `line("NO RADIO")` for literals.

### Two `Slide` types confirm the same gesture with different rules and argument orders
- Where: ui/slide.rs:25-94, ui/second.rs:1169-1240 (ui-faces' file), ui/group/mod.rs:216-253 (`Slider`), ui/drawer/removals.rs:42-45
- Category: Duplication
- Weight: low
- What: `ui::slide::Slide` commits at 90 % of the travel and is called as `handle(event, now, &track)`. `ui::second::Slide`, used by CLEAR and the power-off, commits when the handle's middle reaches the target and is called as `handle(&rail, event)`. Both are named `Slide`. The group screens wrap the first again as `Slider` with a fixed `TRACK`, and the removal screens pass `&removals::SLIDE` at each call. A wrong track passed in still compiles.
- Evidence: slide.rs:21 `(self.travel * 9 + 9) / 10` against second.rs:1222-1223 `let half = rail.handle.size.width as i32 / 2; return drag.offset().x.clamp(0, rail.travel) + half >= rail.travel;`.
- Change: Rename one, or give `Track` a commit rule and have both screens use `slide::Slide`. Hold the track in the slide, as `Slider` does, rather than passing it per call. second.rs is ui-faces'.

### The drawer's `Context` is built twice in the stage, and list text is read out three times
- Where: ui/stage.rs:1387-1398 and 1476-1487 (`drawer::Context`), 775-810 (`group_text`, `drawer_text`, `members_text`)
- Category: Duplication
- Weight: low
- What: The same seven-field `drawer::Context` literal appears in `build_drawer` and `track_overlay`. A `&self` method would compile at both sites, as `members_context` does for the member face. The three `*_text` accessors repeat one `filter_map` over `Shape::Text`.
- Evidence: both literals are `drawer::Context { events: &self.events, gnss: &self.peripherals.gnss, mesh: &self.mesh, messages: self.messages.as_deref(), now, breath: self.breath, font: &self.renderer }`.
- Change: `fn drawer_context(&self, now) -> drawer::Context<'_>`, and `List::texts()` used by the three accessors.

## Names and types

### This device's id falls back to 0 where an `Option` belongs, at 14 sites
- Where: ui/drawer/mod.rs:191, 553, 638, 668, 714; ui/drawer/messages.rs:59; ui/stage.rs:953; ui/group/mod.rs:988; ui/group/sim.rs:311, 375, 591, 620, 646, 705
- Category: Names and types
- Weight: medium
- What: 0 is a real member id ("YOU BECOME MEMBER 00"), so with no group every message from member 0 would read as this device's. The node forgets messages on leaving, so this does not show today. The stage passes the id as `Option<u8>` to `events.removals` at line 961 and as a 0-sentinel to `events.messages` at line 953, eight lines apart. `Mail::own()` exists, yet drawer/mod.rs derives it again four times.
- Evidence: `grep -rn "map_or(0, |group| group.own)" crates/octowhere-ui/src | wc -l` gives 14.
- Change: Pass `Option<u8>` and let `MessageView::thread`'s callers decide. Use `Mail::own` and `Context::own` instead of re-deriving.

### `action()` takes a positional bool and a parameter every caller passes as `None`
- Where: ui/group/mod.rs:1307-1336, and its 15 call sites
- Category: Names and types
- Weight: low
- What: `sub: Option<&str>` is `None` at all 15 calls, so the branch that sets a label and its note "as one block" (1323-1328) is dead. `primary` is `true` at 3 calls. `color` is `chrome::WHITE` at 14 of 15.
- Evidence: `grep -n "action(" ui/group/mod.rs | grep -v "fn action\|status.action\|Some(\""` lists 15 calls, all ending in `None`. 9 of them are exactly `false, ACTION, chrome::WHITE, None`.
- Change: Drop `sub` and its branch. Split into `button(list, label, area)` and `primary(list, label, area)`, with the orange REMOVE as its own call.

### Tuples indexed by number stand in for structs in the stage's damage tracking
- Where: ui/stage.rs:1008-1014 and 1129-1139 (`previous`, `current`), 314 and 2096-2123 (`Drawn::Panel`)
- Category: Names and types
- Weight: low
- What: The stage compares two 5-tuples of view, sheet offset, grid scroll, page and drawer offset, and picks fields out by position to tell "only the grid moved". `panel_damage` reads `before.0`, `after.1` and `after.2` for the readings, the scroll and the accents.
- Evidence: 1136-1138 `let grid_only = current.2 != previous.2 && (current.0, current.1, current.3, current.4) == (previous.0, previous.1, previous.3, previous.4);`.
- Change: A small `Motion { view, sheet, grid, page, drawer }` struct with a `grid_only(&self, before)` method, and named fields in `Drawn::Panel`.

### `removals::CENTRED` is a width beside a `centred()` function and `parts::CENTRE`, a position
- Where: ui/drawer/removals.rs:55-57, 270-272
- Category: Names and types
- Weight: low
- What: `const CENTRED: f32 = 320.0` is "how wide a sentence may run, centred". The same file defines `fn centred(list, text, top, ..)` and uses `parts::CENTRE`, the panel's middle column.
- Evidence: `const CENTRED: f32 = 320.0;` / `const COLUMN: f32 = 290.0;` and `fn centred(list: &mut List, text: &str, top: i32, ...)`.
- Change: `CENTRED_WIDTH` and `COLUMN_WIDTH`.

### `heading_moved` reads as a query but sets the anchor
- Where: ui/stage.rs:1704-1719
- Category: Names and types
- Weight: low
- What: The predicate stores `heading_anchor` when it first sees a heading. Callers combine it with `||` at 1173, where the side effect is easy to miss.
- Evidence: `(None, Some(_)) => { self.heading_anchor = heading; false }`.
- Change: Set the anchor in `restart` or at the reading's arrival, and keep `heading_moved(&self)` pure.

## Unstated invariants and magic numbers

### A step carries one mesh request, and later writers overwrite earlier ones silently
- Where: ui/stage.rs:236-237 (`Update::mesh`), 1048-1050, 1539, 2306-2308, 2351
- Category: Unstated invariants and magic numbers
- Weight: low
- What: `apply` replaces `update.mesh` with the screens' request. `press` and `go_home` write theirs through `interrupt`. The drawer's `Read` goes out only if nothing else did, yet the stage marks the message read locally either way, and the comment says "the mesh's next view says the same". No path loses a request today that I could construct, but nothing states the one-per-step rule or checks it.
- Evidence: 2306-2308 `if effects.mesh.is_some() { update.mesh = effects.mesh; }`. 1048-1050 `if update.mesh.is_none() { update.mesh = Some(Request::Read(id)); }`.
- Change: Make `Update::mesh` a small queue, or `debug_assert!` that it is empty before each write, and say which wins.

### The drawer's viewport, the title's limits and the panel's centre are restated as literals
- Where: ui/drawer/mod.rs:40-41; ui/drawer/rows.rs:30; ui/drawer/parts.rs:13, 18-19; ui/drawer/messages.rs:45, 214, 591; ui/group/mod.rs:47; ui/group/keyboard.rs:99; ui/members.rs:36-38
- Category: Unstated invariants and magic numbers
- Weight: low
- What: `VIEWPORT_HEIGHT = 287` and rows.rs `MIDDLE = 265.5` repeat `VIEWPORT = rect(0, 122, WIDTH, 409)`. messages.rs repeats parts.rs's `TITLE_WIDTH = 274` and writes `TITLE_SMALLEST`'s 26 as a literal twice. The panel's centre is a constant five times (`CENTRE` in group, keyboard and parts, `CX`, `CY` and `CENTER` in members), and members.rs writes 233 inline 13 more times.
- Evidence: drawer/mod.rs:40-41 `const VIEWPORT: Rectangle = rect(0, 122, WIDTH, 409); const VIEWPORT_HEIGHT: i32 = 287;`. messages.rs:214 `style(font, chrome::WHITE, 26, Face::Title.index())`. `grep -c "\b233\b" ui/members.rs` gives 17, 4 of them in constants.
- Change: Derive the height and middle from `VIEWPORT`. Export the title's limits from `parts`. Use one centre constant from `board`.

### Counts that the view names are written as numbers
- Where: ui/group/mod.rs:1915; ui/events.rs:347; ui/group/keyboard.rs:467; ui/drawer/rows.rs:463-465; ui/stage.rs:2783-2787, 2803-2805, 2819-2821
- Category: Unstated invariants and magic numbers
- Weight: low
- What: "ALL 32 IDS ARE TAKEN" hard-codes what the Full screen two arms away prints from `view::IDS`. `Vec<Thread, 33>` repeats messages.rs's `CONVERSATIONS`. The message field's caret rows are 22 px apart, against lines drawn 19 px apart at `MESSAGE_TOPS = [70, 89]`. The refresh rail's fill uses `236 *`, the width of the rect drawn the line before. The compass texture's thresholds and the icons' 5 rows are bare.
- Evidence: group/mod.rs:1915 `copy: ["ALL 32 IDS ARE TAKEN", ...]` against 1125 `format_args!("{count:02} / {} MEMBERS", view::IDS)`. keyboard.rs:467 `((point.y - MESSAGE_FIELD.top_left.y) / 22).clamp(0, 1)`. rows.rs:463-465 `rect(118, top + 113, 354, top + 117)` then `(236 * done / whole.max(1))`.
- Change: Format the count from `IDS`, use `CONVERSATIONS`, name the caret pitch, and derive the rail width from its rect.

### Each detail screen places its buttons twice, once to draw and once to hit-test
- Where: ui/drawer/rows.rs:613-620 against 714-719 and 794-799; ui/drawer/removals.rs:258-268 against 330-563; ui/group/mod.rs `Flow::handle` (448-755) against `Flow::view` (851-1159)
- Category: Unstated invariants and magic numbers
- Weight: low
- What: `detail_buttons` says where DISMISS and VIEW MEMBERS are, and `detail` draws them at the same rectangles by repeating the match. The removal screens and the group screens do the same. Every pair agrees today. Nothing ties them together, so a moved button keeps working only if both sides are edited.
- Evidence: rows.rs:616 `Kind::Gnss(_) if event.protected().is_some() => (None, Some(FOOTER_HIGH))` against 714-715 `if event.protected().is_some() { parts::button(list, FOOTER_HIGH, "DISMISS", false);`.
- Change: Have `detail` draw from `detail_buttons`' result, as the removal detail could from `buttons`.

## Comments

### `Drawn`'s doc comment is attached to `new_list`
- Where: ui/stage.rs:303-311
- Category: Comments
- Weight: low
- What: The two lines describing `Drawn` sit above `new_list`, so rustdoc gives `new_list` three sentences about two things, and `Drawn` none.
- Evidence:
  ```rust
  /// What the settled screen showed after a step, for the next step's damage. The screens settle
  /// under exclusive conditions, so at most one has a snapshot.
  /// An empty list on the heap, built outside the step so its frame never holds one.
  #[inline(never)]
  fn new_list() -> alloc::boxed::Box<List> {
  ```
- Change: Move the first two lines down to `enum Drawn`.

### `is_animating` and `is_changing` describe something other than what they compute
- Where: ui/stage.rs:875-893
- Category: Comments
- Weight: low
- What: `is_changing` says "As `is_animating`, but for the charging gauge". It excludes the gauge and the breath, and the frame loop uses it as "step at once" (firmware/src/main.rs:2663). `is_animating` says "a page slide or a fade", but it is true for the gauge and the breath, which the frame loop steps once a panel frame (main.rs:2667).
- Evidence: `self.is_changing() || self.gauge_moving || self.breathing` and the frame loop's `if stage.is_changing() { Duration::from_micros(0) } ... else if stage.is_animating() { PANEL_FRAME }`.
- Change: Document them by what the caller does: wants a step at once, wants a step each panel frame.

### Stale parameter references in the render examples' docs
- Where: crates/octowhere-ui/examples/render/events.rs:20, 39; crates/octowhere-ui/examples/render/removals.rs:122
- Category: Comments
- Weight: low
- What: `start` is documented "at `now`" and takes no `now`. `health` is documented "with what it last did `ago` before now", but the ages are fixed at 180 s and 300 s. `switched` mentions "declinable until `until`" and takes a `decline` closure.
- Evidence: `/// The receiver's health, as the GNSS task reports it, with what it last did `ago` before now.` above `fn health(driver: &mut Driver, recovering: bool, failed_resets: u8, fix: bool)`.
- Change: Rewrite the three lines to the signatures.

## Tests

### The 2026-10-04 screens' damage tests never check what the flush sends
- Where: crates/octowhere-ui/tests/drawer.rs:216-250, members.rs:306-349, messages.rs:257-283, removals.rs:462-486, group.rs:334-375 (its own `Buffers`); compare tests/stage.rs:458, 663, 1358, 1482, 1749, 2148
- Category: Tests
- Weight: medium
- What: The stage tests check `buffers.panel`, the copy of each step's flush, against a full draw. That proves `changed` alone covers what differs from the frame before. The drawer, member, messages and removal tests check only the framebuffer, which repaints the last step's damage as well and so tolerates a region missing from `changed`. group.rs reimplements `Buffers` without the panel copy. The four observers also count `steps` without asserting a minimum, so a script that stopped stepping would pass; group.rs asserts `steps > 500`.
- Evidence: `grep -n "\.panel\b" crates/octowhere-ui/tests/*.rs` matches only tests/stage.rs.
- Change: Put one `check_damage(stage, &mut Buffers)` in `common` that checks both the buffer and the panel, use it in all five files, delete group.rs's `Buffers`, and assert a step count.

### Each test file redefines the same driver helpers
- Where: crates/octowhere-ui/tests/{group,drawer,messages,removals,members,stage}.rs; crates/octowhere-ui/examples/render/{group,removals}.rs
- Category: Tests
- Weight: low
- What: `tap` is defined 5 times and `child` 3 times. A stage is built with `Stage::new(PeripheralState { timeout, .. })` at 8 sites. The swipe that pulls the panel down and pages it to the group screen's row is written out in tests/group.rs twice (`open_group`, `naming`), tests/removals.rs (`ridge`), examples/render/group.rs twice (`panel`, `awake_hub`) and examples/render/removals.rs (`ridge`). `common/mod.rs` holds only `Buffers` and `differing`.
- Evidence: `grep -n "^fn tap\|^fn child\|driver.stage = Stage::new" crates/octowhere-ui/tests/*.rs`, and `grep -c "swipe(Point::new(233, 80), Point::new(233, 420), 250_000)"` over the tests and examples.
- Change: Move `start(timeout, always_on)`, `tap`, `child`, `open_panel`, `open_group` and `open_drawer` into `common`.

### Fixtures and geometry are copied between tests, examples and the library
- Where: crates/octowhere-ui/tests/members.rs:24, 34-47; crates/octowhere-ui/examples/render/members.rs:20, 35-48; ui/members.rs:958-974; ui/group/sim.rs:136; crates/octowhere-ui/examples/render/group.rs:168-189; crates/octowhere-ui/tests/group.rs:290-297; ui/group/keyboard.rs:203-242
- Category: Tests
- Weight: low
- What: The great-circle `at`/`destination` helper is written three times and `DUBLIN` four times. examples/render/group.rs re-derives the keyboard's key grid in `letter` and `key` (38, 39, 58, 77, 186, 55), and tests/group.rs types a name by raw coordinates, because `keyboard::taps` only knows the message keyboard (`keys(mode, Field::Message)`).
- Evidence: examples/render/group.rs:184 `Point::new(38 + 39 * column + 19, 186 + 55 * row)`. tests/group.rs:295-297 `tap(&mut driver, 77, 241); tap(&mut driver, 57, 296); tap(&mut driver, 272, 296);`.
- Change: Export one `destination` and `DUBLIN` from `group::sim`. Let `taps` take the field, and use it in the example and the tests.

### Pure helpers with boundary arithmetic have no direct test
- Where: ui/drawer/rows.rs:257-282 (`age`, `age_due`, `clock`), ui/drawer/removals.rs:120-130 (`left`), ui/group/layout.rs:452-468 (`format`)
- Category: Tests
- Weight: low
- What: `words::age` and `words::countdown` have unit tests at each unit boundary. The drawer's `age`/`age_due` and `removals::left` do the same kind of rounding with no direct test. Only a few screen texts reach them, such as "24:00 LEFT" and "POSITION 11S OLD". `format`'s cut at `LINE` is untested.
- Evidence: rows.rs and removals.rs have no `#[cfg(test)]` module (`grep -c "cfg(test)" ui/drawer/rows.rs ui/drawer/removals.rs` gives 0 and 0).
- Change: Unit tests at 59/60 s, 3,599/3,600 s and the day boundary, and for `left`'s round-up and step.

## Work and stack

### 22.6 KB of internal heap is held for the whole run by the toast overlay's two lists
- Where: ui/stage.rs:1453-1508 (`track_overlay`), 1142 and 2053 (`spare_list`), 1986-2092 (`track_damage`)
- Category: Work and stack
- Weight: medium
- What: From the second step on, `overlay` and `overlay_spare` both hold a 128-item `List`, on every face, the panel and the always-on face, though the overlay draws at most one toast's dozen items. After the first group screen, `spare_list` keeps another list for good. While the drawer is open over a group page (a toast tapped there), the stage builds the group screen's list every step and drops it in `track_damage`, so the next step allocates a fresh one. AGENTS.md ("Memory") says the group screens hold two 8,848-byte lists while open. That misses the overlay's two and the one kept after closing, and the size is out of date.
- Evidence: `List` is 11,312 bytes on wasm32, a 32-bit target laid out like the board's, and 13,360 on x86-64 (the reproducer crate's `list_size` for x86-64; for wasm32, `const LIST: [(); 0] = [(); size_of::<List>()];` built with `--target wasm32-unknown-unknown`, whose error names the size). Two of them are 22,624 bytes, and the kept group spare another 11,312. `track_overlay` ends `self.overlay = Some((list, arc)); self.overlay_spare = before;` on every step that reaches it. In `track_damage`, `group_list` is stored only in the `Some(Drawn::Group(list))` arm (2018-2019) and dropped otherwise.
- Change: Give the overlay a small list type sized for a toast, or release the spare when no toast shows. Clear `spare_list` when the page closes. Skip building the group list while the drawer covers it. This touches the BACKLOG entries "Let the UI crate take an allocator for its large buffers" and "Lay out the 512 KiB of SRAM deliberately" (heap peak).

### An open conversation is wrapped about four times per step
- Where: ui/drawer/mod.rs:689-703 (`Drawer::step`), 755-760 (`Drawer::read`); ui/drawer/messages.rs:350-358, 370-389, 423-450
- Category: Work and stack
- Weight: low
- What: `thread_rows` wraps every message's body with the font each time it is called. One step with a conversation open calls it in `message_top`, `thread_height` and `thread_anchor` (step), in `shown_unread` (read), and twice in `conversation` (rows and the scroll arc's `thread_height`). That is four full wraps and two partial. `Drawer::step` also walks the whole store for the inbox's height on every step, whichever screen shows.
- Evidence: messages.rs:442 `for (message, top, height) in thread_rows(thread, mail)` and 449 `parts::scroll_arc(list, thread_height(thread, mail), VIEWPORT_HEIGHT, scroll)`. drawer/mod.rs:697 and 703.
- Change: Wrap once per step into a row table the step, read and view share. The BACKLOG entry "Every step rebuilds an open conversation's rows ... measure it with a full store on a board" covers the per-step rebuild. This finding is the 4× within one step.

### The declination is recomputed from the World Magnetic Model at every sensor sample
- Where: ui/stage.rs:940; ui/members.rs:125-134; crates/octowhere-motion/src/declination.rs:133-134
- Category: Work and stack
- Weight: low
- What: Each 250 ms sensor snapshot runs the degree-12 model on the frame loop's path, two 13 × 13 `f32` arrays (1,352 bytes) on its stack, whether or not the member face shows. The result depends only on the last fix's position and the year.
- Evidence: `self.declination = members::declination(&sensors.gnss, &self.peripherals.clock.clock());` inside `if let Some(sensors) = sensors`.
- Change: Recompute only when the fix moves (as `zone_task` does for zones) or the face is about to show.

## Public surface

### Dead or file-local public items
- Where: ui/events.rs:129-137 (`version`); ui/drawer/mod.rs:293-297 (`Drawer::typing`), 273-281 (`at_thread`); ui/group/keyboard.rs:305-322, 334-337; ui/members.rs:60, 163; ui/drawer/messages.rs:41, 98-102, 176-182, 520-531; ui/drawer/parts.rs:38; ui/group/layout.rs:231; ui/group/sim.rs:19, 21, 114-117
- Category: Public surface
- Weight: low
- What: Nothing anywhere reads `Events::version`, though eight methods increment it. `Drawer::typing` is never called, and its doc ("which a toast keeps clear of") describes a toast the stage never shows over the drawer. `Keyboard::{original, len, is_empty}` are unused. These are pub but used only in their own file: `Keyboard::is_saving`, `members::{RECENT, bearing_distance}`, `messages::{REVIEW_VIEWPORT, Conversation, destinations}`, `Mail::is_removed`, `parts::PROSE_SIZE`, `layout::GLYPH`, `Drawer::at_thread` and `sim::{REFRESH, RECOVERY, DECLINE_FOR, PEER_MAC, OWN_MAC}`.
- Evidence: `grep -rn "\.version()\|\.typing()" --include=*.rs crates tools firmware` finds no `version()` call and only the group flow's `typing()`. For the rest, `grep -rln <name> --include=*.rs crates tools firmware host-tests` lists only the defining file.
- Change: Delete `version`, `Drawer::typing` and the three keyboard accessors. Make the rest private.

### The scripted mesh is compiled into the library
- Where: ui/group/mod.rs:10 (`pub mod sim;`), ui/group/sim.rs (1,042 lines)
- Category: Public surface
- Weight: low
- What: The host's fixture mesh, with synthetic names, addresses and Dublin coordinates, is a public module of the crate the firmware links. The linker drops it on the board, but it is part of the firmware's API and builds with every firmware build.
- Evidence: `pub mod sim;` with no `cfg`.
- Change: Put it behind a `sim` feature that the tests, examples, `tools/ui-sim` and `tools/ui-web` turn on.

## Files read

Read in full: ui/stage.rs, ui/group/mod.rs, ui/group/keyboard.rs, ui/group/layout.rs,
ui/group/sim.rs, ui/group/words.rs, ui/drawer/mod.rs, ui/drawer/messages.rs, ui/drawer/parts.rs,
ui/drawer/removals.rs, ui/drawer/rows.rs, ui/events.rs, ui/members.rs, ui/slide.rs,
tests/stage.rs, tests/group.rs, tests/drawer.rs, tests/members.rs, tests/messages.rs,
tests/removals.rs, tests/common/mod.rs, examples/render/events.rs, examples/render/group.rs,
examples/render/members.rs, examples/render/messages.rs, examples/render/removals.rs.

Read in part, for context outside the partition: AGENTS.md, context/BACKLOG.md (the UI, memory
and messages entries), context/design/DECISIONS.md, ui/second.rs (its `Slide`, `Page`),
ui/script.rs (`Driver::step`), ui/picker.rs (`Picker`'s fields),
crates/octowhere-node/src/node.rs (refresh numbering, leave, `forget_messages`),
crates/octowhere-node/src/view.rs (`RefreshView`, `MessageView::thread`, `session_after`),
crates/octowhere-mesh/src/{clock,schedule,table}.rs (constants),
crates/octowhere-motion/src/declination.rs (its arrays), firmware/src/main.rs:2655-2680 (the frame
loop's wait).

Not read: context/WORKING-NOTES.md beyond its size, since the brief's list of owner decisions and
DECISIONS.md covered what the findings touch.
