# Hardware verification questions

These questions came from static review. None is a confirmed hardware defect.

## Frame synchronisation

Does waiting for TE to be high synchronise each transfer to a new frame?
Check whether a transfer can reach the next wait while TE is still high.
Compare the current level wait with a rising-edge wait or low-then-high wait.
Record the observed TE and transfer timing before choosing a change.

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
Partial flushing is now the default and no record says this check was run.

## Failure recovery

If display error recovery is introduced, does an interrupted transfer leave CS
inactive and permit a complete redraw? Verify recovery from a transfer failure
before treating continued rendering as supported.
