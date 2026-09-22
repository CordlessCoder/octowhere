# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- add SX1272/73 and SX1276-family chip variants
- apply variant-specific register mappings and SX1272 errata workarounds
- use burst FIFO transfers and preserve received payload lengths
- fix signed SNR and FEI decoding, including fractional FEI conversion, power-ramp updates, and high-power PA selection
- preserve the payload-size feature, validate public configurations, and widen FEI arithmetic
- keep OCP conversion safe for values below the datasheet range
- clear stale modem interrupts before starting a receive or transmission
- use the correct SX1272/SX1276 field positions for bandwidth, coding rate, header mode, and CRC
- reject CRC and timeout flags independently of packet-header metadata
- expose runtime modem configuration, optimization helpers, and raw temperature measurement

## 0.2.0

### Added

- `async` examples
- `async_half_duplex` examples
- `sync` examples
- `crc` getter
- `random` method

### Changed

- move IQ inversion out of `RxConfig` and `TxConfig` into dedicated `set_iq_inversion` driver method.
- move preamble set to `Sx127xLoraConfig`
- make `optimize_rx_response` public

### Removed

- `RxConfig` struct
- `config_rx` method
