# Design hand-off, 26 Sep 2026

The design agent's current hand-off, committed as the authority for how every screen looks. It
supersedes the per-round folders that used to sit in `context/` (clock face, wordmark, compass
animation, settings panel, round 3), whose specs are kept here under `specs/`. The owner approved
all of it: see [`DECISIONS.md`](DECISIONS.md), which also settles the questions the hand-off
left open.

Start with [`handoffs/IMPLEMENTATION-HANDOFF-CURRENT.md`](handoffs/IMPLEMENTATION-HANDOFF-CURRENT.md).
It names every screen and state, its timing, and the render to compare it against. Paths in it
are relative to this folder, which mirrors the backup's layout so they resolve.

| Path | What it holds |
| --- | --- |
| `handoffs/IMPLEMENTATION-HANDOFF-CURRENT.md` | Every screen and state, motion timing, precedence, validation map |
| `docs/OCTOWHERE-COLOR-ROLES.md` | What each colour means, including the new `VIOLET` settings role |
| `docs/marathon-ui-design-language.md` | The design doctrine, updated by the design agent; replaces `context/marathon-ui-cross-project-handoff.md` |
| `specs/` | The functional specs with the owner's decisions since, and the round 4 clock spec that K1 builds on. The hand-off and `DECISIONS.md` override them where they conflict. Their image links point at folders that were removed; git history and the backup have the images |
| `renderer/concept/*-out/`, `startup-g19-matched/` | The selected renders the hand-off validates against |
| `renderer/` `*.py` | The concept renderer's sources (Python, Pillow, NumPy). The identity's frame-by-frame choreography lives in `concept/startup_s1_g17.py`, its G4/G10/G11 inputs and `startup-g19-matched/matched_scatter.py`. They need the fonts from the backup's `renderer/fonts/` to run |
| `references/` | The video timing map, the reference index, and the dated fault and self-test captures the hand-off names |

The full backup, with the earlier packages, every exploration, the reference stills and the
licensed fonts, is `context/octowhere-design-project/`. It is ignored by git and kept only
locally; the owner has it. The Marathon reference videos sit in `context/` and are excluded too.
