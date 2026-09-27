# OCTOWHERE implementation update — start here

This archive is an **incremental handoff** for the implementation agent that has already built the state in `octowhere-design-backup-2026-09-26.zip`. It contains the owner's subsequently accepted charging bar and fault screen work. It is not another complete design backup or a firmware checkout.

1. Read [IMPLEMENTATION-DELTA.md](IMPLEMENTATION-DELTA.md) for the exact changed behavior, visual authority, data rules and timings.
2. Use [VALIDATION.md](VALIDATION.md) to compare native 466 × 466 captures and frame recordings with the included reference PNGs/GIFs.
3. Consult the original backup's `AGENTS.md`, `handoffs/IMPLEMENTATION-HANDOFF-CURRENT.md`, and specs for every behavior **not expressly superseded** here. Its source videos are not repackaged; **the user will provide the videos if the implementation agent needs them**.

## Authority boundary

The owner approved the V8 fault motion, the irregular V6 charging stripes as carried through the six clock states, solid noncharging fill, and the bidirectional solid/segment transition. This update supersedes the original backup's **white moving fault ticker, fault-to-clock hard cut after four seconds, hatched clock battery interior, and 1 px/frame charging hatch crawl**. It also formalizes blue `#001DFF` and yellow `#ECDB0B` as fault-ticker ink **only inside the red fault screen**, an exception to the original color-role table. The full fault hold, real self-test meaning, actual clock states and colors, settings behavior, replay semantics, touch skip, and static AOD battery treatment remain governed by the original handoff.

The included render sources use Pillow and the original design project placed as a sibling of this extracted update folder. They exist to explain geometry and timing, not to be blitted into firmware. The PNGs and GIFs are self-contained review assets. Fixture data (87%, 12%, MAGNET/GNSS, clock at 13:07) is not live telemetry.

## Folder map

| Path | Purpose |
| --- | --- |
| `references/battery/` | Six-state solid/charging comparisons, scaling, ongoing rhythm, entry and interruption renders |
| `references/fault/` | Single/multiple failure ticker, continuity, 18-frame damaged exit and overscan surface |
| `source/` | Concept compositors and cinematic timing comparison; requires the previous backup to rerender |
| `IMPLEMENTATION-DELTA.md` | Normative update against the implemented baseline |
| `VALIDATION.md` | Capture checklist and reference index |
| `MANIFEST.sha256` | Hashes for all packaged files except itself |

Verify from this directory with `sha256sum -c MANIFEST.sha256`. The downloadable ZIP has a separate checksum beside it. Do not infer implementation approval for earlier exploratory V1–V7 studies or the rejected two-lane fault variant; this package contains the selected direction.
