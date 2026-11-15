# C28 — MD37a's ATV states and rides with no uinput pad

Journal: research/journal/2026-11-05-c28-md37a-recapture. Every input is a from-savestate tape
(research/c15/tape.py) played over AT-SPI; `KIT_CONFIG` must point at a SLUS-21104 kit.toml.

- menu-quit.pad, menu-atv.pad, start-event.pad: the menu tapes, played with tape.sh, saved with `./kit pine save N`.
- tape.sh / shot.py: play a tape, then PCSX2's own screenshot.
- throw.sh / throw.py: the mid-air states.
- record.sh: one ride (research/md37a/*.pad) → context/capture/md37a/NAME.csv.

MD160 did the same for Untamed's monster truck (SLUS-21701): research/md160, journal
research/journal/2026-11-14-md160-untamed-monster-capture.

MD147 moved record.sh's recorder to Untamed's race quad (research/md147/quad.toml, play.sh), journal
research/journal/2026-11-15-md147-untamed-atv-capture.
