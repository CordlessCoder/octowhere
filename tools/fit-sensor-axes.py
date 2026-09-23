# /// script
# dependencies = ["numpy"]
# ///
"""Fits each sensor's axes to the screen's from the axis check screen's `[POSE]` log lines.

Usage: uv run tools/fit-sensor-axes.py <serial log>

Every rotation that maps axes onto axes is tried; both chips have right-handed axes, so
reflections are left out. For the accelerometer, the pose says where up is; for the
magnetometer, a hard-iron offset and Earth's horizontal and vertical field are fitted by least
squares. The best few are printed with their residual. `AxisMap` in `main.rs` takes the result:
screen axis i is the sign times the sensor axis named in position i.
"""
import itertools
import re
import sys

import numpy as np

E, N, U = np.eye(3)


def frame(x=None, y=None, z=None):
    """Rows: the screen's x (right edge), y (top edge) and z (out of the glass), in east-north-up."""
    if x is None:
        x = np.cross(y, z)
    if y is None:
        y = np.cross(z, x)
    if z is None:
        z = np.cross(x, y)
    return np.array([x, y, z])


# Must match `ui::axis_check::POSES`.
POSES = [
    frame(z=U, y=N), frame(z=U, y=E), frame(z=U, y=-N), frame(z=U, y=-E),
    frame(z=-U, y=N), frame(z=-U, y=E),
    frame(y=U, z=N), frame(y=U, z=E), frame(y=-U, z=N),
    frame(x=U, z=N), frame(x=-U, z=N), frame(x=U, z=E),
]

records = {}
for line in open(sys.argv[1], errors="replace"):
    m = re.search(r"\[POSE\] pose=(\d+) mag=\(([-\d.]+), ([-\d.]+), ([-\d.]+)\)uT "
                  r"accel=\(([-\d.]+), ([-\d.]+), ([-\d.]+)\)", line)
    if m:
        values = [float(v) for v in m.groups()[1:]]
        records[int(m[1]) - 1] = (np.array(values[:3]), np.array(values[3:]))
print(f"{len(records)} poses")

maps = []
for order in itertools.permutations(range(3)):
    for signs in itertools.product([1, -1], repeat=3):
        m = np.zeros((3, 3))
        for axis in range(3):
            m[axis, order[axis]] = signs[axis]
        if np.linalg.det(m) > 0:
            maps.append(m)


def name(m):
    return "(" + ", ".join(("-" if m[i].sum() < 0 else "") + "xyz"[int(np.argmax(abs(m[i])))]
                           for i in range(3)) + ")"


fits = []
for m in maps:
    error = sum(np.sum((m @ accel - 9.81 * (POSES[k] @ U)) ** 2) for k, (_, accel) in records.items())
    fits.append((error, name(m)))
print("accelerometer:", ", ".join(f"{n} {e:.1f}" for e, n in sorted(fits)[:3]))

fits = []
for m in maps:
    # raw = mᵀ · (pose · field) + offset, with field = horizontal · north − vertical · up.
    rows, values = [], []
    for k, (magnetic, _) in records.items():
        north, up = m.T @ (POSES[k] @ N), -(m.T @ (POSES[k] @ U))
        for axis in range(3):
            row = np.zeros(5)
            row[axis], row[3], row[4] = 1, north[axis], up[axis]
            rows.append(row)
            values.append(magnetic[axis])
    rows, values = np.array(rows), np.array(values)
    x = np.linalg.lstsq(rows, values, rcond=None)[0]
    rms = np.sqrt(np.mean((rows @ x - values) ** 2))
    fits.append((rms, name(m), x))
for rms, n, x in sorted(fits, key=lambda fit: fit[0])[:3]:
    print(f"magnetometer {n} rms={rms:.2f}uT offset=({x[0]:.1f}, {x[1]:.1f}, {x[2]:.1f}) "
          f"horizontal={x[3]:.1f}uT vertical={x[4]:.1f}uT")
