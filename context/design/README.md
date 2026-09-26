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
| `renderer/` `*.py` | The concept renderer's sources (Python, Pillow, NumPy). The identity's frame-by-frame choreography lives in `concept/startup_s1_g17.py`, its G4/G10/G11 inputs and `startup-g19-matched/matched_scatter.py`; D3 is `concept/family_pass_v3.py`, S1 settings `concept/settings_study.py`, C1 `concept/compass_noise.py`. They read fonts from `renderer/fonts/`, which is not committed: see below |
| `references/` | The video timing map, the reference index, and the firmware's own captures of 25 Sep, which the hand-off names and `compass_noise.py` and `family_pass_v1.py` composite over |
| `inputs/briefs/SCREEN-DESIGN-BRIEF_4_2026-09-25-round4.md` | The screen brief the design agent worked from, second in the hand-off's order of authority. It records the firmware as of 25 Sep; `context/SCREEN-DESIGN-BRIEF.md` is the current one |
| `PROJECT-MAP.md`, `SCREEN-SYSTEM-REVIEW.md` | The design agent's map of its own work and its review of the screen family. Paths they name outside this folder (`DESIGN-LOG.md`, `renders/`, `history/`, `glitch-package/`) are only in the backup |

The renderer's fonts are the same files as those committed under `assets/`, under short names.
To run a renderer, link them into `renderer/fonts/` first:

```sh
cd context/design/renderer && mkdir -p fonts && cd fonts
ln -sf "../../../../assets/KH Interference TRIAL/OTF/KHInterferenceTRIAL-Bold.otf" KHB.otf
ln -sf "../../../../assets/KH Interference TRIAL/OTF/KHInterferenceTRIAL-Light.otf" KHL.otf
ln -sf "../../../../assets/KH Interference TRIAL/OTF/KHInterferenceTRIAL-Regular.otf" KH.otf
ln -sf "../../../../assets/PPFraktion-Free for personal use v1.1/Mono/PPFraktionMono-Bold.otf" MonoB.otf
ln -sf "../../../../assets/PPFraktion-Free for personal use v1.1/Mono/PPFraktionMono-Regular.otf" MonoR.otf
ln -sf "../../../../assets/PPFraktion-Free for personal use v1.1/Sans/PPFraktionSans-Bold.otf" SansB.otf
ln -sf "../../../../assets/PPFraktion-Free for personal use v1.1/Sans/PPFraktionSans-Light.otf" SansL.otf
ln -sf ../../../../assets/MarathonShapiro_Wide65.ttf Shapiro.ttf
```

Then run a renderer from `renderer/`, giving it an output folder so it leaves the committed
renders alone: `PYTHONPATH=. uv run --no-project --with pillow --with numpy python
concept/family_pass_v3.py /tmp/d3`. The D3, S1, K1, H2b and C1 renderers reproduce the committed
renders exactly this way. `concept/glitch-package/` holds only the two H2 stills the settings
study composites.

The full backup, with the earlier packages, every exploration, the reference stills and the
licensed fonts, is `context/octowhere-design-project/`. It is ignored by git and kept only
locally; the owner has it. The Marathon reference videos sit in `context/` and are excluded too.
