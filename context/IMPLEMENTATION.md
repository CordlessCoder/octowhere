# Implementation brief

Base commit: `7612743727ff67a3bd805534dfd41cb37930fa37`.
Review and change the current working tree, not a checkout of the base commit.
Existing edits in Cargo.toml, Cargo.lock and src/main.rs belong to the user.
Do not revert them. Do not commit, flash hardware or publish changes.

## Partition

- Toolchain agent owns Cargo.toml, Cargo.lock, rust-toolchain.toml and
  .cargo/config.toml. Make the firmware build with espup's `esp` toolchain.
  Retain features and fork APIs unless a demonstrated incompatibility requires
  a change. Coordinate any source changes with the parent first.
- Rendering agent owns src/drivers/framebuffer.rs, src/ui/dirty.rs,
  src/ui/geometry.rs, its module declaration and new host-check files under
  host-tests/. Fix clipping and dirty-region defects.
  Provide host tests that exercise production code, including negative
  coordinates, excess input colors, first-row/column regions and non-square grids.
- Synchronisation/touch agent owns src/util.rs and src/peripherals/touch.rs.
  Fix cross-thread trait bounds, touch bounds and recoverable init errors.
  Add focused tests in owned files; coordinate host integration with rendering.
- Parent owns integration, hardware notes, documentation and src/main.rs.

Everyone shares this working tree. Preserve other agents' edits and adapt to
them. Request a partition change before editing another owner's files.

## Verification

Parent owns the final baseline and build results. The original custom toolchain
fails before compilation. Installed `esp` reports rustc 1.95.0-nightly;
the prior offline build stopped at an uncached HAL revision, not a compiler error.
Toolchain agent may fetch dependencies and run build/clippy. Other agents run
only their host checks to avoid concurrent firmware builds.
Do not change cache settings or claim speed improvements without measurements.
Keep full-frame flushing until hardware verification supports enabling partial
flushing. No existing hardware result counts as validation of these changes.

## Done

Return changed files, concrete defects fixed, exact checks and their results,
and any remaining blockers. A successful firmware build must use `esp`, with
the committed configuration and lockfile. Tests must exercise the production
logic rather than a copied implementation. The parent reviews each result.

Comments explain silent invariants, not adjacent code or compiler constraints.
Use short, plain prose. Hardware questions remain questions until verified.

## Results

- `cargo build --locked --offline`: passed with `esp`.
- `cargo build --release --locked --offline`: passed with `esp`.
- `cargo clippy --release --locked --offline`: passed without warnings.
- Host suite: 13 unit tests and one compile-fail doctest passed.
- Host clippy with `--all-targets -- -D warnings`: passed.
- `git diff --check`: passed.

The host command is in host-tests/README.md. Hardware checks remain in
HARDWARE-VERIFICATION.md. No firmware was flashed and no commits were created.
