# 1 — body rays static props (FINAL)

Task S152. 2026-10-28.

## What I did

- **Measured what the brief asked for first** (the "follow-ups may be fixed already" habit). S137's
  journal filed this when the bike's eleven body rays met only the moving cars and the ride's
  centre-of-mass velocity ray and head ray stood in for them against everything else. Since then:
  S138 (`rider_contact::scenery`) runs the eleven body rays once a tick against **every** prop and
  plant the world hands over (`World::wooded_near`), cars among them, and resolves their contacts
  through the bump's list; S125 replaced the velocity ray with the chassis' sweep back; S145 named the
  moving props' movers; S157 put the body rays' scenery entries on the wreck test's list. So the search
  over every prop, and its re-pinning, had already landed.
- **What was still a stand-in**: the wreck test (`rider.rs`, the hit clause's list) still swept the
  port's head ray (`ride::hits_among` → `World::solid` + `World::groups`, i.e. the same props and
  trees) at the frame's *end*, on top of the body rays' entries. That was the last place the static
  props met the stand-in instead of the body rays. Effects: a head reaching a prop wrecked one tick
  before the body rays (which look from the tick's start, as the original's search does before the bike
  update) could list it, and a head ray both listed was counted twice (S157 noted this).
- **Change**: where the body rays searched the world this tick (`scenery` returned `Some`, i.e. the
  bike has collision files and the world hands its props/plants over), the wreck test's list is ground
  entries (P147), the head ray against the other riders only (new `ride::hits_riders`, the rider half
  of `hits_among`), the car search's entries (S137), then the body rays' scenery entries. Where no
  search ran (no `BikeCol`, or a world that only answers `solid`, e.g. test yards and fixtures), the old
  `hits_among` sweep stays as the stand-in. `mover_contact`'s module doc updated to say the static
  props are S138's search's.
- Nothing from opentennis: no counterpart there (tennis has no body-ray contacts).

## What the original does

The bike's collision object searches once a tick, before the bikes update, and the wreck test's last
clause reads that object's one contact list in order (S138, S157 journals; nothing new decompiled here).
No head ray is swept after the frame in the original: that sweep was the port's own (`ride::hits`'s
doc says so).

## How it was proved

- `crates/game-sim/tests/head_on_props.rs` (new): the disc's race125p bike with its body rays thrown at
  20/35/50 ft/s on a flat yard under a ceiling placed between the head ray's end and every other
  ray's (the head ray is near-vertical, so a ceiling is what it crosses). It wrecks reason 6 on the very
  tick whose search, from the tick's start, first lists the head ray (17, 13, 10). **Red before**: with
  the old composition the 20 ft/s throw wrecks on tick 16, before the body rays could list it.
- S138's replay of the original's own wall and nose hits from save states
  (`replay.rs::walls_and_nose_landings_wreck_as_the_original_does`, run in s1 with `--nocapture`):
  all 8 still agree with the original's verdict and tick.
- `./kit check -p game-sim`: 817 passed unchanged (no pin moved). `-p openmx --no-fail-fast`: train.rs and
  every world ride pass; the only reds were lobby LAN drop-ins, pace's 60 fps limit and fixed_rate under
  load average 56 (known load flakes, see `gate.toml`'s [flaky]).

## Not verified / not 1:1

- **No reason-6 wreck on a static prop has a capture of the original.** S138's eight hits are all
  reason 1 (g peak) or ride-outs; the head-on-props test is a synthetic yard proving which tick the
  port's own list wrecks on, not the original's tick. Reason: no capture of a head into scenery
  exists besides S37's recording2 (reason 6 on the ground-side, not replayed here).
- **S157's 2p world-contact replay skips in every slot**: `context/states/c10/c10-130413.ram` is missing
  in s1, s2, s3, s5, merge and the main checkout. Its scenery wreck (P2's reason 6 at 130417/8) was not
  re-checked against this change.
- **The wreck list's order is still not the record's**: rider (head-ray sphere test) entries come before
  the car search's and the body rays' scenery entries; the original interleaves by its search's walk.
  Matters only when an actor entry precedes a scenery one (as S157 noted). Unchanged here.
- **A car ray can still be listed twice** on the wreck list (once from the car search, once from the
  scenery search's `found`, which the bump list dedups but `on_scenery` does not). Harmless for the
  verdict (both scenery), not the original's count.
- **Rider vs rider is still the port's sphere about the hub** (`Ride::rival`), not the original's body
  rays against the other bike's frame collidable. Out of scope.
- **The car search (S137) is not folded into S138's**: its entries keep the scenery response (no mover)
  and its `inside` box test is the port's own. Whether path cars are movers is S151's question.
- **Bikes without collision files, and worlds that only answer `solid`**, still use the frame-end head
  sweep as before (no body rays to read).

## Next

Nothing for S152.
