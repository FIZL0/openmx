# S248 — car particles

2026-11-02. Task: `./kit plan S248`.

A driven car (buggy, monster truck, baja) now throws its own particles. The bike, plane and Kiowa
manager runs each car's list off the track: `buggyparticles.dat`, `monsterparticles.dat` and
`bajaparticles.dat`, all in a freeride track's second archive. That covers tire dust, dirt sprays,
chunks, the exhausts and the wreck's dust and chunks; the tire tracks are not run. The manager's four
tire records read the car's tires the way the original's car loader wires them. In the shell, each seat
keeps one manager per car. New tests in `crates/openmx/tests/car_particles.rs` drive each car on the
disc and check that it dusts on the gas, and ride the shell headless in the monster truck. The work was
not compared to a capture of the original. Details in the numbered file.
