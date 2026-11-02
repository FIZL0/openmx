# S220b — the Kiowa's input step and its frame

2026-11-02. Task: `./kit plan S220b`.

I ported the Kiowa's pad input step (vtable +0x20, `kiowa_input`) as `Pad::from_sticks` and
`Pad::from_pad`. I found it from an ELF-wide scan for writers of the pad words +0x334..+0x348. I then
joined the helicopter's frame as `Kiowa::tick`: the input, then `biplane::World::step` with the
helicopter's preforce, force (S220) and MX measure (S220a) as the world's listeners, and the body
wrapper's gravity and collision response between them. `Ride` holds what the frame reads and leaves
besides `Kiowa`. `Surroundings` is the engine round it: the collision record, the rotor's strike
test, and what a crash test's answer does. Both are bit-exact by oracle on S148's live Kiowa (10,000
cases each). The frame is checked against the original's own world step, run on the state's own
listener elements: every body word, the controller, rotor and aero states, the measure, the crash
state and the rand state. One finding: with no rotor collision object, the original's crash test
reads a stale register. Details in `1-kiowa-input-FINAL.md`.
