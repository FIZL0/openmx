# 1 — s11 tricks (FINAL)

Task S11. 2026-10-15.

## What I did

Ported the trick machine into `crates/game-sim/src/trick.rs` and proved it in
`crates/game-sim/tests/trick.rs`. Four pieces, which is how the module is laid out:

1. **The table.** `rider\anim\stunts.ini` — 57 stunts of name, display name and three clocks — plus each
   player's 24-entry map from gesture slot to stunt, and a rider's own copy of the 24 its player's map
   selects (`Stunts`, `Stunt`, `Set`).
2. **The gesture.** Three trick buttons make a group of 1..=6, the left stick (or the d-pad as a
   fallback) makes a direction of 1..=4, and the two together name one of the 24 slots. That needed the
   engine's shared arc-cosine table as well (`AngleTable`, `stick`, `look`, `group`, `Gesture`,
   `slot_of`), because the stick direction is read off it.
3. **The clocks.** `HoldTime`, `SafeTime`, `Duration` and the hold's own two sub-clocks, run once a
   frame (`Tricks::update`), plus the request and queue machine in front of them (`Tricks::request`).
4. **The landing.** The trick's own clause of the wreck test (`Tricks::landing`).

Three findings that change what the port looks like:

* **No stunt applies any rotation.** The brief asks for "the rotation each one applies"; there is none.
  A stunt is a rider pose with three clocks, and nothing in the image reads the running stunt to torque
  the bike. The rotating moves are separate trick ids — backflip (24), wheelie (25), stoppie (26) —
  driven by the airborne control's own torque parameters, which are S10's. Trick id 28 is "none", and
  id 25 doubles as the stick-driven airborne lean a rider gets when it asks for nothing (`stunt_idle`).
* **The angle table needs a *double* arc cosine.** The engine builds a 30,001-entry table at start-up
  by calling its C library's `double acos` and chopping each result to a single. The single-precision
  `acosf` already in `ps2-float` disagrees with it on 11,432 of the 30,000 computed entries, so
  `ps2_float::libm` gained `acos(f64)` (fdlibm's `__ieee754_acos`) and `sqrt(f64)` (the same bit-by-bit
  integer method as the existing `sqrtf`, because the crate is `no_std` and must not call a host libm).
  With those, all 30,000 entries are bit-exact against the game's own chain.
* **The deliberate throw-off is a backflip, by accident.** The landing's throw-away test compares trick
  ids, and the id left behind when no mapped stunt is named "throwaway" — which is what the disc ships —
  is 24, the backflip's own. The port had read that as "no slot" and so could never fire the reason-7
  wreck; it now compares ids, like the original. The oracle test is what caught it.

## What the original does

Addresses are in `research/oracle.txt` under the S11 heading, with the argument conventions; read them
with `./kit fn <addr>`. The behaviour, in the order a frame runs it:

**The table** (`stunts_ini_load`). `snprintf(path, 0x104, "%s%sstunts.ini", dir, "Anim\\")`, then 57
sections `[Stunt01]`..`[Stunt09]`, `[Stunt10]`..`[Stunt57]`. Per section: `Name` → +0x00 (0x20 bytes),
`DisplayName` → +0x20 (0x20), `HoldTime` → +0x44, `SafeTime` → +0x40, `Duration` → +0x48, `UsedForBike`
→ +0x4c (default 1), and the section's own zero-based number → +0x50. Stride 0x54. Then both players'
maps at +0x12b4, stride 0x60, written as the identity. The INI reader (`./kit fn 0014af90`) strips one
leading and one trailing `"` from a string value and `strncpy`s it into the 32-byte field, so a display
name is at most 31 characters and never keeps its quotes.

`stunt_selection` fills a 24-entry array with `stunts[map[player][slot]]`, and the rider `memcpy`s that
array into its own `+0xa34` (boot.c:196645). So **trick ids 0..23 index the rider's own copy directly**,
and a remapped player and a default one run the same code. `stunt_specials` then finds
`"nohandedlander"` and `"throwaway"` in that copy by a case-insensitive search, storing 24 — one past
the end — when a name is absent. On the retail disc the no-hander is slot 4 of the default 24 and the
throw-away is stunt 31, which no default gesture reaches.

