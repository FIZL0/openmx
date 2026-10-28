# 1 — wreck sound draws (FINAL)

Task S173. 2026-10-28.

## What I did

- Captured both wreck emitters live over S109's reason-11 fall: `capture/voices.toml` + `capture/play.sh`
  (P144's play.sh pointed at the new spec), animandpump.p2m2 played in copy 1, frames 3700..3959,
  0 torn rows → `context/capture/s173/voices.csv` (git-ignored). The run:
  `./kit pcsx2 with -- bash research/journal/2026-10-28-s173-wreck-sound-draws/capture/play.sh <pid> context/recordings/animandpump.p2m2 3700 260 context/capture/s173/voices.csv`
  with `WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1`.
- `crates/game-sim/src/wreck_sound.rs`: `Kit` (the two emitters' clip lengths and `MinImpulse`s,
  configuration) and `Clocks` (how long the voice and the clank stay busy, the clank's last pick, the
  body's speed the tick before). `voice_entry` is the old `Emitter::wreck` on its own busy clock;
  `voice_frame` is the `[IMPACT]` cry on the thrown body's touches; `clank_entry` and `clank_frame` are
  the bike's `[Wreck]` clank at the player's wreck entry and on the wreck body's hits. A pick sets its
  emitter busy for its clip's length plus `TAIL` (0.4 s, the shell's `WRECK_TAIL`); `age(dt)` runs both
  down at the top of the tick. `magnitude` is the emitters' own accumulator length.
- `Rider::frame_bumped`: ages the clocks first; on the wreck tick asks `clank_entry` (on `|ride.mv.g_peak|`,
  when there is a wreck body) before the body's entry, and the entry's voice pick now reads its clock; on
  every other tick, after the physics, the clank's frame (wrecked, the wreck body's record touching, its
  shock peak) then the voice's (the ragdoll's `sensed` touching and speed, or the crash's `landed` /
  `rider_vel`, as `audio.rs RiderBody::of` reads them), all on the game generator. The voice frame runs only
  for a rider with a ragdoll kit, as the entry's pick always has; the clank only with a wreck body.
- The rider's new fields: `wreck_kit` (not on the wire, put back from the live rider on restore like the
  other config) and `wreck_clocks` (on the wire, hashed as `wreck_clocks` only once not idle, so a rider
  never down hashes as before). Pins moved: `RIDERS_WIRE_LEN` 13499 → 13563 and its digest (16 bytes a
  rider), `size_of::<Sim>()` 11808 → 11904. The riders' tick hashes did not move.
- The shell: `audio::wreck_kit` / `Reading::wreck_kit` read `rider.aud`'s `[Fall]`/`[IMPACT]` and the bike's
  `[Wreck]` clips' decoded lengths and the two `MinImpulse`s; main.rs reads them once, whether the sound is
  on or not, and gives every rider `new_rider` makes the same kit.
- Oracle names: `wreck_clank_play`. Notes under P180's block in research/oracle.txt (S173).

## What the original does

The rider emitter's frame (`./kit dis 0x0033e790`, emitter in a0, dt in f12): with +0x23c clear, the body
record (+0x240) reason (+0x104) not 0 nor 13, its +0x1bc set and `MinImpulse < |+0x130|`, and the emitter
idle (vt +0x90), it picks `[IMPACT]` with the no-repeat pick (a draw for two or more sounds) and plays it.
The bike emitter's frame (`./kit dis 0x003dc8c0`): both gate words set, idle, `|*(+0x224)| >= MinImpulse` →
`rand()` always, index `int(n·r·2⁻³¹)`, +1 mod n when equal to +0x220 and n > 1. The entry's call
(`./kit fn 0x003dcb90` → `0x003dc730`) is the same pick with no gate words. Both lengths are
`mula x·x; madda y·y; madd z·z; sqrt.s`.

