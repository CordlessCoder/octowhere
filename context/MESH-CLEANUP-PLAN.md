# Mesh and node clean-up plan

The open part of the 2026-10-03 code-quality review of `337717f` (`BACKLOG.md`, "Simplify the
node"; tasks #63–#77), in the order agreed with the owner on 2026-10-04: the node's structure
first, the stored formats last. Written 2026-10-04 for the next session to follow.

**Base commit: `03cc818`** (master, local). Start every step from the commit the step before
left, never from `origin/master`. After the plan (`ffe5bdc`), the firmware moved into
`firmware/` (`41e6c18`) and the root became the host's workspace, on nightly, in the commit
after it. The log and the frames were unchanged by that. The image changed, since panic
locations now name the local crates by absolute path. The commands below are for the new
layout. Step 1 starts from the workspace commit.

## Rules for every step

- **No behaviour change**, unless the step says otherwise. The proof is the simulator's log:
  every scenario's node log is deterministic, so a refactor leaves it byte-identical. Make the
  baseline once, before the first edit, and compare after each step:

  ```text
  OCTOWHERE_SIM_LOG=1 cargo test -p octowhere-sim --locked \
    --test scenarios --test security --test removals --test messages --test screens \
    -- --test-threads 1 --nocapture 2>&1 \
    | grep -v 'finished in\|Finished\|Running\|Compiling\|Blocking' > <log>
  cmp <baseline> <log>
  ```

  At `03cc818` it is 15,615 lines, and two runs matched byte for byte. Keep every log string as
  it is; the security scenarios read some of them. A step that must change a string says so, and
  then the diff may show that string alone.
- **Gates.** Every line of `AGENTS.md`'s "Build and test", from the root and from
  `firmware/`, passes before a commit. The root's `cargo test --workspace` runs the UI's and
  the simulators' tests beside the mesh's, so a change to a type they use shows there. The
  ignored #84 scenario stays ignored.
- **Stack.** After each step that touches `node.rs` or a mesh type the node holds, list the
  firmware's largest frames and compare them with the baseline below:

  ```text
  OBJDUMP=$(ls ~/.rustup/toolchains/esp/xtensa-esp-elf/*/xtensa-esp-elf/bin/xtensa-esp-elf-objdump | head -1)
  $OBJDUMP -d --no-show-raw-insn -C firmware/target/xtensa-esp32s3-none-elf/release/octowhere \
    | awk '/^[0-9a-f]+ <.*>:$/ {name=$0; getline; if ($0 ~ /entry/) {split($0, a, ","); sz=a[2]; gsub(/ /, "", sz); print sz "\t" name}}' \
    | awk -F'\t' '{v=$1; if (v ~ /^0x/) v=strtonum(v); print v "\t" $2}' | sort -rn | head -12
  ```

  Record a change in the radio task's poll. Growth does not block a step (finish, then
  optimise); write the figure down.
- **Commits.** One or more per step, in `AGENTS.md`'s style (`mesh:`, `node:`, `settings:`),
  no bench detail. After each step: strike its lines from `BACKLOG.md`'s entry, mark its task
  done, and note the step and commit in the memory file `mesh-cleanup-plan.md`.
- **One agent.** Every step touches `node.rs` or `packet.rs`, so the steps run in order, in one
  session. If agents are used anyway, this document is their brief: the base commit above, one
  step per agent, in order, and the rules above count as done.

## Baseline at `03cc818`

