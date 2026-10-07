# /// script
# requires-python = ">=3.11"
# ///
"""Sets the channel-load bench's runs side by side, per shape and motion, one column a rotation,
as means over the seeds.

    uv run tools/channel-load.py results/channel-load.jsonl
"""

import json
import sys
from collections import defaultdict
from statistics import mean


def main():
    runs = defaultdict(list)
    keys = []
    for line in open(sys.argv[1]):
        run = json.loads(line)
        key = (run["shape"], run["motion"], run["rotation"])
        if key not in runs:
            keys.append(key)
        runs[key].append(run)
    print(f"{'shape':<16}{'motion':<8}{'rot':>4}{'pkts/min':>9}{'len':>6}{'busy%':>7}{'busymax%':>9}"
          f"{'duty%':>7}{'drowned%':>9}{'age50':>7}{'age95':>7}{'agemax':>7}{'missing':>8}")
    for key in keys:
        rs = runs[key]
        m = lambda f: mean(r[f] for r in rs)
        print(f"{key[0]:<16}{key[1]:<8}{key[2]:>4}{m('packets') / rs[0]['minutes']:>9.1f}{m('mean_len'):>6.0f}"
              f"{100 * m('busy_mean'):>7.1f}{100 * m('busy_max'):>9.1f}{100 * m('max_duty'):>7.2f}"
              f"{100 * m('drowned_share'):>9.2f}{m('age_median'):>7.0f}{m('age_p95'):>7.0f}{m('age_max'):>7.0f}"
              f"{m('missing'):>8.0f}")


main()
