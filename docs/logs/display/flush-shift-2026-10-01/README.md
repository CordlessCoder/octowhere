# Flush timing with pixel shift, 2026-10-01

`flush-shift-bench` on `bench/flush-shift`, based on master `9d45959`. Core 1 times each flush
with no wait for TE, 64 runs a case, round after round; `summary.txt` takes the median of the
rounds' medians and p90s. `before` is the region flush from `f5bceb3`, before pixel shift, in
the same binary. `full 0,0` is the unchanged straight copy.

Rerun:

    cargo build --release --offline --features flush-shift-bench
    espflash flash --partition-table partitions.csv --port <port> --non-interactive \
      target/xtensa-esp32s3-none-elf/release/octowhere
    timeout 60 espflash monitor --non-interactive -L defmt --elf <elf> --port <port> > log
    uv run tools/flush-shift-summary.py log

Findings: a shifted whole frame costs 30 to 60 µs over the unshifted 11.5 ms. The rewritten
region path costs about 0.5 µs more per row flushed, shifted or not: 15 to 22 µs on a 40 × 40
or 178 × 30 region, 0.6 % on a 466 × 120 band or a 300 × 300 square.
