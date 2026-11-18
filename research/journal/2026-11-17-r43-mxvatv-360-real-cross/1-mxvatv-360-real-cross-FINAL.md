# 1 — mxvatv 360 real cross (FINAL)

Task R43. 2026-11-17.

## What I did

- Looked for every reader of the stunt id past the bike. Disassembled the whole ELF and the game overlay
  into the scratchpad and grepped the byte loads and stores by offset.
  - The "+0x904 byte" MD79 found is the rider record's stunt byte: the record is bike `+0x730`, and
    `+0x730 + 0x1d4 = +0x904`.
  - Record `+0x1d4` is read in only two places, both in the rider's idle layer (vt `+0xd4`). F62a ported
    that layer as `handling::rider::idle_state` / `idle_clips`.
  - Bike `+0x1528` is read by the bike's own animate (bike vt `+0x3e4`). It is the twin of the quad's
    animate that MD65c ported.
- Wrote `crates/game-sim/tests/mxvatv_stunt_cross.rs`. It runs both animators in SLUS-21104's code on
  state04's real bike and rider, with only the syscall stubs hooked.
- Added an R43 comment block to `research/oracle.txt`, after MD165's.
- Filed MD248: put these fades in the port, replacing MD78's chosen 0.25 s.
- Nothing taken from ../opentennis: it has no stunt animation.

## What the original does

Recipes: rider idle layer `./kit dis 0x004eb380 0x004ec330 -p atv`; bike animate `./kit fn 0x004dd8f0 -p atv`;
stunt clock `./kit dis 0x004e4900 0x004e49b0 -p atv`.

- **Rider.** While the record's stunt byte is not 0xff, the wanted state is the stunt id (44/45 for the
  360s).
  - That id has its own clip (`ATV_rider_threesixtyL/R`, 1.467 s on state04). It is played at record
    `+0x1d8`, which is the bike's `min(stunt clock / row Duration, 1)`. The 360 rows' Duration is 1.5 s.
    So the rider's clip runs at 1.467 / 1.5 of its speed and holds its last frame from 1.5 s.
  - On the tick the state changes, the controller is stopped (the pose is saved), the fade goes to 1 and
    the rate is picked. A stunt byte picks the fifth rate (`+0x628`, 2.0).
  - Each tick the fade falls by rate × dt (`adda`/`msub`), clamped to 0..1. It is the saved pose's
    weight in a slerp (turns) and lerp (positions). So the cross is linear, not smoothstep.
  - Out of the stunt (byte back to 0xff) the state changes again. The rate is the new state's: still and
    air 2.0, turn 3.0, slow pair 2.0, wreck poses and pause 0.25. A restart ask (record `+0x1d0`) takes
    the fifth rate.
  - So: **0.5 s linear in, 0.5 s linear out** (into the still/air row).
- **Bike.** When `+0x1528` differs from the id it saw last (or on a restart), it stops its controller
  (pose saved) and sets its fade to 1.
  - Each later tick the fade falls by `+0x14c0` × dt. `+0x14c0` is 3.0. Only the constructor writes it.
  - It plays the bike's clip (`+0x1534[id]`, `Bike_ThreeSixtyL/R`, 1.467 s) at clock / clip length, but
    only for rows whose UsedForBike byte is set. Rows 44/45 have it; the backflip's (41) does not.
  - Out (id back to none) the same fade crosses into the riding blend.
  - So: **1/3 s linear in, 1/3 s linear out.**
- State04's rates (rider `+0x618..+0x628`): 0.25, 0.25, 2, 3, 2. MD165 read the same on every md37a quad
  rider image.

## How it was proved

Function oracle on state04's RAM image, run in SLUS-21104's code (`tests/mxvatv_stunt_cross.rs`):

- `the_riders_fade_rates_are_the_images`: the five rates, the bike's 3.0, and no stunt on the image.
- `the_rider_crosses_into_a_360_in_half_a_second`: stunt byte 45, then the idle layer run tick by tick.
  It checks the state, that the rate is the fifth, and every tick's fade bit for bit against
  `adda(0, fade) msub(rate, dt)` until 0. Then back to 0xff: the state goes back to still at the plain
  rate, again checked every tick.
- `the_bike_crosses_into_a_360_in_a_third_of_a_second`: the bike's animate with id 45 and then 0xff. The
  change tick sets the fade to 1, and every later tick is checked bit for bit until 0.
- The tick counts are 16 and 11, not 15 and 10. dt is 0.03333, not 1/30, so 15 (or 10) ticks leave
  0.0001 of the saved pose, and one more tick clears it.

`./kit check -- -p game-sim --test mxvatv_stunt_cross`: 3 passed.

## Not verified / not 1:1

- **The port is unchanged.** It still crosses the 360 in over MD78's chosen 0.25 s, and the bike's machine
  uses the same cross as the rider's. R is research only; the fix is MD248.
- **Not watched live.** The rates were run in the original's code on a save state, not seen in PCSX2
  during a real 360. The 360 step setting the stunt byte and `+0x1528` is from MD79/R17's reading, and
  the copy into the record is read, not run here (state04 rides no stunt).
- **The state after a real 360 was not seen.** The test goes back to the still state (state04's). In the
  air the rider may go to another plain state. Every plain state but turn, slow, wreck pose and pause
  takes the same 2.0, so only a turn (3.0, 1/3 s) would differ.
- **Only the right 360 (45) was run.** The left one (44) takes the same code path and rate; its clip is
  bound on state04 too.
- **Only state04's numbers.** The rates come from the rider's constructor caller (MD165) and the bike's
  constructor. Another bike or build could set others; MD165 saw the same rates on every quad image.
- **The clips' poses under the fade were not compared.** The tests check the fade weights, not the bone
  quaternions the play writes. F62a's tests cover the play itself.

## Next

MD248 ports it.
