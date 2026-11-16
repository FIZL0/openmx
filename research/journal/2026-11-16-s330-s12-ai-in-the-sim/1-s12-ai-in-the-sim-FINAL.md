# 1 — s12 ai in the sim (FINAL)

Task S330. 2026-11-16.

## What I did

C33's AI bodies now hold much longer:

| Rider | Before (agrees through tick) | After |
|-------|------------------------------|-------|
| ai0 | 14 | 28 |
| ai1 | 18 | 28 |
| ai2 | 16 | 206 |

`C33_AGREES_THROUGH` in crates/game-sim/tests/ai/race4.rs is now `[28, 28, 0, 206, 6]`. Three changes did it:

- **The others are read off their cards.** `ai::Lane` has a new field, `cards`, with one `ai::Other` per player.
  - `Lane::feed_cards` and `Other::fed` (crates/game-sim/src/ai.rs) fill it.
  - The values: the bike frame's position and look rows (`xyz`, each `w` kept), the chassis velocity, and its
    length (mula/madda/madd, sqrt).
  - `Sim::step_all` (crates/game-sim/src/lib.rs) feeds the cards once, after every rider's frame.
  - If the cards are empty at the first step, it fills them from the riders as they stand.
  - `Sim::think` hands each brain the other players' cards. Before, it built them live with `other_of`
    (the centre of mass, this tick).
- **The brain thinks after its bike's frame.**
  - `Sim::levers_of(i)` (crates/game-sim/src/tick.rs) gives the controls of the levers the last think left.
  - The bike rides those. Then `Sim::think` runs after the frame (and after `props_back`).
  - `think` no longer returns controls.
  - The lever lag (the bike rides the last frame's levers) is unchanged. What changes is that the brain sees
    its own bike where this frame left it.
- **An AI rider's stick is not squared again.**
  - `ride::Controls` has a new field, `squared`. `Held::step` takes the stick as it comes when it is set.
  - `ai::ride_controls` sets it. Pads leave it false.

## What the original does

- The original's whole frame (the scene root's update, recipe in research/oracle.txt under S330) run in Ee from
  C33's image keeps ai0's body bit-exact through row 23 (all 20 frames run).
- The riders' own updates, or the collision node update alone, part at row 15, exactly as the port did. The
  difference is the race cards.
- The cards are fed once a frame after every rider has moved. This happens through the score manager's tick and
  its race handler, on free09 too, where there is no race.
- A brain's list of the others is their cards (S12e's oracle helper in tests/ai/sim.rs already read them):
  - position (+0x60): the bike node's position, not the centre of mass
  - velocity (+0x110)
  - look (+0x120)
  - speed (+0x108)
- Hooking the card feed out of the whole frame makes ai0 steer 1.0 and part at row 15, the same as the port did.
- C33's image's cards were fed at the end of the image's own frame (their velocity is the image tick's, −1.0626).
- The hand-off's lever stores are the last lever writes of a rider's update. They come after the bike's limiter.
  - So the brain's view of its own bike is this frame's.
  - With the cards fed, the port's steer then matched the original's tick for tick (t10 −0.05581683).
- +0x628/+0x62c are the stick as the limiter reads it. A pad's read writes them squared; the AI's hand-off
  writes them directly.
  - The port squared the AI's steer a second time (−0.0558 → −0.0031).
  - At full lock that was hidden: 1.0 and 0.718 hit the same rate limit.

## How it was proved

- A bit-exact replay of a save-state capture (C33, `s12e_free09s_four_racers_ride_as_the_capture`):
  - ai0 and ai1 agree through tick 28, ai2 through 206.
  - Every agreed row is compared on the body's centre, quat, velocity and spin.
- The diagnosis came from temporary probes, removed before landing. They ran the original's frame in Ee and
  logged the brain's reads and the stores to ai0's lever words, comparing:
  - the whole frame
  - the whole frame with the card feed hooked out
  - the bike/node updates alone
- `./kit check -- -p game-sim --test ai`: 55 passed.

## Not verified / not 1:1

- **ai0 and ai1 part together at tick 29.** They are 3 ft apart, and sideways velocity and yaw spin jump.
  - This is most likely the two bikes touching (rider-against-rider contact) at the launch.
  - Not traced.
- **ai2 parts at tick 207**, around 138 ft up near (4273, 3531). Not traced.
- **The human still parts at row 4** (task scope: AI bodies only). Its card is therefore wrong from then on.
  - The AIs read it, so the AI marks may partly wait on the human.
  - Feeding the port the interpreted original's human card did not move ai0 and made ai2 worse. But the
    interpreted human is not the capture's either (it has no pad).
- **The card's other words are not modelled on the free ride's lane:**
  - the active word is taken as "not wrecked"
  - the marked bit is false
  - progress is 0
  - the brain's own card (flags, the settle clock at +0x1b0) stays as the image left it
  - The original's feed sets the grounded bit 0x400 and the settle clock grows. The seeker reads them for
    landings above 0.5 s, which C33 doesn't reach before tick 29.
- **Where the feed runs relative to the seam wrap** (`wrap_round`, after every rider) is a guess. free09
  doesn't wrap.
- **`think` now runs after `props_back`**, so the brain's sweep sees the props in the world. Before, it ran
  while the props were lent to the bike. No replay walks a prop near an AI rider to tell which is right.
- **The flick ring still samples an AI rider's stick.** Whether the original's player-record sampler runs for
  an AI bike was not checked.

## Next

- S336: trace ai0/ai1's tick 29 (rider-against-rider contact between two AI bikes at the launch).
- The human's row 4 (its own task).
