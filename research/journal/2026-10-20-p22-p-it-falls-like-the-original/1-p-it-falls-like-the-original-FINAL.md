# 1 — p it falls like the original (FINAL)

Task P22. 2026-10-20.

## What I did

- Settled R6's open question from the code and from RAM. Neither of the velocity's two readings is
  right. The captured `bike_vel` is a **0.8× copy** of the body's velocity, and the body moves 1.0×
  its own velocity × dt.
- Re-measured the original's flights in body units (`bike_vel / 0.8`). Its gravity, its drag and
  its integration are the port's, to the third decimal. **No physics change was needed** in
  `air.rs` or `ride.rs`, and none was made.
- Re-ran P15's take-off on the original's own approach line. The take-off angle matches. The gap
  P15 saw came from the units and from a different approach.
- Added `flight::a_flight_falls_and_carries_like_the_originals` to `crates/game-sim/tests/air.rs`,
  and a P22 block to `research/oracle.txt`.

## What the original does

`./kit fn 0x0024c7e0` is the rider body's per-frame mode method (`rider_body_mode` in the oracle).
Among other things it writes the owner's +0x530, which is the type-4 actor record at owner+0x400,
its +0x130. That is the word `./kit record` captures as `bike_vel`. The value written is the request
block's +0x90 (the block is the pointer at owner+0x488) times 0.8:

- `lui $3,0x3f4c` at 0x0024d40c builds 0x3f4ccccd.
- `vmulx.xyz $vf1,$vf1,$vf2x` at 0x0024d42c multiplies by it. The mask is xyz only, so w stays 1.0.

The same method copies the drawn transform (owner+0x580, which is `bike_xform`) from
request+0x50..+0x8c.

request+0x90 is the chassis's own velocity, chassis+0x260, bit for bit. I checked this in
recording2.p2m2's save state on all four bikes. The chassis objects are found by their vtable
0x44e8f0 at +0x08, and they sit at 0xd5d610, 0xdfa240, 0x1032f80 and 0x15a96b0. On 12 of 12 lanes,
actor+0x130 equals chassis+0x260 × 0.8 under the VU's truncating multiply. Under round-to-nearest
some lanes miss by an ulp.

S8 had already measured `bike_vel = chassis+0x260 × 0.8`. This task found the instruction that
writes it, and with it the answer to R6:

- R6's "position advances 1.25× velocity × dt" is 1/0.8.
- The 1/24 s step was a coincidence of 30/24.

## How it was proved

**Code and RAM.** The 0.8 comes from the instruction above. The equality was checked against the
save state's RAM (4 bikes, 12 lanes).

**Flights against the captures.** The new test is in `crates/game-sim/tests/air.rs`. It reads the
disc's `race125p.phy` out of MX.PSR and finds every ballistic stretch in both acceptance captures. A
ballistic stretch is at least 20 ticks with dvy/dt in (−36, −28), with the edge ticks dropped. For
each stretch, the test flies the port hands-off from the stretch's starting body velocity, high over
a flat floor, with the capture's dt per tick. Then it fits dvy/dt = a + b·vy on both sides and takes
the horizontal |dpos| / (|v|·dt). It skips the port's first frame, which has no AB2 history.

| capture | flights | ticks | original a, b, carry | port a, b, carry |
|---|---|---|---|---|
| acceptance-freeride | 9 | 621 | −32.282, −0.0610, 1.0017 | −32.275, −0.0608, 1.0000 |
| recording2-freeride | 10 | 603 | −32.240, −0.0639, 1.0006 | −32.236, −0.0637, 1.0000 |

The asserts:

- gravity: |Δa| < 0.05
- drag: |Δb| < 0.005
- carry: the original's within 0.01 of 1, the port's within 0.01

`./kit check` passed.

The −32.2 is `phy::G` × the file's `Gravity` of 1. The slope is the vertical share of
`Drag 0.025 × ½ × speed² / Weight 400`. The fixture bike (`freeride-bike.phy.txt`) has Drag 0.5.
Flown with that bike, the port shows a drag twenty times larger, so any comparison has to use the
disc's file.

**Take-off on the original's line, in a scratch harness that was not committed.** The original's
line is acceptance rows 361→389: it lands at row 358, dips, and climbs a ramp that steepens to about
28°. It leaves around (4049, 187, 3636) at 66.3 ft/s body speed, vy +36.0, 28.5°, and spends 84 air
frames. The port on the same ramp with the gas pinned leaves at 79.4 ft/s, vy +43.6, 28.8°, and
spends 81 air frames. The angle and the flight agree. The higher speed is the drive (see Next).

**Why P15 saw −38, 1.25× and vy 18.6:**

- It read `bike_vel` raw. The 0.8 copy changes at 0.8 × −32.2 ≈ −25.8.
- It compared that against the port's body velocity. Partly-grounded ticks inflated the port's
  side to "−38".
- Its "lip" at Free09 4061,3638 is a point the original was already in the air over.
- Its port run-up came from a standstill on a different line.

## Not verified / not 1:1

- **The flights are fitted, not bit-exact tick for tick.** The original's flights carry the human's
  stick, the port is flown hands-off, and the rows are the drawn root, not the chassis. A bit-exact
  airborne tick needs the replay to reach a flight, which is the acceptance run's job.
- **Air-time lean forces are not compared.** The port's flight has no input, while the human leaned
  in the original's. This test cannot tell whether the lean moves the centre of mass's fall.
- **The take-off was compared at different speeds** (79.4 against 66.3 ft/s), because the port's
  drive is too strong. Only the angle and the air-frame count were compared.
- **The approach line was approximate.** I started the port near row 361's position and heading,
  not from a bit-exact state. The harness was a temporary hack in the shell and was not committed.
- **The drawn root is not the chassis position.** The "carry" ratio on the original's side uses
  `bike_xform` translation. In the air the offset between the two moves with rotation, which is why
  the tolerance on the original's side is 0.01 and not tighter.
- **The emulator was not used.** Everything came from the save state's RAM, the captures and the
  disassembly.

## Next

- P20/P21 (drive): the port reaches 79 ft/s where the original reaches 66 on the same ramp.
  Coasting, it loses speed up that ramp much faster than the original (66 → 40 in ~1.1 s). Those
  are drive and rolling-resistance gaps, not air gaps.
- The replay harness's `acceptance_start` still seeds `chassis.vel` from `bike_vel`, so the seeded
  velocity is 20% low. That is S8's choice and I left it alone; whoever raises the acceptance gate
  past tick 0 should divide by 0.8 there.
- `Sim::bike_vel()`'s doc in `crates/game-sim/src/lib.rs` says the captured quad is "the body's
  velocity". It is 0.8× of it. I left it alone because it is not P22's file.
