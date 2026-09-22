# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- add chip-variant definitions for SX1272/73 and SX1276-family register maps
- expose variant-specific LoRa modem field layouts
- describe variant-specific FEI scaling and IQ inversion fields
- expose burst SPI transactions for FIFO transfers

## [0.5.0] - 2026-06-27

### Added

- `LF_MAX_HZ` and `HF_MIN_HZ` constants

## [0.4.1] - 2026-06-21

### Fixed

- export `SpiDevice` imported via conditional compilation so downstream crates don't need to duplicate logic

## [0.4.0] - 2026-06-21

### Added

- `defmt` feature

## [0.3.0] - 2026-05-17

### Added

- `PLL` register addr, `PLL_PLL_BANDWIDTH` mask, `AGC_REF_AGC_REFERENCE_LEVE` mask
- `DEFAULT_FREQUENCY_HZ` constant
- add offsets for register masks
- `PA_RAMP` mask and offset
- `get_mask_offset` fn

### Changed

- DIOx registers use `OFFSET` suffix instead of `SHIFT`

### Fixed

- `AGC_THRESH_x_AGC_STEP` masks and offsets

### Removed

- `InvalidFdev`, `InvalidPreambleLength` and `InvalidSymbolTimeout` members from `Sx127xError` enum

## [0.1.0] - 2026-04-22

### Added

- `bits` module
- `error` module
- `registers` module
- `spi` module
- `calculate` module with `frf` calculation
- `InvalidInput` error
- remove `PLL` register
