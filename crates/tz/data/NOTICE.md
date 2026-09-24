# Time zone data

`zones.bin` is built by `tools/tz-data.py` from two sources.

- The zone boundaries are from [timezone-boundary-builder](https://github.com/evansiroky/timezone-boundary-builder),
  release 2026d, simplified. They are derived from OpenStreetMap data, © OpenStreetMap
  contributors, and are made available under the
  [Open Database License 1.0](https://opendatacommons.org/licenses/odbl/1-0/). `zones.bin` is
  a derived database under the same licence.
- The zone names and rules are from the [IANA time zone database](https://www.iana.org/time-zones),
  release 2026d, which is in the public domain.