| What | Value |
| --- | --- |
| Simulator log | 15,615 lines, identical across runs |
| Radio task's poll | 15,824 bytes (14,576 before the 2026-10-04 security fixes) |
| `async_main`'s poll | 15,920 |
| `Mesh::pair` | 6,240 |
| `Group::restore` | 6,000 |
| `identity::verify` | 5,376 |
| `Mesh::queue_unsaved` | 5,360 |
| `Pairing::start_transfer` | 5,344 |
| `Group::clone` | 5,136 |
| Flash image, plain build | 1,848,064 bytes (`espflash save-image`, `AGENTS.md`'s options); 1,846,768 after the move |

Steps 1 to 3 left the log identical (`0a25c3d`, `7381762`, `2e21c2c`; the radio task's poll took
16,352 bytes from step 1). The UI and firmware review's bug fixes came next and changed behaviour
on purpose. Their one log change is a new scenario, `refreshes_count_on_across_leaving_and_founding`.
From step 4 on, compare with the log at `4caeca0`: 15,699 lines, identical across runs. The image
there is 1,846,480 bytes, and the frames are as after step 1.

## Step 0: the boards (whenever they are connected)

Not a refactor, and independent of the steps. The 2026-10-04 security fixes changed what goes
on the air (header flag bit 1, record type 11, the clock's bound and agreement) and have run
only in the simulator. Flash both boards with master and check that they pair and hear each
other, that a phantom's removal switches, that the old key is dropped on the members' on-key
records, and that a restart rejoins. Record the run in `docs/logs/lora/`. Do it before step 1 if
the boards are there; otherwise run it whenever they are.

## Step 1: the node's removal state (#63)

`octowhere-node/src/node.rs` holds ten fields for removals: `rekey`, `kept`, `catch_up`,
`caught_up`, `removal_notice`, `notify`, `keys_posted`, `beacon_round`, `refill` and `on_key`.
Six places change overlapping subsets of them (line numbers at the base):

- `forget_messages` (2209) resets all ten, on leaving.
- `switch_key` (2372) keeps `kept` to the new generation, sets `refill`, `caught_up`,
  `keys_posted`, `on_key` and maybe `notify`.
- `keep` (2502), a decline after the switch, clears `kept`, `catch_up`, `caught_up` and `on_key`.
- `take` (about 1819), once every member is on the new key, clears `kept` and drops catch-ups.
- `send_old` (1455–1470) retires catch-ups, counts the removal notice down and sets
  `beacon_round`.
- `remove` (2277, 2310) and `tell_removed` (2568, 2581) set `keys_posted` and `notify`.

Move them into a `Removals` type in a new module, `octowhere-node/src/removals.rs`, whose
methods name the transitions: on leaving, on a switch, on a decline after it, when every member
is on the new key, and after an old-key packet is sent. `Mesh` holds one `removals` field.
`rekey` goes in too, as the review listed it. Expect many `self.rekey.` to become
`self.removals.rekey.`; a field the methods need is fine to keep public in the module.

Done when nothing outside `removals.rs` assigns to those fields, the log is identical, and the
frames are recorded.

## Step 2: `step()`'s choice of what goes out (#64)

`step()` (1213) chooses, in its own body, between a notice (about 1283), a packet under an old
key (1295) and this node's own slot (1305), each preceded by a `listen` whose callers reset
`after` themselves (1289, 1299, 1307; also `restart` at 2186).

- Split the choice into a plain function returning an enum, such as
  `Next::{Notice { at }, Old { at, round }, Own { round, start, send_at }}`, which can be
  unit-tested without a radio.
- Make `listen` reset `after` itself when it hears a packet.
- `send` (1908), `send_old` (1422) and `send_notice` (1494) take a `Timebase`, not an
  `Option<Timebase>` that is never `None` there.

Done as step 1, plus a unit test of the choice.

## Step 3: the tests the review flagged (#75)

- In `octowhere-mesh/src/messages.rs`'s tests, a `lacking` store is built and never asserted
  on, and a call to `a.sent((30, 1))` changes nothing that is asserted. Assert on both, or
  remove them.
- `rekey.rs`'s tests define a `group(own, ids)` helper three ways. Make it one.

## Step 4: say what the code cannot (#74)

Comments only.

- Record type 5 must never be reused (`packet.rs`, beside `record::MEMBER`).
- `IDS` is bound by 5-bit header fields and `u32` sets.
- The pairing's frame lengths 40 and 26 are literals, though `OFFER_LEN` exists
  (`pair.rs:79`): name them.
- `STORED_MAX` (`rekey.rs:717`) is 22 unlabelled terms: label them.
- A packet sends at most 8 messages and takes 16.
- `MAX_RECORDS` (`compose.rs:15`) is 3 but cannot bind, since a signed member or gone record
  takes over 100 of a packet's 239 bytes, so no more than two fit. `MAX_RECORDS + 1` is
  unexplained.
- `members.rs`'s copy of `set()` leaves out `unsent` without saying why.
- `MESH_VERSION` (`firmware/src/settings.rs:69`) is still 1, though the layouts grew through shims,
  while its comment says a later layout can tell itself apart. Correct the comment, not the
  version.
- `pair.rs` has two stacked docs, the first stale.

## Step 5: time and rounds, one way (#71)

- `ROUND_S` once, in `schedule`. `rekey.rs:28` has its own, and `node.rs:255` recomputes it.
- One conversion from a timebase's microseconds to seconds, flooring as `base_of` does. Four
  places truncate instead: `node.rs` 1530, 1754, 2316 and 2759.
- Rounds are `u32` in `rekey` and its stored format, and `i64` elsewhere. Keep `u32` where it
  is stored or sent, and convert at one helper.

Flooring and truncating differ only before 1970, so the log should not change.

## Step 6: an id set type (#69)

An `Ids(u32)` type in the mesh crate, with insert, remove, contains, iteration, count and the
raw bits. There are 113 `1 << ` sites, tests included:

| File | Sites |
| --- | --- |
| `members.rs` | 34 |
| `rekey.rs` | 20 |
| `messages.rs` | 14 |
| `node.rs` | 13 |
| `table.rs` | 9 |
| `absorb.rs` | 7 |
| `compose.rs`, `schedule.rs`, `firmware/src/settings.rs` | 4 each |
| `packet.rs`, `unsaved.rs` | 2 each |

Commit a module at a time. Raw `u32` stays at every boundary that is stored or sent; leave
`settings.rs`'s byte writes alone, since no host test covers them.

## Step 7: one mismatch counter (#70)

`Requests::heard` (`members.rs:955`) and `Summaries::heard` (`messages.rs:717`) each count
digest mismatches their own way, and their names hide that they change the group and the
store. Give them one counter type, and names that say they change state. Renaming them changes
no log string.

## Step 8: records and headers built one way (#68)

- `Builder` frames a record six ways (`packet.rs` 394–473: summary, message, positions, member,
  gone, on key). Make one `record(kind, len, write)`.
- Add `Header::new` for the four headers `node.rs` builds from literals (1352, 1430, 1501,
  1913), and a sealing builder that seals the packet when it finishes.

## Step 9: names and types (#72)

- `messages::Name` (`messages.rs:116`, `(u8, u32)`) collides with `members::Name`; rename it.
- `Pending.switch` (`rekey.rs:184`, this device's round) and `NewKey.switch` (`rekey.rs:98`, the
  group's) need names that tell them apart.
- `now: u32` with 0 for unknown becomes an `Option`: `is_ahead`, `utc_seconds` and their
  callers, and the pairing's `utc`.
- The pairing's `Phase::Transfer` and `Compare` duplicate its own fields.
- `MeshView`'s refused pairing becomes its own state, not a placeholder `Searching` phase. This
  changes `octowhere-node`'s view, so run the UI's tests and both simulator tools.

The log may change only where a renamed type's `Debug` output is printed; check every changed
line is that.

## Step 10: the mesh crate's leftover public surface (#73)

Not rechecked since `337717f`:

- `rekey::Last` (`rekey.rs:214`) and `Undo` (226) are public with public fields; check whether
  anything outside uses them.
- `pub mod bits` (`lib.rs:11`).
- `Group::new`, used only by tests, carries a stale doc.

Before narrowing, grep the node, the firmware, the simulator, the UI and both tools for uses.

## Step 11: copies and rebuilding each step (#76)

The one step expected to change frames, downward.

- `Pairing::start_transfer` (`pair.rs`, frame 5,344) clones the 2.6 KB group on the receive
  path, though its only failure comes before any change.
- `old_slot_at` (`node.rs`, about 1385) rebuilds a 1 KB `Schedule`, an HKDF and 32 AES blocks,
  at every step while a member is being caught up. Keep the old key's schedule.
- Check the remaining `Box::new(group.clone())` temporaries.

Record the frames before and after.

## Step 12: the group header's flash layout (#67)

`firmware/src/settings.rs` writes and reads the group's key, this device's id and the generation at
offsets one apart (line 55 documents them; the write is at 437–463, the read about 395).

1. Add a test of today's bytes first. The firmware's settings code has no host test, so copy
   today's encoder into the mesh crate's test as the reference.
2. Move the encode and decode into the mesh crate.
3. Add a round-trip test.

The bytes must not change, and `MESH_VERSION` stays 1.

## Step 13: one slot encoding (#66)

Last, as it touches both the wire and flash formats. A member or gone record is encoded in four
places:

- `Builder::slot` (`packet.rs:481`), on the wire;
- the pairing's `welcome` (`pair.rs:985`) and `read_welcome` (1017);
- `settings.rs`'s `write_member` (120) and `read_slot` (156).

The decoders tell a member from a gone record by different length rules.

1. First, a test of each path's bytes as they are at that point, flash ones copied from
   `settings.rs` as in step 12.
2. Then one encode and decode in `members.rs`, used by all three paths, with one documented
   length rule, and every byte unchanged.

## Matters of taste (#77)

Only when the code is touched anyway:

- `Group` mixes replicated data with send bookkeeping.
- Positional bools: `Message::private` takes eight arguments.
- `Clock`'s `(i64, i64)` tuples.
- Mixed byte orders: do not churn; pick one for new formats.
- `seal`/`open` and `seal_bound`/`open_bound` could be one pair.

## After the plan

- Rerun `bench/stack-watermark` on the boards, since the frames moved (`BACKLOG.md`, the
  removal entry).
- Update `AGENTS.md`'s stack and image figures.
- Remove the "Simplify the node" entry from `BACKLOG.md`, and this file, once every step is
  done; git keeps them.
