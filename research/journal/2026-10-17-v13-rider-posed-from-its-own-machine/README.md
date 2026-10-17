# V13 — rider posed from its own machine

2026-10-17. Task: `./kit plan V13`.

V12's capture caught the renderer's two animation machines paired with the wrong skeletons: the
sixty-state machine poses the **rider's** 22-joint body off `Rider\Anim\` and binds all sixty of its
clip slots, while the 12 `bike_*` clips V9 found belong to the bike's own machine. This task unpicks
that in `game-render` — the sixty-state rig is now `anim::Stance`, which takes any set, and
`Chassis` holds one instead of being one — and takes V12's seven measured `Params` numbers and the
two lag time constants at the same time. Three new tests pose the rider's own skeleton from the
machine (60 of 60 slots bound, every frame of a minute drawn where the bike's set draws nothing on
3519 of them, and the grounded pose grid drawing across all four bands), and V9's four chassis tests
still hold. The shell is still wired the V9 way: finishing the swap moves `rider.rs` and `main.rs`,
which V13 did not come for. Details and the unproven list: the numbered file.