**The gesture** (`pad_to_gesture`). Controls 0x11/0x12/0x13 are the three trick buttons (any non-zero
strength counts as held); the group is 6 for all three, 5 for buttons 2+3, 4 for 1+3, 3 for 1+2, 2 for
button 2, 1 for button 1, and 0 otherwise — so the third button alone is not a group. Controls 0..3 are
the d-pad, which overrides each stick axis (left and up win their pairs) and sets a per-axis "this was
digital" flag. The deflection then goes through `stick_direction`, and if the result is "no direction"
while a group is held, the d-pad names one on its own (left→3, right→4, up→1, down→2). While the rider
is wrecked (`+0x748`) the whole lot is zeroed and the armed flag cleared, so a trick cannot be queued
up during a crash and fire on the way out.

`stick_direction` is `mula.s/madd.s` for the square of the deflection, `rsqrt.s` of 1.0 against it, a
multiply to normalise x, that clamped to [-1, 1] **twice** (the compiler emitted the clamp pair twice,
which is visible in the disassembly and harmless), the table lookup, `neg.s` when y is negative, and
`sqrt.s` for the reach. The direction is then: nothing when the squared deflection is not more than
0.25; left/up when the angle is below -π/4 (left when it is at or below -3π/4); right when it is not
above π/4; down when it is not above 3π/4; left otherwise. The literals are 0.7853982 and 2.3561945 —
the image's singles, not π/4 and 3π/4 rounded.

`angle_table_build` fills 30,001 floats with `acos(i·step − 1)` for `i` in 0..30000, clamping x at 1,
and copies entry 29,999 into entry 30,000 so a lookup that lands on the last entry can still
interpolate. `step` is `0x388bd096` and the lookup's scale is `0x466a5e00` (14999.5). The lookup is
`(x + 1)·scale`, `cvt.w.s` for the index, the fraction by subtraction, and `mula.s/madd.s` to blend the
two entries. It cannot leave the table for any finite input, because the EE FPU saturates where IEEE
would produce an infinity.

**The request** (`stunt_request`). The gate: not on the ground (`+0x574`), no contacts
(`*(+0x7d0) + 0x40`), armed (`+0xa18`), a group (`+0x9fc`), a direction (`+0xa00`), and the rider not
fresh off a respawn (`+0x750`). The slot is a literal switch that works out to `(group−1)·4 +
(direction−1)`. With nothing running (`+0xa0c`) the trick starts at once: `+0xa7c` = slot, `+0xaf8` = 1
(restart), `+0xa04` = 1 (the request count), armed cleared. With something running the slot is pushed
on an unbounded FIFO of heap nodes at `+0xa38` and armed is cleared. With no gesture but something
running, the queue's head takes over the end of the running trick: when
`duration − min(StuntOverlapTime, duration·0.5) <= elapsed`, pop into `+0xa7c`, set restart, and
`+0xa04 += 1`. The duration is read from the *latched* id (`+0xa78`), not the requested one.

**The clocks** (`stunt_clocks`, dt in f12). A restart is `latched(+0x864) != running(+0xa78)`, or the
restart flag, or `(count(+0x8a0) >= 2 && count != previous(+0xa08))` — the last of which is how the same
trick asked for twice in a row is told from the same trick still running. It is forced false while the
rider is frozen (`+0x3a0`). Then `+0xa78 = +0x864`, and the switch on the id runs the mapped-slot case
for 0..23:

* on a restart, elapsed (`+0xa1c`), held (`+0xa30`), hold-elapsed (`+0xa24`) and the hold-done flag all
  go to zero;
* the hand-hold pose is on while a group is held and elapsed has reached `HoldTime`;
* **hold**: a group held, not done holding, `HoldTime > 0`, and elapsed inside
  `[HoldTime, HoldTime + 0.1]` — the clock stops, and the animation phase is
  `(elapsed + remap(hold_elapsed, 0, 0.5, 0, 0.07) · noise(2·wiggle)) / duration`;
