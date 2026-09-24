# /// script
# dependencies = ["shapely>=2", "numpy", "tzdata"]
# ///
"""Builds the time zone data the firmware embeds, and the vectors its tests check against.

Usage, from the repository root:

    uv run tools/tz-data.py <combined-with-oceans-1970.json> <cities15000.txt>

The boundaries are timezone-boundary-builder's `timezones-with-oceans-1970.geojson.zip`
release, the cities GeoNames' `cities15000.zip`, and the rules the `tzdata` package's IANA
release. Writes `crates/tz/data/zones.bin` and the test vectors under `crates/tz/tests/data/`.
The format is described in `crates/tz/src/data.rs`.
"""
import datetime as dt
import importlib.resources
import json
import struct
import sys
import zoneinfo
from pathlib import Path

import numpy as np
import shapely
import tzdata
from shapely import STRtree

# Douglas-Peucker tolerance and the grid vertices are stored on, in degrees.
TOLERANCE = 0.01
QUANTUM = 0.005
# Positions are 1e-7 degree units, so a stored coordinate is this many of them.
E7_PER_QUANTUM = round(QUANTUM * 1e7)
BOUNDARY_RELEASE = '2026d'
# The transitions the tests check lie in these years.
VECTOR_YEARS = range(2026, 2036)
# Changes a TZif file lists explicitly before this are dropped, and a time before it converts as
# at it.
SINCE = int(dt.datetime(VECTOR_YEARS.start, 1, 1, tzinfo=dt.timezone.utc).timestamp())
REPO = Path(__file__).resolve().parent.parent
OUT = REPO / 'crates/tz/data/zones.bin'
VECTORS = REPO / 'crates/tz/tests/data'


def zoneinfo_file(name):
    return importlib.resources.files('tzdata.zoneinfo').joinpath(name)


def footer(name):
    data = zoneinfo_file(name).read_bytes()
    return data.rstrip(b'\n').rsplit(b'\n', 1)[1].decode()


def explicit_changes(name, since):
    """The changes a zone's TZif file lists after `since`, with the state in force at `since`
    first, as (unix, utc_offset, dst, abbreviation). Empty when its closing rule already governs
    everything after `since`."""
    data = zoneinfo_file(name).read_bytes()
    assert data[:4] == b'TZif' and data[4] >= ord('2')

    def counts(at):
        return struct.unpack_from('>6l', data, at + 20)

    isutcnt, isstdcnt, leapcnt, timecnt, typecnt, charcnt = counts(0)
    # Skip the version 1 block, which has 32-bit times.
    at = 44 + timecnt * 5 + typecnt * 6 + charcnt + leapcnt * 8 + isstdcnt + isutcnt
    isutcnt, isstdcnt, leapcnt, timecnt, typecnt, charcnt = counts(at)
    at += 44
    times = struct.unpack_from(f'>{timecnt}q', data, at)
    at += timecnt * 8
    indices = data[at:at + timecnt]
    at += timecnt
    types = [struct.unpack_from('>lBB', data, at + 6 * i) for i in range(typecnt)]
    at += typecnt * 6
    chars = data[at:at + charcnt]

    def state(index):
        offset, dst, abbreviation = types[index]
        return offset, bool(dst), chars[abbreviation:chars.index(0, abbreviation)].decode()

    later = [i for i, time in enumerate(times) if time > since]
    if not later:
        return []
    first = later[0]
    current = state(indices[first - 1]) if first > 0 else state(0)
    return [(since, *current)] + [(times[i], *state(indices[i])) for i in later]


def varint(value, out):
    while value >= 0x80:
        out.append(value & 0x7F | 0x80)
        value >>= 7
    out.append(value)


def zigzag(value):
    return -2 * value - 1 if value < 0 else 2 * value


def rings(geometry):
    for polygon in getattr(geometry, 'geoms', [geometry]):
        yield polygon.exterior
        yield from polygon.interiors


def quantised(geometry):
    """Each ring of `geometry` on the stored grid, without the closing vertex, dropping any that
    collapse below a triangle."""
    out = []
    for ring in rings(geometry):
        points = np.round(np.asarray(ring.coords)[:-1] / QUANTUM).astype(np.int64)
        keep = np.ones(len(points), bool)
        keep[1:] = np.any(points[1:] != points[:-1], axis=1)
        points = points[keep]
        if len(points) >= 3:
            out.append(points)
    return out


