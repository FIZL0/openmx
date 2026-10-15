# I4 — i4 p2m2 tape

2026-10-15. Task: `./kit plan I4`.

The human's PCSX2 input recording reads as the pad the game was handed: `first_recording.p2m2` (3621
video fields of free riding, two pads, PCSX2 2.9.127) becomes 1811 ticks of I1's report bytes and plays
through I3's `Playback` end to end, with no capture session, no PINE and no emulator in the loop. The
file proves its own byte mapping — twelve of the sixteen controls are recorded twice, as a bit and as a
pressure byte, and all 43,452 pairs agree where a neighbouring pairing disagrees 5,814 times — and the
`dt` S8 left open is settled from the recording's own save state: the 30 fps clamp, on every frame. The
one thing the file cannot say is which of a tick's two video fields the game read; both are available,
the default is the first, and the 480 pairs that differ (18 of them in a button) are counted rather than
guessed at.

Details in `1-i4-p2m2-tape-FINAL.md`; the tick phase and the per-frame `dt` are the two items S8 can
close with one capture of the recording playing back.