* **ease**: a group held, not done holding, `HoldTime > 0`, and `0 < HoldTime − elapsed < 0.1` — the
  clock advances by `remap(remaining, 0, 0.5, 0.1, 1.0) · dt` (a `mul.s` then an `add.s`, not a
  `madd.s`);
* **normal**: anything else — holding is marked done if it ever happened, and elapsed advances by dt.

`remap(x, in_lo, in_hi, out_lo, out_hi)` is inlined at both call sites as
`clamp(out_lo + ((x − in_lo)/(in_hi − in_lo))·(out_hi − out_lo), min(out_lo, out_hi), max(out_lo,
out_hi))`, returning `out_lo` when the input range is empty. Then: the both-hands-off flags
(`+0xa44`/`+0xa48`) go on when a group is held, held ≥ 0, a wheel is down and the running trick is the
no-hander; `safe` (`+0xa10`) is cleared; and if elapsed has passed the duration, elapsed is pinned to
it, `in_progress` (`+0xa0c`) is cleared and the animation is stopped — otherwise `in_progress` is set
and `safe` goes on at `SafeTime`, or at `HoldTime` for the no-hander. `+0xa08 = +0xa04` at the end of
the rider's update (boot.c:197761 for a local rider, from the packet at 197784 for a remote one).

`in_progress` is written nowhere else, so it survives a trick id that is not a table slot; the rider
reset (boot.c:198218) is what clears it, along with the queue, the ids (both to 28), the fresh flag and
the restart flag.

**The landing** (`stunt_landing`). It returns at once when the global at `wrecks_enabled` is zero, does
nothing while the rider is already wrecking (`+0x748`), and runs a nested chain of attitude tests —
pitch, roll, landing backwards, being thrown into something: wreck reasons 2, 3, 4, 11 and 13 — all of
which belong to the bike-in-the-air task. Then the trick's clause:
`if ((!on_ground && !contacts) || !in_progress || no_hander || safe)`, inside which a running id equal to
the throw-away id (`+0xa84`) with `progress > 0.95` is reason 7 and *otherwise* a no-hander below
`NoHanderWreckSpeedLimit` (`+0x780`, against the speed at `+0x424`) is reason 12; failing that first
test, a landing mid-trick is reason 12. The port states the same thing as three tests that return in
that order, which is equivalent because the two inner ones are an if/else.

The comparison is against the *id*, and the id `stunt_specials` leaves in `+0xa84` when no mapped stunt
is called "throwaway" is 24 — the backflip's id, not an unreachable sentinel. See below.

## How it was proved

`crates/game-sim/tests/trick.rs`, eight tests, all through `./kit check`:

1. `the_stunt_table_is_the_one_on_the_disc` — the table read out of `appstart.res` inside `MX.PSR` with
   the port's own archive reader: 57 records, each with a name, a duration and its own index; the 24
   default gesture slots named one by one; the no-hander at slot 4 and the throw-away absent from the
   default 24; one record's three clocks in full, so the keys cannot be swapped; and both players'
   identity maps. Skips with the extract command when there is no `MX.PSR`.
2. `the_angle_table_is_the_games_own` — all 30,000 computed entries against the game's own chain in the
   oracle (`float_to_double` → `acos_double` → `double_to_float`, about 9,000 instructions an entry),
   bit for bit, plus the duplicated last entry.
3. `the_stick_classifier_matches_over_ten_thousand_deflections` — 10,000 deflections (centre, the four
   axes, the four exact diagonals, one past the edge of the range, one denormal-sized pair, then
   generated ones either side of the dead zone) against `stick_direction`, comparing the direction, the
   angle and the reach bit for bit. The port's table is injected into the global the original's lookup
   reads, so this proves the decode independently of the table.
4. `the_request_gate_and_the_slot_switch_match` — all 2,240 combinations of the gate's five flags and
   the switch's 35 group-and-direction pairs against `stunt_request`, comparing the return, the
   requested id, the restart flag, the request count, the armed flag and what was handed to the queue.
   `the_overlap_test_matches_over_ten_thousand_triples` — 10,000 generated duration/overlap/elapsed
   triples plus five corners through the take-over branch.