def encode(ring_list):
    out = bytearray()
    varint(len(ring_list), out)
    previous = np.zeros(2, np.int64)
    for points in ring_list:
        varint(len(points), out)
        for point in points:
            for delta in point - previous:
                varint(zigzag(int(delta)), out)
            previous = point
    return bytes(out)


def inside(edges, lat_e7, lon_e7):
    """The firmware's even-odd test of a point against a zone's edges, in exact integers."""
    ax, ay, bx, by = edges
    straddle = np.nonzero((ay > lat_e7) != (by > lat_e7))[0]
    crossings = 0
    for i in straddle:
        a_x, a_y, b_x, b_y = int(ax[i]), int(ay[i]), int(bx[i]), int(by[i])
        d = b_y - a_y
        left = (lon_e7 - a_x) * d
        right = (lat_e7 - a_y) * (b_x - a_x)
        crossings += left < right if d > 0 else left > right
    return crossings % 2 == 1


def edges_of(ring_list):
    starts = np.concatenate(ring_list) * E7_PER_QUANTUM
    ends = np.concatenate([np.roll(points, -1, axis=0) for points in ring_list]) * E7_PER_QUANTUM
    return starts[:, 0], starts[:, 1], ends[:, 0], ends[:, 1]


def main(boundaries_path, cities_path):
    zoneinfo.reset_tzpath([])
    features = json.load(open(boundaries_path))['features']
    boundary = {f['properties']['tzid']: shapely.geometry.shape(f['geometry']) for f in features}
    tab = importlib.resources.files('tzdata.zoneinfo').joinpath('zone1970.tab').read_text()
    listed = {line.split('\t')[2] for line in tab.splitlines() if line and not line.startswith('#')}
    # Ocean zones go last, so a point that simplification left in both a land and an ocean zone
    # takes the land one.
    names = sorted(set(boundary) | listed | {'Etc/UTC'}, key=lambda name: (name.startswith('Etc/'), name))
    rules = sorted({footer(name) for name in names})
    rule_index = {rule: index for index, rule in enumerate(rules)}
    assert len(names) < 1 << 16 and len(rules) < 1 << 8

    stored = {}
    for name, geometry in boundary.items():
        stored[name] = quantised(shapely.simplify(geometry, TOLERANCE, preserve_topology=True))

    strings = bytearray()
    placed = {}

    def string(text):
        if text in placed:
            return placed[text], len(text)
        offset = placed[text] = len(strings)
        strings.extend(text.encode())
        assert offset < 1 << 16 and len(text) < 1 << 8
        return offset, len(text)

    rule_records = b''.join(struct.pack('<HB', *string(rule)) for rule in rules)
    boundary_data = bytearray()
    zone_records = bytearray()
    change_records = bytearray()
    for name in names:
        ring_list = stored.get(name, [])
        if ring_list:
            everything = np.concatenate(ring_list)
            box = (*everything.min(axis=0), *everything.max(axis=0))
        else:
            box = (1, 1, 0, 0)
        encoded = encode(ring_list) if ring_list else b''
        name_offset, name_length = string(name)
        changes = explicit_changes(name, SINCE)
        if changes:
            print(f'{name}: {len(changes)} changes listed ahead of its rule')
        zone_records += struct.pack(
            '<HBB4iIIHH', name_offset, name_length, rule_index[footer(name)], *map(int, box),
            len(boundary_data), len(encoded), len(change_records) // 16, len(changes),
        )
        boundary_data += encoded
        for unix, offset, dst, abbreviation in changes:
            change_records += struct.pack('<qlBBH', unix, offset, dst, len(abbreviation), string(abbreviation)[0])

    header_format = '<4sHHH2xIIIII8s8s'
    rules_at = struct.calcsize(header_format)
    zones_at = rules_at + len(rule_records)
    changes_at = zones_at + len(zone_records)
    strings_at = changes_at + len(change_records)
    boundaries_at = strings_at + len(strings)
    header = struct.pack(
        header_format, b'OWTZ', 1, len(names), len(rules), zones_at, changes_at, strings_at,
        boundaries_at, E7_PER_QUANTUM, tzdata.IANA_VERSION.encode(), BOUNDARY_RELEASE.encode(),
    )
    blob = header + rule_records + zone_records + change_records + strings + boundary_data
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(blob)
    print(f'{OUT.relative_to(REPO)}: {len(blob)} B, {len(names)} zones, {len(rules)} rules, '
          f'{len(boundary_data)} B of boundaries, tzdata {tzdata.IANA_VERSION}')

    write_points(boundary, stored, names, cities_path)
    write_transitions(names)


