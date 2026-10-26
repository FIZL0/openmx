# 1 — switch fails trick (FINAL)

Task P161. 2026-10-26.

## What I did

- The bug: `Garage::switch` (P102) spawned the other machine and left the bike's rider as it was —
  the trick machine running, the score card's combo open and scoring. The HUD tracker follows the
  card every frame whatever machine is up, so the pop-up stood over the plane, and after the switch
  back the combo banked on the bike's next landing.
- `game_sim::rider::Rider::trick_lost` (new): the wreck entry's half for the tricks and the points,
  and nothing for the bike — `tricks.reset()`, the latched flight trick cleared, the trick pack's
  spin and front flip watch cleared (as the wreck entry does), and `Score::wrecked()`: the running
  node invalidated, the combo closed unbanked, the one-wheel clocks and the backflip watch zeroed.
- `Garage::switch` calls it on every switch that happens (not on a refusal, not on a switch to the
  machine already up). That one call covers every path:
  - the M key, the pause's Switch To Machine and a tape all go through `main.rs`'s
    `switch_machine` → `Garage::switch`;
  - online, `net/rides.rs` switches a seat at the tick its `Wanted` byte asks for another machine
    through `vehicle::switch_lane`, which builds a `Garage` around the lane and calls the same
    `Garage::switch`. So the loss is a step of the seat's `Sim` on the tick the input lands —
    rollback-safe, saved and hashed with the score card as everything else in the `Sim`;
  - a split: every seat has its own `Garage` and `Sim`, and each view's `ride_hud::Tracker` reads its
    own seat's card (`track_score`), so only the switching seat's combo goes.
- The HUD needed no change: `Tracker::update` already takes a combo that closes with nothing banked as
  a bail (`outcome Some(false)`, `BAIL_COLOUR`, no "N Stunt Combo Total", played out over `OUT` and
  cleared). score.rs and ride_hud.rs, named on the task line, needed nothing new — `Score::wrecked`
  is exactly the bail the task asks for.
- From opentennis: nothing; it has no machines and no trick combos.

## What the original does

- **Can it switch mid-air?** The prompt's gate, as R8/P103 read it (`research/oracle.txt`,
  `freeride_near_machine` and `freeride_switch_prompt`; `./kit fn 0x003a8330`, `./kit fn 0x003a7210`):
  a driverless, unlocked machine within 30 ft, and the rider not wrecked and not above 58.667 ft/s.
  Nothing in either reading looks at the airborne word, so a slow rider in the air within 30 ft of a
  parked machine is offered it — in practice a short hop beside a parked plane, since a jump of any
  size is ridden above 40 mph. The port's `vehicle::offer` is the same gate, with no airborne test
  either. The pause's "Switch To Machine" calls the same swap with no range or speed gate; whether
  the original's pause menu greys it out in the air was not checked.
- **What the swap does to a running trick:** `machine_swap` (`./kit fn 0x00210b90`) takes the
  controller off the old machine, hides the bike's nodes and runs the driver/HUD hand-over
  (`./kit fn 0x0027b9f0`); I saw no call into the score card or the trick clocks in either, but did
  not follow every virtual call. So what the original does with a combo open at a swap is not known;
  the port's loss is the user's ask (2026-10-26), made as the bail that already exists.

## How it was proved

- `crates/openmx/tests/switch_trick.rs` (no disc): the stock bike ridden on a flat floor until it has
  touched down, thrown from 400 ft, with a 24-stunt set of one-second tricks (the stock rider has
  none), the tricks pressed through the real trick machine and score card, each tick the garage's
  step then the HUD tracker, as the shell does:
  - `without_a_switch_the_trick_banks` — the control: the same trick lands and banks;
  - `a_switch_mid_trick_loses_it_as_a_bail` — onto the plane mid-trick: nothing banked, the combo
    closed, the trick machine stopped, the pop-up `outcome Some(false)` in the bail colour with no
    combo total, gone after the play-out; back on the bike nothing is in progress and three
    seconds of riding on banks nothing;
  - `a_switch_mid_combo_loses_the_whole_chain` — a first trick ridden out, a second started (pop-up
    `X 2`), then the switch: the whole chain lost, the first trick's points with it; the rest as above.
- With the `trick_lost` call commented out both switch tests fail (combo still open, scoring,
  490 / 2570 points on the card), and pass with it.

## Not verified / not 1:1

- Whether the original keeps, banks or loses a combo open at a machine swap: not identified (see
  above); the loss is the port's own, the user's ask.
- Whether the original's pause menu offers Switch To Machine in the air: not checked live; the port's
  pause offers it anywhere, as before.
- No live capture of a mid-air swap in PCSX2: a slow airborne rider within 30 ft of a parked plane
  is a set-up I did not build; the airborne reading above is from the decompilation notes alone.
- The online path is proved by construction only (`switch_lane` calls `Garage::switch`), not by a
  netplay test: a lane switch needs the disc's `biplane.phy` as its template, and the new tests are
  disc-free.
- The test rider's 24 stunts are made up (one second each), not the disc's table; the trick machine
  and the score card they drive are the ported ones.

## Next

Nothing.
