# V113 — kiowa look

2026-11-02. Task: `./kit plan V113`.

The Kiowa drawn off the disc: all four skins (`kiowa`, `kiowa2`, `kiowa_ss`, `kiowa2_ss`), the four
`bladeblur` parts hung on the posed blade nodes, `tailrotorblur` (or `tailrotorbent` once wrecked) on the
tail rotor's node, and the rotor pose (spindle about y, tail about x, by one angle) in
`game_render::kiowa`. `crates/openmx/src/kiowa_draw.rs` makes the GPU actors. Since nothing flies the
Kiowa yet (P216, S220b), the shell draws it parked at the track's `[Helicopter*]` bays, see-through like
every bay. HeloCPresets is found, parsed (4 presets) and pinned by a test, but it is not wired to a camera
because there is no flown Kiowa to follow. Checked by eye beside a PCSX2 shot of C14's free09 save state
from the same Aero Cam 1 spot. Details are in 1-kiowa-FINAL.md.
