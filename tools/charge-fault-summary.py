# /// script
# requires-python = ">=3.11"
# ///
# Summarises a charge-fault-bench log: uv run tools/charge-fault-summary.py OUT.log
import re,sys,statistics as st
rows=[]
for l in open(sys.argv[1]):
    m=re.search(r'\[BENCH\] view=(\d+) t=(\d+) full=(\w+) repaint=(\d+) changed=(\d+) draw=(\d+)',l)
    if m: rows.append((int(m[1]),int(m[2]),m[3]=='true',int(m[4]),int(m[5]),int(m[6])))
def rep(name,sel):
    d=[r[5] for r in sel]
    if d: print(f"{name:26} n={len(d):4} median={st.median(d)/1000:.2f} ms  max={max(d)/1000:.2f}")
rep('fault frames', [r for r in rows if 2<=r[0]<122])
for n in range(18): rep(f'exit {n}', [r for r in rows if r[0]==122+n])
rep('exit all', [r for r in rows if 122<=r[0]<140])
clock=[r for r in rows if r[0]==0 and not r[2] and r[3]<=4200]
rep('clock gauge-only', clock)
