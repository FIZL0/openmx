# 1 — kiowa blades collide (FINAL)

Task S271. 2026-11-03.

## What I did

- `crates/game-sim/src/kiowa.rs`:
  - `Blades::shape: Option<Arc<Mesh>>`, read by `Blades::new` from a new `shape` argument
    (`BLADE_SHAPE`, `bladebentf.col`). It parses as a plain `Mesh`.
  - `Blades::knocks()`: each blade whose probes and mesh are in the broad phase (`Started::probes`),
    its mesh posed the way a knocked prop's is (`world::Prop::posed` on the node's matrix), as
    `flight_search::Knock`s for a search's `Scene::knocking`. Nothing calls it yet; the shell builds
    no blades (P265).
- `crates/game-sim/tests/kiowa.rs`:
  - `blades_for` passes the shape.
  - New `thrown_blades_land_and_sleep_as_the_originals`. It is a separate test rather than more cases
    in `thrown_blades_fly_as_the_originals`, because that one throws blades past the edge and
    everywhere mid-air on purpose. The setup is the flying test's: S148's world cut down to the
    helicopter's body, and the node's listeners emptied. On top of that:
    - free09 is loaded from the disc: its terrain, the scene's meshes (`Props`), its plants (`Trees`,
      from `free09.esb`), and the knockable props' bodies (only so the throws can avoid them).
    - The port's records start from the state's never-searched ones. Their swept from/to is
      (0,0,0,1).
    - 24 throws. Each is a calm helicopter whose four blade nodes sit 4–25 ft over the ground, one in
      each corner of an 80 ft square, so no blade meets another's mesh.
    - Each site is at least 200 ft from the helicopter's hull, 120 ft from every knockable prop and
      80 ft past every scene mesh's sphere.
    - Each frame, the original's clear (vt +0x74) and collide (vt +0x7c) run on every blade record
      filed on the node, in the node's order. Against that the port runs `Blades::search(dt, field,
      props, trees)`. Then both run the world step and the listeners, as in the flying test.
    - It runs until all four sleep, or 1,200 frames (40 s).
    - Compared every frame, for every blade:
      - `same_blades` and the node's rows;
      - the record's four touching flags, the swept from/to, the collider's relative velocity, world
        CofG and normal speed;
      - the ray probe's category list (mode and list);
      - that `knocks()` poses exactly the blades whose probes are in.
- `research/oracle.txt`: an S271 block.

## How it was checked

- `cargo test --release -p game-sim --test kiowa -- --nocapture` passed all 19, none skipped. The
  new test printed: `24 throws, bit-exact (96 landed, 77 asleep, 77 searching asleep, 128518
  contacts, 16 blade-frames on a scene mesh)`.
  - All 96 blades touch the ground.
  - 77 go to sleep and stay filed, searching with the wake mask's list (mode 0 [0x40]), as the
    original does.
  - A blade's rays met a scene mesh on 16 frames.
- `./kit check -- -p game-sim --test kiowa` passed 19.
- `./kit audit` shows only the two existing plan/DONE.md leaks.

## Not verified / not 1:1

- **Other rays striking a blade.** `Blades::knocks` poses the mesh, but nothing hands it to another
  search yet, and no test has a ray strike a blade's mesh. That covers another blade's rays, a
  prop's, the bike's and the plane's. The throws are spread out so that no blade meets another.
- **What the throws stay clear of.** The knockable props' `f.col`s, the toys' meshes and the
  helicopter's hull are in the original's broad phase but not in `Blades::search`'s, so the throws
  avoid them. A blade among them is not covered.
- **Blades still awake at 40 s.** 19 of the 96 blades were still moving when the 1,200 frames ran out
  (rocking or sliding on slopes). They stayed bit-exact to the end, but those cases never reach
  sleep.
- **Plants.** The plants are in the port's search, but whether any blade reached a trunk was not
  counted.