def write_points(boundary, stored, names, cities_path):
    cities = []
    for line in open(cities_path):
        fields = line.split('\t')
        cities.append((int(fields[14]), float(fields[4]), float(fields[5])))
    cities.sort(reverse=True)
    rng = np.random.default_rng(7)
    random = zip(np.degrees(np.arcsin(rng.uniform(-1, 1, 2000))), rng.uniform(-180, 180, 2000))
    points = [(lat, lon) for _, lat, lon in cities[:2000]] + list(random)

    zones = []
    for name in names:
        if stored.get(name):
            everything = np.concatenate(stored[name]) * E7_PER_QUANTUM
            zones.append((name, everything.min(axis=0), everything.max(axis=0), edges_of(stored[name])))
    full_names = list(boundary)
    full = STRtree([boundary[name] for name in full_names])
    lines, wrong, overlaps = [], 0, 0
    for lat, lon in points:
        # Positions as the receiver reports them, in 1e-7 degrees.
        lat_e7, lon_e7 = round(lat * 1e7), round(lon * 1e7)
        found = [
            name for name, low, high, edges in zones
            if low[0] <= lon_e7 <= high[0] and low[1] <= lat_e7 <= high[1]
            and inside(edges, lat_e7, lon_e7)
        ]
        overlaps += len(found) > 1
        # The firmware takes the first zone in table order.
        expected = found[0] if found else ''
        point = shapely.Point(lon_e7 / 1e7, lat_e7 / 1e7)
        truth = [full_names[i] for i in full.query(point, predicate='intersects')]
        # The source overlaps where zones are disputed, so any of its zones counts.
        if truth and expected and footer(expected) not in {footer(zone) for zone in truth}:
            wrong += 1
        lines.append(f'{lat_e7},{lon_e7},{expected}')
    VECTORS.mkdir(parents=True, exist_ok=True)
    (VECTORS / 'points.csv').write_text('latitude_e7,longitude_e7,zone\n' + '\n'.join(lines) + '\n')
    print(f'points.csv: {len(lines)} points, {wrong} with a different rule than the full '
          f'boundaries, {overlaps} in more than one zone, {sum(line.endswith(",") for line in lines)} in none')


def write_transitions(names):
    """Every zone's offset at the start of the vector years, and each change after it, to the
    second, as Python's zoneinfo gives them from the same release."""
    utc = dt.timezone.utc
    start = dt.datetime(VECTOR_YEARS.start, 1, 1, tzinfo=utc)
    end = dt.datetime(VECTOR_YEARS.stop, 1, 1, tzinfo=utc)
    step = dt.timedelta(hours=6)

    def state(zone, instant):
        local = instant.astimezone(zone)
        return int(local.utcoffset().total_seconds()), bool(local.dst()), local.tzname()

    lines = []
    for name in names:
        zone = zoneinfo.ZoneInfo(name)
        now = start
        current = state(zone, now)
        lines.append(f'{name},{int(now.timestamp())},{current[0]},{int(current[1])},{current[2]}')
        while now < end:
            after = now + step
            if state(zone, after) != current:
                low, high = int(now.timestamp()), int(after.timestamp())
                while high - low > 1:
                    middle = (low + high) // 2
                    if state(zone, dt.datetime.fromtimestamp(middle, utc)) == current:
                        low = middle
                    else:
                        high = middle
                current = state(zone, dt.datetime.fromtimestamp(high, utc))
                lines.append(f'{name},{high},{current[0]},{int(current[1])},{current[2]}')
            now = after
    (VECTORS / 'transitions.csv').write_text(
        'zone,unix,offset,dst,abbreviation\n' + '\n'.join(lines) + '\n')
    print(f'transitions.csv: {len(lines)} lines')


if __name__ == '__main__':
    main(*sys.argv[1:])
