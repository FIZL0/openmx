# 1 — untamed bike step (FINAL)

Task MD93c. 2026-11-03.

## What I did

- **`handling::untamed::vehicle_update`** (`crates/game-sim/src/handling/untamed.rs`). It works over a new
  `untamed::Update` (the words it reads and writes) and reports its calls as a new `untamed::UpdateCall`.
  The callback is handed each call together with `&mut Update`, so a call can change what the update
  reads after it. This is the `Call` style of `quad::update` (`m.call(Call::BikeVehicleUpdate(dt), q, body)`).
  `UpdateCall` is its own enum, not more variants on the existing `untamed::Call`, because it carries `F`
  (no `Eq`) and I did not want to touch the wreck entry's and put-back's type.
- **Not wired into the ride.** SLUS-21104's update is not ported either: `quad::update` hooks it as
  `Call::BikeVehicleUpdate`, and the ride's frame is the port's own (`ride.rs`). Untamed's would take that
  call's place once a ride runs SLUS-21104's update as a function.
- Nothing from opentennis applies: it has no vehicle update.

## What the original does

I read 0x004f9bd0 with `./kit dis -p game` on a scratch `KIT_CONFIG` (this slot's kit.toml with
`paths.context = context/untamed` and an `[[overlay]] game` at 0x48ea80, which picks up
`context/untamed/elf/game.elf`). I read SLUS-21104's 0x004e1b90 with `./kit fn -o atv`, and named the
vtable slots off both RAM images. The field and slot map is in research/oracle.txt `# MD93c`.

It is SLUS-21104's update, with every field and slot moved, plus one addition:

- **No vehicle state.** Only the base update runs, and it returns without clearing `+0x1690`.
- **The skip bit** (bit 3 of a global flag byte; it is clear in both images). When it is clear:
  - the ghost steps;
  - the vehicle state is read again. Kinds 3 and 4 are remote; the others are local.
  - **Local:** the wreck test (handed 0), then the stunt idle.
  - **Remote:** the stick, the brake and an input word are zeroed. The listeners step, then the node is put
    where the body is, then the stunt clocks run, then the remote's wreck setter is handed the reason. The
    reason is read after the stunt clocks.
- **New in Untamed (remote only).** After zeroing the stick, it works out the stick's angle and length
  again from the zeroed words:
  - `x · rsqrt(1, x² + y²)`, clamped twice;
  - looked up in the arc cosine table, the same lerp as the 360's heading;
  - negated when `y < 0`;
  - the length is `sqrt(x² + y²)`.
  - For a zeroed stick the angle is always the middle of table entries 14999 and 15000, and the length 0.
    The port does the arithmetic the original does rather than storing a constant.
- **The tail runs whether or not the skip bit is set:**
  - the request countdown counts down, and raises the one-frame request `+0x34c` when it reaches 0;
  - a timer above 0 counts down, and when it runs out its object (`*(+0x17b8) + 0x30`) is told (vt `+0x90`)
    if there is one, then the timer is zeroed;
  - the base update runs, and `+0x1690` is zeroed.
- **The timer's test** is `c.ole timer, 0` / `bc1t` (skip when `<= 0`). The decompile says `0 < timer`. The
  port follows the asm.

## How it was checked

- **`untamed_vehicle_update_is_the_originals`** (`tests/untamed_reworked.rs`) runs 20,000 generated states
  on `nat_or.bin`.
  - **Hooks.** All nine callees are hooked: the ghost, wreck test, stunt idle, listeners, node, stunt clocks,
    remote wreck, the timer's object and the base update. Each hook checks `a0` (the bike, `bike+0x30` for
    the listeners, the object for the timer) and logs `dt` from `$f12` and the arguments.
  - **Hooks change state.** Each hook also changes the bike the way the port's callback does: the ghost
    removes the state or changes its kind, the wreck test and listeners set the countdown, the idle and
    node set the timer, the clocks set the reason, the setter sets the request, the object sets the timer,
    and the base update sets the cleared word. So every read after a call is checked to come after it.
  - **Comparison.** The call lists must be equal. Every word of the bike and the harness's vehicle state
    must match by bits. The original may write nothing outside those and the stack.
  - **Coverage.** Each path reaches at least 100 states: no state, skipped, local, remote, the ghost took
    the state, the ghost changed the kind, timer out (told), timer out (alone), request raised. Kinds 0–5
    and `u32::MAX` are generated, which covers the unsigned `kind − 3 < 2`.
  - **Arc cosine table.** The table and its scale in RAM are compared against `AcosTable::build()`, as in MD93b.
- **Hand mutations, each red and then reverted:**
  - no angle and length recompute (SLUS-21104's behaviour);
  - the state read before the ghost;
  - the reason read before the stunt clocks;
  - `+0x1690` cleared on the no-state return;
  - `+0x1690` cleared before the base update;
  - the timer zeroed before its object is told;
  - only kind 3 remote;
  - the skip bit skipping the tail too;
  - the request raised at 1;
  - `y <= 0` negating.
- **Regressions.** `untamed_reworked` (all 8), `untamed_step` and `atv_quad` pass with their images present.
  `./kit audit` shows only the 4 leaks already on main.

## Not verified / not 1:1

- **No capture.** No ride was recorded and the update was not compared on one; the proof is a function oracle
  on generated state with every callee hooked. None of the callees (the ghost, wreck test, stunt idle,
  listeners, node, stunt clocks, remote setter, timer object, base update) is ported here.
- **Not wired into the ride.** Nothing in the port runs `vehicle_update` yet, the same as SLUS-21104's update
  (hooked in `quad`, not ported). So the new remote-stick recompute changes nothing on screen.
- **Untraced meanings:**
  - the skip bit's flag record (`*0x003ff6b8`);
  - the input word `+0x12d0`;
  - the timer `+0x17bc`, its record `+0x17b8` and that record's object;
  - `+0x1690`.
  The port names them by what the update does with them.
- **An unordered timer.** The harness never fed a NaN or Inf timer. The port's `!(timer <= 0)` follows the asm
  for one anyway.
- **The PC exe** (`pc_bike_vehicle_update`, 0x005fab90) was not read again for this task: `-o pc` is not
  configured in this slot. I took R11's journal reading of it (ghost first, local wreck test and stunt idle,
  remote kinds 3/4 clearing the inputs).