Over S109's wreck the original picks three times in all: `[Fall]` 2 at 3726 (the body record's reason
rises; the bike's reason rose at 3725), the clank 3 and `[IMPACT]` 0 at 3792 (the wreck body's first
touchdown). The entry's clank is silent (impulse 0 off the back); the later hits (15.2, 15.5, 8.4) are
under 18.8; the gated touches at 3793..3795 and 3818 find the voice busy (3818 is 0.867 s into a 0.9295 s
clip).

## How it was proved

All in `crates/game-sim/tests/wreck_sound.rs` (`./kit check -- -p game-sim --test wreck_sound`, 8 passed):

- `the_voice_frame_matches_the_original`: rider_voice_frame in the EE on first_recording's state over
  10 000 generated cases (reason −1..16, touching words 0/1/0x100, velocities on, a bit either side of and
  up to 3× MinImpulse, emitter busy 0/1/2, generated `[IMPACT]` lists and last picks) — cried or not, last
  pick and generator state equal on every case.
- `the_clank_matches_the_original`: bike_wreck_clank and bike_wreck_entry_clank in the EE over 10 000
  cases (a third entries; gate words, busy, impulses about 18.8, lists 1..4, last picks) — clanked or not,
  +0x220 and the generator equal.
- `the_s109_wreck_draws_as_the_original`: the port's `Clocks` driven tick by tick off voices.csv's gates
  with the original's own clip lengths pick on exactly the ticks the original's last picks move (3726 Fall,
  3792 clank, 3792 IMPACT — three draws, none elsewhere over 3700..3959), the original's index is one of
  each tick's draws through the port's pick, and ≥ 7 gated touches/hits stayed quiet on the clocks.
- `the_clank_always_draws_and_the_clocks_hold`: a list of one still draws; clip + tail ticks; gates.
- The old S53 oracle tests (`the_pick_…`, `the_wreck_sound_…`) and S109's
  `the_wreck_enters_from_the_riding_bodies` still pass: the entry's draws did not move.
- `./kit check --all` before the pin update: only the two pins failed (plus lobby/LAN load flakes that
  passed on the lone rerun).

## Not verified / not 1:1

- The voice's tail past its clip is not measured: it is taken as the bike's 0.4 s (P164 bounds only the
  bike's, 0.34–0.78 s). S109's wreck does not pin either tail: its only quiet touches fall inside the clips.
- The sim's clip lengths are the shell's decoded ones, up to 1.3 ms shorter than the original's +0x24 (less
  than a tick; could flip a pick that lands on the boundary tick). Without a disc the kit has no lengths and
  each emitter is busy for the tail alone.
- Every seat's kit is the first bike's (`new_rider` gives all riders one), and online sims cloned from
  seat 0 carry seat 0's: a seat on another bike with a different `[Wreck]` list would count and time its
  clanks by the first bike's.
- What the port feeds the voice frame is the port's stand-in, not the original's record: +0x1bc is a latch
  in the capture (1 from the first touchdown to past the put-back), the port's is the ragdoll's per-tick
  `sensed.touching`; +0x130 is the record's velocity, the port's the ragdoll's `sensed.vel`, held as the
  larger of this tick's and the last (as audio.rs did). Proven only for the original's own inputs (the
  capture test); the port's own S109 ride, whose downed bodies are not the original's, was not counted.
- The order of the clank's and the voice's draws inside one tick (bike's frame first here) is not pinned:
  at 3792 the clank's index fits draw 2 or 3 of seven and `[IMPACT]`'s draw 0, 1 or 6. Where they fall
  among the tick's other draws (dust and the like) is not known either.
- The voice frame's own gate (+0x23c) is not modelled: it is 1 in first_recording's riding state and 0 all
  through the S109 wreck; what sets it was not looked for.
- The shell's audio still picks its sounds on its own dice (audio.rs); it does not yet play game-sim's
  picks, so what is heard and what the sim drew can differ in index (not in count when the clips agree).

## Next

Done here; P195 filed for the shell to play the sim's picks.
