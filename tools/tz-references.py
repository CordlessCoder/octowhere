# /// script
# dependencies = ["tzdata"]
# ///
"""Writes each zone's reference point, from the IANA `zone1970.tab` and `zone.tab` the `tzdata`
package carries, as `crates/tz/src/references.rs`. The zone picker lists the zones at an offset
nearest first by these. Use the `tzdata` release `tools/tz-data.py` built `zones.bin` from.

Usage, from the repository root:

    uv run tools/tz-references.py
"""
import importlib.metadata
import importlib.resources
import re
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / 'crates/tz/src/references.rs'


def degrees(text):
    """`+DDMM`, `+DDDMM`, `+DDMMSS` or `+DDDMMSS` as signed degrees."""
    sign = -1 if text[0] == '-' else 1
    digits = text[1:]
    whole = 2 if len(digits) in (4, 6) else 3
    value = int(digits[:whole]) + int(digits[whole:whole + 2]) / 60
    if len(digits) > whole + 2:
        value += int(digits[whole + 2:]) / 3600
    return sign * value


def points(file):
    text = importlib.resources.files('tzdata.zoneinfo').joinpath(file).read_text()
    for line in text.splitlines():
        if not line or line.startswith('#'):
            continue
        _, coordinates, name = line.split('\t')[:3]
        latitude, longitude = re.fullmatch(r'([+-]\d+)([+-]\d+)', coordinates).groups()
        yield name, degrees(latitude), degrees(longitude)


def main():
    reference = {}
    # zone.tab names one zone per country, which reaches the aliases zone1970.tab folds away.
    for file in ('zone.tab', 'zone1970.tab'):
        for name, latitude, longitude in points(file):
            reference[name] = (round(latitude * 100), round(longitude * 100))
    release = importlib.metadata.version('tzdata')
    lines = [
        f'//! Each zone\'s reference point, in 0.01 degrees, from tzdata {release}\'s `zone.tab` and',
        '//! `zone1970.tab`. Written by `tools/tz-references.py`; do not edit.',
        '',
        '/// Name, latitude and longitude, sorted by name.',
        'pub(crate) static REFERENCES: &[(&str, i16, i16)] = &[',
    ]
    for name in sorted(reference):
        latitude, longitude = reference[name]
        lines.append(f'    ("{name}", {latitude}, {longitude}),')
    lines.append('];')
    OUT.write_text('\n'.join(lines) + '\n')
    print(f'{len(reference)} zones to {OUT}')


if __name__ == '__main__':
    main()
