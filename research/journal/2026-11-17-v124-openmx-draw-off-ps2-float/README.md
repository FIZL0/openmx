# V124 — openmx draw off ps2 float

2026-11-17. Task: `./kit plan V124`.

The openmx crate's draw side audited for `ps2_float` paths that feed no sim, replay or oracle test, as
C26 did game-render. Only one was found: the wreck camera's lags and aim turn, now host f32 and checked
against the originals over generated inputs. The rest are type seams into sim APIs or ports proven in
the PS2's arithmetic. Details in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