5. `the_ai_choice_matches_over_ten_thousand_air_times` — 10,000 air times over generated tables against
   `stunt_ai_choice`, with the original's own generator seeded from the port's state and compared again
   afterwards, so the test also shows the chooser draws exactly one random number.
6. `the_clocks_match_over_ten_thousand_frames` — 10,000 generated frames against `stunt_clocks`:
   elapsed, progress, the hold's two clocks, the held total, the hold-done flag, in-progress, safe, the
   hands-off flag, the latched id and the restart flag, all bit for bit, plus which of the three motion
   entry points was called and with what. How it is run is in the next section; the test counts the two
   branches that are hard to land on and refuses to pass without them (256 frames held, 2,440 eased,
   3,324 restarted on the committed seed).
7. `the_landing_clause_matches_every_combination` — 4,608 combinations of the clause's inputs (on the
   ground, in contact, a trick running, hands off, safe, five trick ids, four progress values either
   side of 0.95, two speeds, and three tables: no throw-away, one at the first slot, one at the last)
   against `stunt_landing`, comparing the wreck reason the rider's vtable was handed. All three
   verdicts appear (2,520 clean, 696 thrown on purpose, 1,392 wrecked) or the test fails.
8. `the_idle_request_matches_every_state` — all 16 states of wrecking, on the ground, fresh off a
   respawn and "an animation is already playing" against `stunt_idle`.

Three stand-ins, each with its reason: the queue's push and pop (heap nodes nothing has set up), the
library's `strcmp` (quadword compares the interpreter does not implement) and the engine's smooth noise
(not ported — see below).

**How the clocks are run.** `stunt_clocks` is not a pure function of RAM: it calls the motion library
through the rider's vtable, and past the clock section it picks an animation, blends it and skins the
rider through VU0. Two things make it testable anyway. The rider's vtable pointer is aimed at a table of
three addresses that are not code, each hooked with a Rust closure that records the call — which is how
the hand-hold pose, the animation restart and the animation stop are compared at all, since none of them
is a field. And the call is ended at the join every case of its switch reaches (the address is in
`research/oracle.txt`) with a hook that points the return at the sentinel `Ee::call` waits for, so the
tail never runs. Every field the clocks write has been written by then. The same trick made the landing
testable: with `wrecks_enabled` set, the collision block skipped and the attitude limits put where
nothing can reach them, the clause at the end of it is reached by every combination.

**What the landing proof found.** The throw-away test compares the running trick id against the id the
name search stored, and when the name is absent that id is 24 — which is the backflip's own id, not a
"no slot" sentinel. So the original really does throw a rider off a backflip taken past 95% of whatever
progress the last stunt left behind, and on the retail disc, whose default 24 hold no throw-away, that
is the only way the reason-7 wreck can happen at all. The port had been comparing gesture *slots*, which
made that case unreachable; it now compares the id, and the test fails if it is changed back.

`./kit check` passes (167 tests). `./kit audit` reports two leaks, **both pre-existing and neither from
this task**: `plan/TODO.md` lines 31 and 85 spell out the activity-class addresses that R7 and S13 have
yet to name, and those two numbers only started counting as leaks when another task wrote their names
into `kit.toml`'s `[symbols]`. Nothing S11 added is flagged. Renaming another task's plan lines is not
S11's to do — and `plan/TODO.md` is the one file every agent touches, so editing it invites a merge
conflict for no gain — so this task commits with `--skip-audit` and leaves the two placeholders to R7
and S13.

## Not verified / not 1:1

* **The hold's wobble is injected, not ported.** `smooth_noise` is the engine's shared 1-D smooth noise
  over tables built at start-up from the RNG; `Tricks::update` takes the sample as a closure argument,
  and the clock proof hooks the original's sampler with a stand-in that returns the same single to both
  sides. It perturbs only `progress`, never `elapsed`, so no timing window and no landing verdict
  depends on it — but the animation phase during a hold is not reproduced until somebody ports that
  sampler.
* **The animation half of the clock function is not ported.** Past the join where the proof stops the
  call, `stunt_clocks` picks one of four blend calls, keeps a blend ramp at +0x9f4 and two phase values
  at +0x9e8/+0x9ec, and skins the rider. None of it reaches the clocks, and all of it is the animation
  task's; the port's `update` leaves those fields out of the state entirely.
