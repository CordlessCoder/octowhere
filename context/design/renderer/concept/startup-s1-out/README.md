# OCTOWHERE start-up: S1 self-test and G3 identity motion study

Concept frames at 466×466, built from the supplied backup renderer. Test completion times, 047° compass/clock fixture values, UTC/version microtext and status values are renderer fixtures, not hardware captures. These are design experiments; no firmware was changed or timed.

## Sequence and semantics

Success: six real driver checks (`POWER`, `CLOCK`, `TOUCH`, `MOTION`, `MAGNET`, `GNSS`) → 200 ms all-decided hold → identity in purple/lime → existing hatched OCTOWHERE logo card → F2-style clock entry. `OK` still means only that the driver answered; GNSS `OK` does not mean position fix. The decision counter counts passes and failures, as in the existing sequence. The presentation does not hold up boot to wait for a decorative phase.

Failure: six checks, a failed check turns red immediately at its deadline → 300 ms hold after all are decided → existing fault K for four seconds → clock. It does not play the success identity. The failure demo uses MAGNET as the fixture failure. The clock then reflects its actual state in an implementation; the concept's final clock is a visual fixture and does not simulate missing sensor data.

## Changes explored

1. **Self-test / S1.** Replace the 3×2 registration grid with one vertical six-row index. Each row shows its number, canonical 5×5 subsystem glyph, name and `--`, `OK` or `FAIL`. The larger title and horizontal separators echo the S1 settings index while preserving all six simultaneous outcomes. White means a responding driver, red means a failure. Completed glyphs still build by rows.
2. **Identity / restrained (G2r).** Retain G2 layout B, the real-data microtext, the lime word, hatch column, chosen logo card and ~1.2 s final hold. Swap the constantly rotating purple scatter for independently generated vertical scan columns that hard change a few times and then rest. The clock keeps blue block noise, so the identity has its own texture and color grammar.
3. **Identity / stretch (G3).** The same composition and field, but the word starts as tall blocks and steps down through 3.15×, 2.6×, 2.1× to the established 1.8× height as more letters arrive. It settles before the flicker and logo card. This is the version in the full success GIF.
4. **Clock arrival.** After the existing impact card, the F2 face reveals its upper content, lime band and lower labels in short hard stages. Once revealed, the frame is the F2 clock. This is a compositional mockup, not a timing change to the implemented clock entry.

## Files

- `startup-S1-success.gif`: the complete successful concept path.
- `startup-S1-failure.gif`: the complete failed concept path with the unchanged fault K treatment.
- `identity-G2r-restrained.gif`, `identity-G3-stretch.gif`: isolated motion comparison for the identity before its card.
- `startup-S1-storyboard.png`, `startup-S1-failure-path.png`: the two branches clearly separated.
- `selftest-S1-all-pass.png`, `selftest-S1-fail.png`, `identity-G3-held.png`: native stills.
- `startup_s1.py`: renderer. Place in the backup's `renderer/concept/` and run `PYTHONPATH=. python3 concept/startup_s1.py OUTPUT_DIR` from `renderer/` with Pillow and NumPy installed. It imports the existing G2, fault K, logo card and F2 texture generators.

## Family relationship

The vertical test index and S1 settings share reading order, indexes, outlined modular icons and short state labels. The identity has purple vertical signal strips and lime title, the fault owns the red full field, and the clock owns its broad lime slab with blue background noise. The compass remains a dial and H2 AOD stays sparse. This gives each phase its own composition while reusing glyph construction, typefaces, clipped round geometry and hard cut timing.

Before firmware implementation, check six-row readability on the panel, rendering costs of the revised identity and F2 entry, boot time relative to real asynchronous driver deadlines, and correct sensor-dependent clock state after failure. The original G2 final layout, logo card and fault K were retained as the baseline; this package is an alternative study, not a replacement specification.
