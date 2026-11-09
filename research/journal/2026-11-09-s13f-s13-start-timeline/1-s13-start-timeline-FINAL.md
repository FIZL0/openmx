# 1 — s13 start timeline (FINAL)

Task S13f. 2026-11-09.

## What I did

- `crates/game-sim/src/race.rs`: `Supercross::start_timeline` (the activity's listener on the score
  manager's countdown) and its intro's end (`end_intro`). It acts on `StartItem`s (a rider's race
  interface: an optional body as `Axes`, and four words) and returns a `StartStep` saying what it did:
  armed, intro ended plus where the countdown's elapsed time goes, overlays, call, warning. The doc on
  `shuffle_lanes` now says what the AllAI path skips.
- `crates/game-sim/tests/race.rs`: proofs 18–20 (below).
- `research/oracle.txt`: race_countdown_listener described in full. New names: race_intro_end,
  scene_camera_cut, hud_overlay_show, hud_overlay_mode, list_nth. The activity_supercross block no longer
  calls this code "unanalysed".
- Plan: S304 filed (run the timeline on `Sim::race`, with the countdown's interval and accumulator). S290
  noted with the hold this found.
- Nothing taken from opentennis: it has no races.

## What the original does

`./kit dis 2fb9b0 --len 0x420`. Ghidra has no function there. It is the activity's vtable slot reached
through the thunk at 0x300480 (listener slot 0x5c).

**How it is called.** score_manager_tick calls it at 0x25fe38, with f12 = length, f13 = elapsed and
f14 = interval. The call comes on the countdown's first tick (accumulator +0xc8 == 0) and again each time
the accumulator reaches the interval (+0xc0, 0.1 s from the reset); then `acc -= interval`. After the call
`acc += dt`.

**What it does**, in order:

1. 0x2f90e0, which only returns 1.
2. The class bit of "MXSupercrossInterface" (name_bit on the class registry).
3. If elapsed == 0 (c.eq, so −0 and denormals count): each actor in the scene's list
   ((app+0x3cc)+0x30+0x30+0x34) is cast with vt+0xa4. Each item with a body (+0x68) gets:
   - linear freedom 0,1,0 and turn freedom 1,0,1;
   - its words set to 0, 0, 1 and the intro-camera flag. The flag is: the scene camera's +0x68 is set
     and equal to its +0x40.
4. Then the first of three windows the clock is in, each `[from, to)`:
   - **lights** `[+0x4b0, +0x4b4)`: if +0x4c8 == 0, the intro's end runs (0x2fd630 with fade 1). Then
     the race overlays are shown by viewport mode: 0 "RacingOverlays", 1 Horizontal1/2, 2 Vertical1/2.
   - **call** `[+0x4b0 + 1.5, +0x4b4 + 1.5)`: 0x2fd7d0 plays clip +0x484[+0x4ac] (vt+0x1c), and
     +0x110 = −1.
   - **warning** `[+0x4b8, +0x4bc)`: every item with a body is freed on all six axes, and its words get
     +8 = 0 and +0xc = 1.
5. It returns 1. No rand.

**The intro's end** (`./kit fn 0x2fd630`):
1. Stops the scene cameras by viewport mode.
2. Moves the countdown:
   - if +0x4c4 == 0 and +0x114 < 6: +0x114 = 6;
   - if +0x4c4 != 0: +0x114 = 0;
   - +0x118 = 0 in every case.
3. With fade set, fades the intro clip over 750 ms if it is playing.
4. Sets +0x4c4 = +0x4c8 = 1.
5. Sets word +0x14 = 0 on every item, with no body test.

**+0x114 / +0x118 are not the activity's own.** They are the score manager's elapsed time and
accumulator (+0x50 + 0xc4 / 0xc8). On S196's image they equal the manager's words exactly. So ending an
unseen intro jumps the countdown to 6 s, the end of the intro. Ending it once it has been seen starts the
countdown over.

**What the items are.** On S196's sx01 image the scene list holds one item. Its body is the human
rider's bike chassis (bike +0x7a4). So the zero-time arming is the start hold that S290 is looking for:
the bike may move only in height. The warning window lets it go 1.5 s before the clip's drop time. On
this image that is 3 s before the drop (warning at 6.48, length 9.48).

**AllAI grid** (`./kit fn 0x2fa520`): no shuffle. In its place come race_tip_find("RacingOverlays") and
hud_overlay_mode(overlay, 6, 0). The vehicle/rider resets over +0x14 are skipped too, along with their
interface-id lookups. They live only in the shuffled branch. The placements and places are the same as
in the shuffled path.

## How it was proved

`./kit check -- -p game-sim --test race -- start_hold start_timeline all_ai`: 3 pass. A mutant of the
arming's turn freedoms failed proof 18, so the oracle does run here.

- **18, the_start_timeline_matches_the_countdown_listener**: the listener itself, 20,000 generated cases.
  - Windows as a reset leaves them (with intro or without), each now and then any word or a stray one.
  - Clocks: 0, −0, denormals, each edge and 6, a ulp either side, edge + 1.5, edge + 1.55, stray, or
    anywhere from −1 to 13.
  - 0–5 actors: others' class, items with no body, items with a body holding any freedoms and words.
  - Viewport modes 0–4, intro seen 0–3, ended or not, the intro clip playing or not, the camera flag both
    ways, the accumulator any word.
  - Compared against `start_timeline`:
    - the activity's words, the countdown's elapsed and accumulator, v0;
    - every item's six freedoms and four words;
    - the clip played (which one), the fade (clip, 750) and the number of overlays shown.
  - All five step kinds occur. The intro's end coincides with the arming 100+ times, jumps to 6 s 300+
    times and restarts at 0 300+ times.
- **19, the_grid_with_all_ai_set_keeps_the_lanes**: the grid slot with AllAI at 1, 2, 0xffffffff or
  0x80000000, over 2,000 starts. The vehicle list is non-empty, and list_nth / interface_id are hooked
  to count.
  - Compared: the lanes (identity) and the port's lanes, every ground query and placement, the card
    places.
  - No draw by either side, exactly one hud_overlay_mode(…, 6), zero resets.
- **20, the_sx01_image_holds_the_start_hold_as_armed**: S196's RAM image (`context/states/s196-sx01.ee.bin`,
  rebuilt here from s2's slot 21 save with `./kit state ram`), 3.59 s into the countdown, before the lights,
  intro on.
  - The class bit is looked up by name (already registered) and every scene actor is cast natively: one
    item, whose body is the rider's chassis.
  - Its linear freedoms and words are what `start_timeline(0)` leaves.
  - The image's clock is in no window (`start_timeline` returns a default step).

## Not verified / not 1:1

- **The bike's turn freedoms.** On the image the rider's chassis reads ang_free 1,1,0, but the arming
  writes 1,0,1. The oracle proves the arming writes 1,0,1, so something else rewrites them between the
  countdown's zero and 3.59 s. That writer was not looked for (no ELF scan, no watch): it is outside the
  listener. Proof 20 asserts the image's 1,1,0 as found, not as ported.
- **The cadence is not ported.** Only the listener is; the score manager's calls to it (interval and
  accumulator) are not. `race::Countdown` has no accumulator, and adding one moves the snapshot/hash
  wire. `start_timeline` is not called from `Sim::race` yet, and its `StartStep` (countdown jump, rider
  hold) is applied nowhere. Filed as S304; S290 is noted.
- **The items in the port are a stand-in.** `StartItem` is a stand-in for the interface actors, and the
  four words' meaning (what reads them) was not traced. The image shows only that, with one rider, the
  item's body is that rider's chassis. Which actors are items with several riders, and with AI riders, is
  not proven.
- **The drop's item writes are not ported.** race_drop sets words +0xc = 1 and +0x10 = 0 on items with a
  body, but `Race::drop` does not do this. It is outside this task's stretch and unproven.
- **Presentation is not ported.** That covers the overlays shown, the camera stops in the intro's end,
  the start call clip and the intro's fade. The oracle checks they happen exactly when `StartStep` says
  so, but nothing plays them.
- **Some code is unproven inputs.** The intro's end is only proven as the timeline calls it (fade = 1).
  Its other callers, if any (an intro skip?), were not looked for.
- **No live capture.** The timeline is proven by the oracle and one image only. No capture of a start
  passes through the windows.
- **The AllAI path's HUD call** (race_tip_find, hud_overlay_mode) is hooked, not ported (presentation).

## Next

S304 runs the timeline on `Sim::race`. S290 applies the hold to the riders' chassis.
