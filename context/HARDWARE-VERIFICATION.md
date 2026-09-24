# Hardware verification questions

These questions came from static review. None is a confirmed hardware defect.

## Frame synchronisation

Answered on 2026-09-24 by `bench/tearing`; `docs/hardware-notes.md` has the timing. A full
flush took about as long as the panel's scan, so a flush started at the blanking raced the scan
and tore. TE now pulses at scan line 150, and the display core flushes from its rising edge,
behind the scan. The owner saw no tearing during drags.

## Panel wake sequence

Does the current display-on, delay, sleep-out sequence meet this panel's
requirements? Check the controller documentation and the module's init sequence.
Compare cold start and wake behaviour with sleep-out, delay, display-on.
Record the required delay and visible results before changing the sequence.

## Partial transfers

After the geometry fixes pass host checks, does partial flushing preserve both
alternating framebuffers? Exercise moving touch marks, changing text, screen
edges and regions crossing dirty-grid boundaries. Look for stale pixels and
tearing. Verify the panel accepts the aligned transfer windows.
Partial flushing is now the default. On 2026-09-24 the owner watched the compass with row-span
damage, which flushes dozens of small regions a frame while the dial turns, and it looked
correct. The other screens, touch marks and the cases above have not been checked.

## Failure recovery

If display error recovery is introduced, does an interrupted transfer leave CS
inactive and permit a complete redraw? Verify recovery from a transfer failure
before treating continued rendering as supported.
