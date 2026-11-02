# 1 — the nose and the roof, recorded (FINAL)

Task S191. 2026-11-01.

## What I did

- **Made three states in copy 2, at full speed.** All come from S129's across lineage, so the bike has
  ridden since its last ghost gather and its nearby search holds the cars (`bike_search_holds_cars`
  is true for all three; S170's requirement).
  - **headon** (slot 17): S170's head-on state (slot 16) run on with the new `research/s191/run_to.py`
    to frame 18714. The engine's nose meets the bike, standing on the rails about 4° off their line,
    61 frames later.
  - **stand** (slot 19): S170's from-behind state (slot 15) run on to frame 16589. The nose arrives
    36° off the bike's tail, 54 frames later.
  - **roof** (slot 18): `research/s146/roof.py <pid> 10 0 3 18 9 8`. The bike is put 8 ft over the
    tenth path car's roof with the wagon's velocity (55.13 ft/s) and saved 3 frames into the fall, at
    frame 16382. `roof.py` gained an optional height argument (OVER) for this; its default is still
    S146's 2.5 ft.
- **Recorded each at a quarter speed** with `watch/s129-train.toml`: headon 200 rows (frames
  18716–18915), stand 250 (16592–16841), roof 240 (16385–16624). None has a torn read or a gap. The RAM
  images are `context/states/s191-{headon,stand,roof}.eeMemory.bin` (`./kit state ram <slot>`).
- **No pad.** `/dev/uinput` was root's again after the reboot, so `./kit pad serve` failed and every
  run and capture had nothing held. The bike rolled about a foot in 2,300 frames on the flat, so I went
  ahead without brakes. Whatever the pad bytes were, they are in the rows.
- **Replayed them** in `tests/replay.rs`'s new `the_train_wrecks_and_carries_the_bike_as_the_original_does`.
  It is S129's fixture, with three differences:
  - wrecks are on;
  - the disc's body rays are on the rider (S190's `free06_bike_col`);
  - the train clock is watched.
- **Fixed the hanging rear's contact velocity** (`ride.rs` `share_load`). Details below.

## What the original does

- **headon:** the nose throws the rider over the bars (reason 1) on tick 60, frame 18775. That is
  the frame of S170's log. The chassis then stays frozen until the put-back, around row 155.
- **stand:** the nose meets the bike on tick 53 and carries it to about 56 ft/s. The bike slides and
  slows, and wrecks on tick 93 with a head hit (6). The put-back is around row 170.
- **roof:** the bike lands on the roof on tick 18 and rides it. With no brakes it slides back along
  the wagon at about 49 ft/s against the wagon's 55. It falls off the end between rows 110 and 150,
  and nose-dives (8) on tick 158.

## The fix: where a hanging wheel measures what it stands on

`wheel_contact_velocity` (0x0025b710) measures the other body (the vehicle, +0x1f4) once per
substep. It keeps the measure at +0x1b0/+0x1c0 and sets +0x1a0, which `wheel_clear` (0x0025b200)
empties at the top of each substep. Its three callers are:

- 0x0025bf84 and 0x0025c188, both in the suspension step (0x0025bc70). Both are inside the branch
  where the compression is above zero.
- 0x0025c5dc, in the spin step (0x0025c530): the `+0xc4` branch, where a hanging wheel borrows the
  other wheel's load. It asks right after copying the other wheel's +0x1f0/+0x1f4.

So a wheel with no contact of its own does not ask in the suspension step. Its first ask is the share
branch's, and that measures what the other wheel's copied hit objects name. The port measured both
wheels up front (`under`), so the hanging rear measured nothing, because its own +0x1f4 was empty. Now
`share_load` measures after the copy, and returns that measure for the rest of the wheel's
integration.

The roof landing is the case that shows it: front first onto the wagon, rear hanging. The original's
rear contact velocity was wagon-relative (x −2.22), while the port's was world (52.97). Before the
fix roof parted on the landing (tick 18), and stand parted on tick 55. After it:

| ride | parts on | the original's wreck | the port's wreck |
| --- | --- | --- | --- |
| headon | 60 (the wreck's tick; origin by an ulp) | 60, over the bars (1) | 60, over the bars (1) |
| stand | 65 | 93, head hit (6) | 65, head hit (6) |
| roof | 158 (the wreck's tick) | 158, nose-dive (8) | 158, head hit (6) |

## What still differs: a lone head ray on a car

Stand on tick 65 and roof on tick 158 part the same way. I put a temporary print on the wreck test's
contact list (`rider.rs`, since removed). On the tick before each, the bike's body rays had no entries.
On the tick itself the list held a single entry, the head ray's (reach 6.674), on a car. S145 reads an
entry on a car as scenery, so the port wrecks with a head hit. In stand the original rides on for 28
more ticks. In roof the original's nose-dive verdict stands, and the hit clause, which is asked second,
would have replaced it. So on those ticks the original's list had no head entry. The chassis, wheels
and train clock agree bit for bit up to that tick, so the difference is in the head ray's test against
the car's mesh (or in which car meshes the record's search holds then), not in where the bike is. The
capture holds no contact list, so I could not take it further. This is filed as S191a: record the
original's collision record (count, entries, `on_body`) for these ticks and compare it.

## How it was proved

- `cargo test --release -p game-sim --test replay the_train_wrecks_and_carries`: passes in s2 with the
  three captures and images. It asserts each ride's agreement tick, the capture's wreck and the port's
  wreck (`S191_RIDES`).
- `cargo test --release -p game-sim --test replay --test traffic --test bike_col`: 108 + 16 + 4 passed
  (1 ignored), S129/S146/S162/S190 train replays among them, with the fix.
- `./kit check`: TESTS PASSED, 3646 tests.
- Before the fix the same S191 test parted roof on 18 and stand on 55, with port wrecks on 176 and 62.

## Not verified / not 1:1

- **Stand's and roof's wreck verdicts are the port's, pinned, not the original's** (see above, S191a).
- **The original's contact list was never read.** The share-branch fix is read off the code (the
  callers of 0x0025b710 and `wheel_clear`'s place in the substep loop), and its effect is checked
  through the chassis and wheel lanes, not through +0x1a0/+0x1b0/+0x1c0 themselves, which the capture
  does not hold.
- **The front's measure is unchanged.** A hanging front, with the rear down, never takes the share
  branch, so it stays as it was.
- **No brakes, no pad**, in all three rides (above). A braked nose or a braked roof ride is not
  recorded.
- **roof's bike lies about 54° across the wagon**, the heading across's state had. It is not along
  the wagon. A bike ridden onto a roof along the train is not recorded.
- **S190's across replay runs with wrecks off** (`acceptance_start` turns them off). So its "the port
  does not wreck" assert passes whatever the port would do. The original does not wreck there, and
  this replay, with wrecks on, did not check across.
- **The captures, images and states are only in s2's context/** and copy 2's save slots 17–19. Elsewhere
  the test skips and reads as passing.
- **Not ridden in the shell.** The fix is on `ride.rs`'s substep, which the shell steps too.

## Next

S191a (filed): the lone head ray on a car.
