# C19 — the Kiowa's rotor strike, through the wreck to the put-back

2026-11-03. Task: `./kit plan C19`. This is C17's follow-up.

**What holds.** `a_captured_rotor_strike_replays_bit_exact` (`crates/game-sim/tests/kiowa_capture.rs`)
now runs `Wreckage::act` on the crash test's ask, as the MX helicopter does. It checks:

- Rows 1205..=1298 are bit for bit, as before, and the wreck row now includes the blade throw.
  - The throw's twelve `rand()`s are the original's whole move on 1298. The port's generator is
    seeded from 1297's `rng_state` and ends on 1298's.
- The rotor probe's first hit is the original's.
  - A new capture of the same tape reads the probe's ray record (`watch/c19-kiowa-strike.toml`).
  - On 1298 the record holds one hit, and `objects[0]` is a trunk (category 4).
  - The hit point is (3007.8188, 144.10242, 4051.0874), the same bits as the port's first hit,
    which is tree 138.
- The crash test asks nothing more while the wreck clock runs. The put-back comes on 1449. All
  four blades are thrown.
- After the put-back, the body stands within 0.01 ft of the original's (see below for why it is not
  compared bit for bit).

**What does not hold: the wrecked fall** (filed as a follow-up). See `1-strike-to-put-back-FINAL.md`.
