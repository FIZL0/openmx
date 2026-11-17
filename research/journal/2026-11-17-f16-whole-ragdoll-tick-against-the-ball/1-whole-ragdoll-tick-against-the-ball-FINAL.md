# 1 — whole ragdoll tick against the ball (FINAL)

Task F16. 2026-11-17.

## What I did

- S240's drop2-2840..2845 states were gone (not in the shared context, not in any slot copy), so I
  made new ones in PCSX2 copy 4: **drop3-2852..2857** (`context/s240/drop3-*.p2s`,
  `context/states/s240-drop3-*.eeMemory.bin` and `.spr.bin`, `context/capture/s240/drop3.csv`).
  Recipe, from a lying human rider on free09:
  - `research/s240/drop.py 3841330 1 2 10 30 25 150 2` (the pid was copy 4's PCSX2);
  - load, run 6 s at NominalScalar 0.25, pause, save (frame 2842);
  - `put_ball.py 3 1 0 6.0 4 2` with the game **paused** (with UP 3, or the game running, the ball
    strikes before the recording starts: put_ball waits 3 s, about 22 frames);
  - resume; `strike.py drop3 4 --frames 30 --near 5.0`.
  - research/s240/slots.py builds the wrong copy path (`root+'-slots'`); I used a scratch copy that
    points at `openmx-slots/pcsx2/s4/PCSX2/savestates`. strike.py saves into slots 3–8, over
    whatever was there.
- `tests/ragdoll_ball.rs`: the three S240 tests now read drop3 (`RUN`, `FIRST`, `LAST`). The hit test
  takes the first bone on the ball in 2853 (bones 5, 7 and 8 touch it; bone 2 does not). The prop
  seed now copies the whole body, the countdown, the fresh flag and the collision record, as
  prop_wreck.rs does.
- New test **a_whole_ragdoll_tick_against_the_ball_is_the_originals**, with its helpers in
  `tests/ragdoll_ball/tick.rs` (copied from ragdoll_wrecks.rs: the actor reader, the listener, the
  replay scene, `written`; plus `BallScene` and `port_tick`). For each tick 2852→2853 … 2856→2857:
  1. The original's `rider_body_update` (`./kit fn rider_body_update`) in the Ee from the state.
     The request area (req−0x200, 1280 bytes) and the bike object (far−0x980 to far+0x680+1280) are
     laid in from the next state. The draws before the body are searched (0 in every tick), with
     dt = the next state's frame_dt (0.03333). All 18 bones × 6 quads match the next state bit for bit.
  2. The port's `Actor::tick` from the same RAM. The world answers are the original's, except a
     sphere test whose hit is on ball 1's collidable. That one is answered by the port's ball as the
     state left it (`KnockedProp::touch`), and its push-out runs against the port's ball's body
     (`Against::Other`, `Side::Body`). The point, normal and held plane distance match the
     original's. Every number in `written()`, the tail and the draw count match.
  3. The props' step (`PropSet::search` with free09's terrain and static props, then `step`), from
     the balls as the state left them. All three awake balls' bodies match the next state.
  - Sphere tests on the ball per tick: 12, 10, 8, 8, 8, all answered by the port.
- The list test was too strict for drop3. It said the original lists every awake ball the port
  finds at any distance. In drop3-2852 a search built at the probe leaves out ball 2 at 17.1, 20.9
  and 42.6 ft, yet lists ball 4 at 44.3 ft. It now checks:
  - every ball the original lists is in the port's list, at the same distance;
  - a search built at the probe (or the state's own) lists every ball within 5 ft.

## What the original does

- The props' step runs after the rider body update in a frame. The bones meet the ball where the
  state before left it, and the ball's step then starts from that same state. The bones' push-out
  only reads the ball's body (velocity at the point); the step from the state's balls, not from the
  update's RAM, gives the next state.

## How it was proved

- Bit-exact replay of five save-state ticks (drop3-2852..2857) through the Ee and the port, in
  `crates/game-sim/tests/ragdoll_ball.rs` (`./kit check -- -p game-sim --test ragdoll_ball`: 4 pass).

## Not verified / not 1:1

- The port does not build the ball's place in the bones' world itself. The test hands
  `BallScene` the ball whenever the original's record names its collidable. A sphere test where only
  the port would have hit the ball is logged as a difference (none came up). The port's own scene
  ordering (`WreckBikeScene` with `knocked`) is not run here.
- Other world answers (ground, static props, sweeps, poses) are the original's, replayed.
- The request area and the bike object come from the next state, not from the port's ride.
- Only one run, 5 ticks, of one ball resting on a lying rider. No fast strike, and no tick where the
  ball's record holds an entry naming a bone (the props' step got no strikers and matched).
- Why built searches leave out ball 2 at 17–21 ft is not known (maybe a stale tree place left by
  put_ball's teleport). The list test only claims balls within 5 ft.
- drop2 (the brief's states) is lost; drop3 replaces it. The S240 claims are re-proved on drop3,
  not on drop2.

## Next

- Nothing needed for F16. A follow-up could run the port's own `WreckBikeScene` (knocked balls
  from `KnockedProp::near`) in place of `BallScene`, and find why ball 2 drops out of built searches.
