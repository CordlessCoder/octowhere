# UI and firmware code-quality review, 2026-10-04

Three reviewers read the UI crate and the firmware at `b8ab7ca`, from `BRIEF.md` (what they were
told: partition, rules, categories, format). Their findings are `ui-runtime.md`, `ui-faces.md`
and `firmware.md`, with line numbers at `b8ab7ca`. The mesh and node had the same review on
2026-10-03; its clean-up is done, and its plan, `MESH-CLEANUP-PLAN.md`, is in git at `55167be`.

112 findings: ui-runtime 36, ui-faces 39, firmware 37. The eight bugs are fixed, each with a
regression test that fails without its fix, and so are the findings under "Taken since" below;
the others are open.

## Verified, and fixed

The eight bugs were checked against the code at `b8ab7ca` by reading it, beside the reviewers'
own reproductions. The reproductions ran in scratch crates outside the repository; their
sources are in `reproducers/` (the paths quoted in the findings files no longer exist). The rest
of the findings carry the reviewers' evidence and were not rechecked one by one. The heap and
frame figures in them are the reviewers' (list sizes on wasm32, frames from their own build).

1. **Power key on a dimming screen** (`ui/stage.rs` `wake_by_key`): a short press while
   `Dimmed` or `Darkening` sets `Awake` without `restart(now)`, so the timeout check later in
   the same step dims again. Fixed in `3995e0d`.
2. **Drawer over the always-on face** (`ui/stage.rs` `step_rest`, `draw`): the timeout's move to
   `AlwaysOn` does not close the drawer (only `sleep()` does), and `draw` checks the drawer
   first, so the drawer stays lit at the always-on level. Fixed in `64d3b45`: the timeout's
   rest clears what the key's does.
3. **Refresh sessions renumbered** (`octowhere-node` `node.rs`, `self.refresh = None` on leave
   and on join, numbering `map_or(1, ..)`; `view.rs` documents "since boot"): after a leave or
   a join, a new refresh makes no event and its end is not told (`ui/events.rs`,
   `ui/group/mod.rs` compare `session >` the last seen). `ui/group/sim.rs` copies the reset.
   Fixed in `e0732c0`: both count refreshes since boot; a simulator scenario covers it.
4. **Refresh screen shows 00:00** until the mesh takes the request up (`ui/group/mod.rs`
   `refresh`). Low. Fixed in `f53410f`.
5. **Identity title held after a skip** (`ui/identity.rs` static `TITLE`, freed only by
   `draw_card`): a skipped start-up never draws the card, so 36,660 bytes stay on the internal
   heap for the run. Fixed in `67848e0`: the start-up owns the title (`identity::TitleSlot`);
   `tests/startup_heap.rs` counts the heap.
6. **Outline example mislabelled** (`examples/outline.rs`): tiles labelled HOLLOW draw outlines.
   Low. Fixed in `93cd6cc`.
7. **Touch queue drops a lift or cover when full** (`firmware/src/main.rs` `put_touch_read`):
   `|| reads.is_full()` lets a new report overwrite a waiting lift or cover, against its doc
   and `AGENTS.md`. Fixed in `e642ad2`: the rule is `util::place_touch`, tested in `host-tests`,
   and a full queue drops the newer read.
8. **Corrupt RTC year reads as 2255** (`octowhere-peripherals` `rtc.rs`): `is_valid` never
   checks the year, so `bcd_to_dec_checked`'s 0xFF sentinel passes. Low. Fixed in `4caeca0`.

## Taken since

- The rest and power-key transitions, written in seven places in `stage.rs` (ui-runtime,
  Structure): one `Stage::enter` in `3fca20c`. A toast's wake differed from the others in two
  ways, now tested: from off it climbed on a panel still asleep, and over a dim it dropped a
  brightness being edited.
- The giant-glyph test's bound (ui-faces, Tests): the reserved raster's, in `6e4812b`.
- Every Public surface finding but two, in `302d45c` (UI), `1a31fae` (firmware), `cfd4e5c`
  (host-tests), `bac2d24` (peripherals) and `4abb8d6` (motion). Three of them were wrong in
  part. `messages::Conversation` is used by the drawer and stays public. host-tests needs
  `heapless`, for `util.rs`'s tests, which it compiles from `firmware/src`. And
  `octowhere-peripherals`' `allow(dead_code)` is for its register maps, so the registers the
  removed calls wrote stay. `main.rs` no longer carries `#![expect(unused)]`. The
  builds that replace the frame loop or the mesh (`fontdue-target-bench`, `lora-link-*`) allow
  unused code instead, and the link tests are functions of their own.
- The display driver, moved into `crates/co5300` and generic over its bus (owner's choice):
  its blocking path is gone (firmware, Public surface), the transfer is written once for a
  command and once for the pixels (firmware, Duplication), and the commented-out code and the
  stale headers went with the old files (firmware, Comments). Two of the old driver's resets
  disagreed; the crate keeps the one the board ran.
- The scripted mesh, with the script driver that holds it, moved into
  `crates/octowhere-ui-script` rather than behind a feature (owner's choice; ui-runtime, Public
  surface), so the firmware no longer links either.

## Worth acting on first, besides the bugs

- `async_main`'s poll frame, 15,920 bytes, sits under the frame loop for the device's life and
  inlines boot-only work (`Store::new`, `load_mesh`, `mesh_start`); unmeasured how much
  (firmware, Work and stack).
- The toast overlay holds two lists, about 22.6 KB of internal heap, for the whole run; a list
  is 11,312 bytes on wasm32, not `AGENTS.md`'s 8,848 (ui-runtime, Work and stack).
- The 2026-10-04 screens' damage tests never check what the flush sends (ui-runtime, Tests).
- Two `Slide` types confirm the same gesture by different rules (both UI reports).
- The board's group-write store is split between `main.rs` and `mesh/device.rs`, with the write
  numbering worked out in three places (firmware, Structure).

## Not reviewed

`lc76g`, `sx127x-lora`, `sx127x-common`, `tz`, `octowhere-sim`, `tools/ui-sim`, `tools/ui-web`,
and `octowhere-node`'s `view.rs` and `inbox.rs`.
