# 1 — s12 ai in the sim (FINAL)

Task S331. 2026-11-14.

## What I did

- `game_sim::ai::Lane::spawn` (crates/game-sim/src/ai.rs) builds the AI lane from the roster's AI riders.
  For each rider it calls `Pilot::spawn` in roster order from one generator, then puts in the rider's seat
  (`Seat::spawn`) and the race card. It also adds `ai::Entry`, the race's words a seat starts with (race
  phase, path net, live, card), and the seat constants the original's constructors write (`SEAT_*`,
  `AI_INTERFACE`).
- `Sim::spawn_ai(roster, file, difficulty, event_kind, &entry, rng)` (crates/game-sim/src/tick.rs) is the
  call for a real map load:
  - It numbers the AI riders `roster.humans + k`.
  - It takes the Sim's seeker world (built by `Sim::load_seek`) and the game's controller tables
    (`ragdoll::Tables::build`).
  - It sets `Sim::ai`. An empty roster clears it.
  - It asserts every AI player has a bike, so the shell has to `add_player` each racer and `load_seek`
    first. That wiring is P313a's (main.rs / race_start.rs), not this task's.
- The test `s331_the_lane_spawns_at_the_load_as_c33_holds_it` is in crates/game-sim/tests/ai/race4.rs.

## What the original does

- The AI interface's constructor writes the seat's speed scale 1, slow radius 100, slow speed 20 and air
  cone 0.99.
- The bike's constructor writes the look-ahead 0.5 and a bump chance of 100.
- The character reader then overwrites the bump chance with the preload-success spread's value at the
  difficulty. A second writer does the same lerp, and the port's `read_character` already does it.
- The vehicle build writes the interface's player number from the player object and sets the interface
  type to 1.
- Everything else the spawn writes (the Wander's ten draws, the character's two draws, `Racer::take`) is
  what `Pilot::spawn` already ports (S12/S12b).

## How it was proved

- **Save-state capture C33** (context/capture/c33/free09-ai4.ee.bin: free09, four racers, difficulty 90,
  free ride). The test builds a Sim in the game's player order with the image's bikes, loads free09's
  seeker world through `Sim::load_seek` with the image's slot count, spawns the lane with the retail
  AIChrctr.aip, and compares it with the image:
  - the seeker world, set for set, including the owners;
  - each pilot's seat, whole;
  - brain kind, mode and spawn flags;
  - tuning, avoidance, vehicle, reach base and max, trick plan, steer filter, weights and least speed;
  - fifteen habit numbers;
  - the controllers' curve table.
  It also checks that the generator moves 12 draws per rider.
- Result: every comparison holds. The seeker world matches the image's, owners included (their positions come from the Sim's bikes and match bit for bit). So do all three seats, the brain words, the tunings, the habits and the curve table. `./kit check -- -p game-sim --test ai -- s331 s12e_free09 --nocapture` ran it without a skip, and S12e's own C33 test still passes beside it.

## Not verified / not 1:1

- **The drawn spawn words are not proved:** the Wander's ten draws, gate delay and wheelie max. The game
  reseeds its generator before the race. Walking C33's generator back reaches a 32-bit seed (0x193e77)
  8804 draws before tick 2, and no wander-draw sequence exists within 2^32 draws back. So the image can't
  show which draws the spawns took. The order across riders (roster order, one after the other, nothing
  between) is the port's assumption.
- **The interface kind is taken as 1** for every AI rider. That is what C33 holds. The order of the
  vehicle build's kind write against the spawn was not traced.
- **The race words are static.** The lane's card, race phase and live come from `Entry` at the load and
  are never refreshed from `Sim::race` (the laps card, `race::COMPUTER_CARD`). On a race track the riders
  sit still until the card says they take part. Filed as S331b.
- **No path-net following.** `Pilot::think` passes no node, edge or spot, so a race track's AI path isn't
  followed. Filed as S331c.
- **Identity is unused.** `AiRider::identity` (the rider-database entry) doesn't reach the lane, and
  nothing in the pilot reads it yet.
- **Seek slots differ.** C33's seeker world has 4 slots and recording2's had 5. The shell passes
  players+1. The test uses the image's count, and `spawn_ai` takes whatever `load_seek` built.
- **The open closure differs.** The shell opens a place-0 zone for every player, while race4.rs (from
  the image) opens it for player 0 only. Not changed here, because it is the shell's (P313a). Both of these are filed as S331a.
- **The reach gain is not compared.** The race start sets it from the keeper's state (0.2 at state 3), not
  the spawn.

## Next

Landed. P313a can call `Sim::spawn_ai` after `add_player` for each racer and `load_seek`.
