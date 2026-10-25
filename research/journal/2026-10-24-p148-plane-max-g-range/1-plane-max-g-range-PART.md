# 1 — plane max g range (FINAL)

Task P148. 2026-10-24.

## What I did

- `MAX_G` (crates/game-sim/src/modifiers.rs) is now `(3, 15)`, as the user set it in the brief. The knob
  still steps a g at a time round its ring (Off, 3 … 15, Off). That makes 14 settings. The percentage
  sliders have 50, so I kept the 1 g step and did not add a coarser step above 9 g. Parse refuses
  anything outside 3–15 except Off (100), and `set` clamps to 15. The menu page (menu_pages.rs) reads
  `least()`/`most()`, so it needed no change: the meter fills over 3–15 and Off is a full meter.
- **The measurement.** I flew the fastest plane the knobs make: 500% Biplane top speed and 500%
  acceleration. Its level top speed is 784 ft/s. From level cruise there, with the limit Off, a full
  pull reaches **47.9 g** with the File scheme (the file's pitch-rate autopilot) and **87.0 g** with
  the shell's default New scheme. So 15 g still bites, with plenty of room. For comparison, Off at
  400% top speed pulls 24.6 g (File) and 58.2 g (New), and at 300% it pulls 18.1 g and 42.6 g.
- **P137's hold overshot above 13 g.** At 784 ft/s the File pull reached 15.238 g against a 15 g limit,
  14.14 against 14, and the New knife-edge pull 15.125 against 15. The trace shows an onset ring of
  11.4 → 9.9 → 15.2 g before the pull settles at 14.6 g. At 9 g the rise is smooth. The angles are
  about 0.011 rad, far from the 0.209 rad stall break, so the cause is not stall nonlinearity. My
  explanation is that the wing starts further short of the limit's angle, so the elevator stays full
  for longer and the swing after it grows with the limit. The fix is in biplane.rs `g_limit`: above
  9 g (`SWING_FROM`), the aim drops by 0.004 for each g over (`SWING`), so 15 g aims at 0.956 instead
  of 0.98. Up to 9 g the aim is exactly P137's `AIM` (`AIM - SWING * 0`). P137's tests at 3–9 g pass
  unchanged, including the bit-for-bit `the_stock_plane_is_never_held_at_9g`.
- After the fix, at 500% (784 ft/s) and 15 g: File pull 14.921, File push 14.389, New pull 14.454, New
  push 14.491, New knife-edge 14.885. At 300% and 400% every limit from 10 to 15 g stays under too.
- Tests (crates/game-sim/tests/plane_max_g.rs): `the_knob_steps_and_keeps` covers the new range, walks
  the whole ring, round-trips 15 g through the text, accepts `plane_max_g=12`, and refuses 16 and 2.
  The new `the_top_limit_holds_the_fastest_plane` covers 10–15 g at 500%, 400% and 300% top speed
  (500% acceleration), with pull, push and knife-edge pull, under both schemes. Every run must stay
  under its limit. At 500% each must also stay under the Off pull, a steady pull must reach ≥ 92% of
  the limit, and Off must pull > 22.5 g.
- I found nothing in opentennis to take from. This is a knob range and a mod-only limiter.

## What the original does

Nothing. The original has no g limit, and Off (the default) never enters the limiter
(`Flight::max_g == 0`), so the original's ride is unchanged. `off_flies_the_original` still holds.

## How it was proved

By simulation only: `plane_max_g.rs` on the disc's `biplane.phy`, 30 Hz frames over an endless flat
floor, every frame of 6 s runs. No emulator was needed, because the limiter is an addition and has
no counterpart in the original to compare against.

## Not verified / not 1:1

- 15 g is held on the File pull at 500% with only 0.5% to spare (14.921 g). The margin per g
  (`SWING`) was tuned on these test manoeuvres: a full pull, push and knife-edge pull from level at top
  speed. A different entry, such as a pull out of a dive above the plane's level top speed, has not
  been flown and might swing further. The reason is that the tests start from level cruise, as P137's do.
- The g counted is the last substep's lift at frame end, as in P137's tests. A peak between substeps
  is not checked.
- The File knife-edge pull reaches only about 3–4 g at every limit, because the file's autopilot holds
  the pull there. That is P137's behaviour, not new, so I applied the "within 92%" check to the plain
  pull only.
- The menu meter for 3–15 g was not looked at on screen. It is computed from `least()`/`most()`, which
  the code reads generically.

## Next

The work is done, and every P148 test passes (`./kit check -- -p game-sim --test plane_max_g`: 7 passed).
`./kit done` refused anyway, because two bike replay tests fail on bare HEAD bb13cfa6. I proved that in a
scratch worktree, so P148 did not cause them. They are filed as S119:
`the_captured_backwards_turns_never_wreck` (flat agrees through 213, not 330) and
`the_slip_cases_replay_from_the_save_state` (turn parts at 75). Once S119 lands, rerun
`./kit done P148 "..."`.
