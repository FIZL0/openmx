# 1 — untamed quad put back (FINAL)

Task MD168a. 2026-11-16.

## What I did

- `crates/game-sim/src/quad.rs`: `Tailed::untamed` holds the vehicle's put-back words. When it is set,
  `frame_on` runs `Ride::tail_on_untamed` instead of `tail_on`. That one:
  - writes the last-free spot while free,
  - runs the tail with Untamed's hazard clock, and on a put-back the vehicle put-back (`untamed::put_back`)
    and a ground reset (`untamed_settle`: still body, squared axes, at the spot plus the turned c-of-g),
  - keeps the spot (`untamed_keep_spot`),
  - resets each put-back wheel (`untamed_wheel_reset`).
- `Ride::untamed` seeds the spot at the spawn (body place, node forward, up).
- `snapshot.rs`: wires `Tailed::untamed` and `untamed::PutBack`.
- `tests/untamed_race_quad.rs`: the seed reads the image's last-free and saved words. `replay` steps a
  copy with the put-back held off on any off row; if that copy matches, the row counts as "read before the
  put-back". The two hazard tests now run through the put-backs: hazard-time to row 190, hazard-on through
  all 370 rows.
- research/untamed-machines.md and research/oracle.txt notes.

## What the original does

- Quad put-back (`./kit fn 0x004db460`): wheel reset (`0x004c41a0`) on all four wheels, detach, vehicle
  put-back (`0x004ca900`), quad words.
- Vehicle put-back: spot = saved `+0xd30`, snapped to the ground (`0x004b0910`) and raised 0.3 ft along
  the normal; forward = saved `+0xd40`, up = the normal; then ground reset `0x004b9130` → orient
  `0x004b92c0` (SLUS-21104's `0x00492e50`) → body commit `0x004b94e0`.
- Keep spot (vt `+0x3ac`, `0x004cda80`): the spot is `+0xbf0` (last free) when stuck, else the body.
  It is saved only with `+0xc10` clear (no wheel on a hazard; set in the quad update `0x004dd000`) and
  ground normal y > 0.8. The forward is saved flat (y 0) and normalised.
- `+0xbf0` is written at the tail's start while free; "free" is the previous tick's clock result.
- So hazard-time goes back to where it left the ground (row 150), hazard-on always to the spawn.
- The "tick later" in the task is the capture: rows are read after the steps and before the tail, so a
  put-back row shows the tick without it.

## How it was proved

- Bit-exact replay fixtures on MD147's captures (`context/capture/md147/hazard-time.csv`,
  `hazard-on.csv`, images in `context/untamed/md147/`):
  - hazard-time: every well-read row 0–190 matches; row 180 matches the tick with the put-back held off.
  - hazard-on: every well-read row 0–369 matches (row 14 skipped as a bad read); rows 30, 61, 216, 278,
    309, 340 match with the put-back held off. Each later row lands exactly.
  - The hazard clock matches every row from 140 (hazard-time) and 1 (hazard-on), allowing the known read
    lag and, on a put-back tick, the count read before it is zeroed (hazard-on row 92).
- Tests: `untamed_quad_replays_md147_hazard_time_through_the_put_back`,
  `untamed_quad_replays_md147_hazard_on_through_the_put_backs`.

## Not verified / not 1:1

- Keep spot's caller and timing: it is called from the region tracker (`0x0018bb70` / `0x0018bf20` through
  `0x004ce960`), not traced. The port runs it at the end of every tail. Proved only by the replays.
- The forward it keeps: the port uses the tail's node forward. Not traced to the original's source; on these
  rides it gives the row's words, but a ride that turns in the air before a put-back may differ.
- Keep spot's clearance check (`0x004cdfc0`): not ported. Nothing in these rides blocks a spot.
- The vehicle put-back's rays, attachment put-back and post (8, 0x15): not modelled in the quad's path.
- The quad words the quad put-back sets after the vehicle put-back: only the wheel reset is ported; the
  rows show no other body word off, but untested words may differ.
- Hazard-time past row 190: rows 238 on are off. The port's map edge (reason 13) throws the quad at
  row 238 where the original rides on. That is the port's world box on Rocksbury, not the put-back. The
  test stops at 190.
- The ground reset is `untamed_settle`, written from the SLUS-21104 orient/commit it matches, not an
  oracle of `0x004b9130` itself.

## Next

- MD182 (filed): the Rocksbury map edge (reason 13 at hazard-time row 238).
