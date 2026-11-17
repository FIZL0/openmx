# 1 — quad meets bikes (FINAL)

Task MD179. 2026-11-16.

## What I did

- Gave `game_sim::quad::Ride` what a car has for S249/F26:
  - `hull`: the chassis' frame mesh (`ATV_R_ChasisF.col` / `ATV_S_ChasisF.col`, beside the riding
    `…M.col`; `quad::hull_file`) stood at scene node 0's world frame, where its record's rays stand.
  - `flown` / `pushed`: its `atv_body::Body` and centre of mass carried on `rider_contact::Flown`
    (new `quad: Option<FlownQuad>`). The bike's `Bump::resolve` weighs it with
    `atv_contact::Weighed` instead of the plane's `contact::Weighed`, then the body goes back with
    position kept, as a car's does.
  - `step_met`: after `record_on`'s refill, the bikes' mirrors are filed on the held list
    (`Held::file_mirrors`, up to the record's room). `Held` keeps the bikes' chassis (`bikes`,
    off the wire) so `resolve` no longer refuses entries naming another body.
- `vehicle::AtvRide` reads the hull mesh (optional), and has `hull` / `flown` / `pushed` / `step_met`.
  `Garage::step_among` passes the seat's `met`; `Garage::hull_pushed` routes to the quad.
- main.rs's split pass lists ridden quads' hulls after the cars', chains their flowns into each
  bike's Bump, and gives the quad's seat `met_riders` from its mirrors.
- Test `split_contact::a_split_bike_is_stopped_by_the_other_seats_quad` (main.rs). A bike at 30 ft/s
  into a standing Race 250 P: with the hull, the bike ends at z 35.5 behind the quad and the quad is
  pushed 17.7 ft (up to 28 ft/s). With no hull, the bike rolls to z 41.9 and the quad moves 0.43 ft
  (settling). Skips without the disc or `mods/machines/mxvatv_atv`.
- ../opentennis: nothing to take (no vehicle contact there).

## What the original does

MX vs. ATV's quad is in the same world as the other machines through its frame mesh (`…F.col`),
as the cars are with theirs. The bikes are Untamed's; the two never meet in either original, so
this pairing is the port's own.

## How it was proved

Looked-right only: the openmx test above (pushed vs. not pushed, with and without the hull). The
existing atv_quad fixtures still replay (gate), which covers the record path with no mirrors.

## Not verified / not 1:1

- The hull's category is the car's `COLLIDABLE`. The quad record's own category word is not read
  off SLUS-21104; this choice makes ghost bikes ignore it as they ignore a car.
- Resolving the bikes' mirrors on the ATV record (as Vehicle sides, ATV `Weighed` on the quad) is
  not proved against SLUS-21104. No capture has a quad struck by another body (MD98 is unported).
- The record's touch flags and the wreck's contact view are not set from mirrors. `wreck_entries`
  reads the pass's own contacts, so a bike strike cannot wreck the quad rider.
- A push writes back `Ride.body` only. The rig's copy (`Held.body`) keeps its non-view fields until
  its next rig.
- The quad pushed by a bike is a looked-right test, not a frame-by-frame compare: the bikes and
  quad come from two different games.
- Online (MD173) is not wired; only the split-screen pass is.

## Next

MD173 can put the same hull / flown / pushed / met through net/rides.rs as the cars go.
