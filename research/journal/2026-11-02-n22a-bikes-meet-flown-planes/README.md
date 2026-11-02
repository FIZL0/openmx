# N22a — bikes meet flown planes

2026-11-02. Task: `./kit plan N22a`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

In split and online a riding bike went straight through the other player's biplane. In the original
the plane is a record on the level's collision node like the bikes: the bike's body rays strike the
plane's hull mesh (`Biplane\col\biplanef.col`), the bike's resolve pushes the plane's body weighed by
its mass, the plane's own resolve takes the mirrored entries, and the rider wrecks (reason 1). That is
ported now (`rider_contact::gather_met` and `Bump::planes` on the bike side, `Flight::frame_met` on the
plane side, wired through `vehicle::Among::met` and `net/rides.rs`). It is proved bit for bit against
three splitscreen images where a bike hits a flown plane, through the contacts, every resolve call and
the whole tick. Plane against plane is not ported (no capture; split off as N22a1). Details in
`1-bikes-meet-flown-planes-FINAL.md`.
