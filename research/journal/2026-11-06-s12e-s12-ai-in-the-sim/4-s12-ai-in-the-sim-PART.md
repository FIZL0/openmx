# 4 — the joined AI tick, proved on free09's three riders

2026-11-12. S312, S313 and S306 have landed, so the task was reopened.

## What was done

- **`ai::Racer`** is one AI rider's brain-side state in one value: the brain, its copy of the rider
  (`Senses`), the second flag word, the seeker and its reach, the assist's aim, the controller's last
  working values, the tunings, the trick plan, the habits, the steering filter, the assist weights and
  the listeners.
- **`ai::Around`** is what one tick reads and writes outside the brain: the rider as `feed` reads it,
  the ground, the seeker's world, the interface's numbers, the card, the race's rider list, the levers,
  and the body's yaw rate and velocity.
- **`Racer::tick`** runs `ai::update` through a private `Steps` impl (`Join`) built from the ported
  pieces:
  - feed: `ai::feed`.
  - route: `seek::seek`, then `follow::tail` with `avoid`, `plan_trick` and `assist` as its three
    callouts.
  - controls: `ai::controls`, and letting go when the card lacks bit 2.
  - approach: `habits`.
  - extra: `filter_steering`.
  - hand-off: `hand_off`.
  - listeners: `listener_walk`.
  - other mode: `update_base`.

  `plan_trick` runs once a frame per rider, inside the tail, as the original does. The task line asked
  for exactly that.
- **`Told`** hands back what the tick asks of the rest of the game: the answer, the stuck rescue, the
  listeners told, the motocross feeler fan, and the steps reached that are not ported. Those are
  `"path"`, `"vehicle avoid"`, `"fly"`, `"mode two"` and `"mode four"`.
- **`ai::ride_controls`** maps the AI's levers onto a `ride::Controls`, for the Sim lane to come.
- **The generic vehicle avoidance.** `ai_update_mx` takes its motocross path only when the brain's
  mode (+0xd0) is 2, its card (+0xd4) is set and card+0x40 is non-zero. Otherwise it calls
  `ai_update_vehicle` with the same arguments. A riding brain is mode 5 (`ai::RIDE`). So every AI rider
  in a free ride avoids with the generic VehicleAI pass, which has never been ported. The S12c port
  (`ai::avoid`) is the motocross path.
  - The join takes the motocross path only on that test.
  - Otherwise it calls `Around::vehicle_avoid` if one is given, or tells `"vehicle avoid"`.
  - Filed as **S317**.

## What the original does (new this session)

- **The tail's call to the avoidance** (in `ai_tick_flag`): a1/a2 = ai+0x150/+0x154 (the ignore
  list), a3 = the position +0xc0, and t0 = t1 = +0x1d0. So the heading and the steer it writes are the
  same quad, and so are the two speeds (t2 = t3 = +0x1e0). The port's `avoid` reads the heading only
  before it writes the steer, so the aliasing does not matter on the motocross path. Whether it
  matters on the generic path is for S317.
- **`ground_query` writes the height over the point it is handed.** A point read after the call has
  the answer's height in `y`.
- **free09's brains have `tick_flag::APPROACH` set** (flags 0x3e0b82b / 0xbe0b82b). So the habits run
  every tick. S312's note that free09 never sets it was wrong.
- `NO_LEARN` and `WAS_ON_GROUND` are the same bit (seen in the seeker's learning test).
- **Two fields belong to two pieces at once.** The seeker's learnt speed is the habits' start
  preload, and the card's settle word is the habits' air time. `Join` copies each one across when it
  hands the value from one piece to the other.
- Order inside one rider's native tick on free09:
  1. `ai_feed` (two ground queries).
  2. `ai_tick_flag`: seek, `ai_seek_hand_off`, the tail's cone, then `ai_update_mx` →
     `ai_update_vehicle` (eleven ground queries), `ai_trick_plan`, the assist.
  3. The controls dispatch → `ai_controls`.
  4. `ai_habits`.
  5. The listener walk (none here).

  The riders are updated in `S12E_RIDERS` order, and the generator carries from one to the next.

## How it was proved

`tests/ai/sim.rs`, `s12e_the_joined_tick_is_the_originals_on_free09`. It loads recording2's free09
image and runs three AI riders × 60 ticks, interleaved. Each tick runs the original's `ai_update(brain,
dt)` in the interpreter, then `Racer::tick` on the port's own carried state.

- **The port's side.** It is read from RAM once, at the start, and from then on carries only its own
  results. The disc world comes from S313's loader.
- **The ground.** The original's ground answers are recorded at `ground_query` and replayed to the
  port in order.
- **The generic avoidance.** The heading, speed and flags the original's `ai_update_vehicle` left
  stand in for it.

Compared every tick:

- the asked ground points
- the answer
- `rand_state`
- both flag words
- the brain's frame, motion, ground and last position
- the flat forward and the filter's output
- the body velocity
- the stuck and climb clocks
- the four controls and the controller's angles
- the assist's aim
- the trick plan and its slot
- every habit word and the preload's firing word
- the five levers and the yaw rate
- the whole seeker

After the loop, it compares every jump's three legs in the seeker world. The test passes. Six of the
180 rider ticks draw from the generator.

## Not verified / not 1:1

- **The generic vehicle avoidance is not ported** (S317). The test feeds the original's result to the
  port, so the heading and speed the tail ends with are not the port's own work. Every riding brain
  runs this pass.
- **No AI riders are in the Sim.** Nothing yet:
  - spawns the roster's riders as bikes;
  - builds a `Racer` from a `Character` (there is no brain constructor that maps a character to the
    brain's tuning words; the test reads them from RAM);
  - steps them in `tick.rs` in actor-list order.

  The proof the task asks for, a free09 4-racer capture replayed (`rand_state` and the AI bodies), is
  not recorded or replayed. Reason: S317 has to come first, or the bodies would ride on stood-in steer.
- **The bikes do not move in the proof.** Each tick re-reads the image's frozen bike, and the
  original's `ai_update` does not step the body. So every seeker stays in state 1 on the ground. These
  were not exercised in the joined form (each is oracled on its own):
  - the take-off, flight and landing states;
  - the bump roll;
  - the trick request;
  - the path branch.
- **Not joined at all:**
  - the path follower's branch (`"path"`; no free09 brain has a path net);
  - the flying interfaces' controller (`"fly"`);
  - modes two and four;
  - the trick set: `Around::tricks` is `None` in the test. On the ground `plan_trick` never asks for a
    trick, so `gesture_gate` is not reached. The join does not call it yet. Wiring it in place of
    `Tricks::request` needs a rider in the Sim to have tricks.
- **The motocross feeler fan.** The test checks that the fan is not built on the generic path. The
  generic pass's own probes are left out of the replayed ground.
- opentennis has no rider AI to take from.

## Next

Port S317, drop the stand-in from the test, then give the Sim its AI lane:

1. A `Racer` built from the roster's `Character`.
2. A bike per rider.
3. `Racer::tick` before each AI bike's step, in actor-list order.
4. `ride_controls` into the bike.

Then record the free09 4-racer capture.