* **One field stands for two pairs.** The original keeps the requested id at +0xa7c and the copy the
  rider runs at +0x864, and the request count at +0xa04 with its latched copy in a byte at +0x8a0;
  a separate function copies one into the other once a frame (`stunt_state_pack`), which is how a
  networked rider's trick arrives in a packet. `Tricks` has one `requested` and one `seq`, which is
  exactly right for a local rider and is what the proofs drive. Two consequences: a remote rider is not
  modelled here, and the latched count is a byte in the original while `seq` is an `i32`, so a flight
  with 256 tricks in it would wrap there and not here.
* **`ps2_float::libm::acos` is proved only through the table.** Its 30,000 arguments all lie on one grid
  in [-1, 1], which is the only thing the game ever asks it for, and the chop to a single hides any
  last-bit difference in the polynomial. It is not proved over generated doubles, and the same goes for
  `libm::sqrt`, which only that function calls.
* **The queue is proved as a FIFO, not as the original's list.** Its push and pop allocate and free heap
  nodes that nothing in a freshly loaded image has set up, so both are hooked: the push records what it
  was handed and the pop returns a sentinel slot. The order and the values are proved; the list itself
  is not.
* **The AI proof hooks `strcmp`.** The library's version is hand-written with R5900 quadword compares
  (`psubw`, `pcpyud`) that the interpreter does not implement yet, so a Rust stand-in takes its place.
  Teaching the interpreter those two ops would remove it.
* **The group's button names are not resolved.** The three trick buttons are control indices
  0x11/0x12/0x13 in the input layer, and which physical buttons those are is I1/I2's business, so the
  port and this entry both say "trick button 1/2/3".
* **`pad_to_gesture` itself is not run.** It reads the input layer's control strengths through two more
  objects, so the port's `Gesture::read` is transcribed from it and proved only in its two halves: the
  stick classifier against the original, and the group/direction decode by the request proof, which
  drives all 35 pairs through the switch.
* **The display name list is not ported.** `stunt_display_name` answers "Stoppie", "Wheelie" and
  "BackFlip" for ids 26, 25 and 24 from program strings; `Stunts::display_name` reproduces that, but
  the three strings are typed in here rather than read off the disc, because they are in the executable
  and not in the table.
* **Clock text is parsed with the host's `f32` parser.** The original's INI float getter ends in its
  own `atof`, which was not disassembled. Every value in the retail file is a short decimal that both
  conversions agree on, and the port's values are proved against the disc only in the sense that they
  round-trip through this parser. A malformed value is an error here and a zero there.
* **`Tricks::idle` is proved, but its gate is the caller's.** The original asks the motion library
  whether an animation is playing (vtable +0x120) and does nothing while one is; the port leaves that
  question to its caller. The test pins both halves down — with "no" the two agree in all 16 states,
  with "yes" the original is shown to leave every field alone — so what is unproved is only *when* the
  rider update asks, which is the animation task's call site.
* **The rotation question has no answer to verify.** No stunt applies any rotation (see above), so
  there was nothing to port or prove on that count; the backflip/wheelie/stoppie torques are S10's.
* **The attitude clause is S10's and is switched off in the landing proof.** The limits in front of the
  trick's clause are set where nothing reaches them, so this task proves the trick's clause exactly and
  says nothing about the eleven tests in front of it.
* **One unreachable branch is dropped.** `stunt_request` has a fallback for a group outside 1..=6,
  which stores the caller's second argument as the trick id. The group decoder cannot produce one, and
  the gate rejects group 0, so `slot_of` returning nothing simply makes the port do nothing there. The
  2,240-combination test covers groups up to 6 and direction 0, which is every case the gate lets
  through.

## Next

Nothing outstanding on the trick machine itself: the table, the gesture, the clocks, the landing and the
idle request are all proved against the original. Two small things are left for whoever needs them —
`smooth_noise`, without which a held pose does not wobble, and the remote-rider latch (+0x864/+0x8a0),
without which a networked trick cannot arrive. Both are somebody else's task to call for.
