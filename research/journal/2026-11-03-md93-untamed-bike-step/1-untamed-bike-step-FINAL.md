# 1 — untamed bike step (FINAL)

Task MD93. 2026-11-03.

## What I did

- **The split.** The brief asks for six ported functions with oracles and two classifications. I did the
  three smallest, which also have the clearest place in the port: the preload, the wreck entry and the
  put-back. The rest is filed with `plan add MD93`:
  - MD93a: backflip
  - MD93b: spin in the air
  - MD93c: vehicle update
  - MD93d: classify the air state and the no-hander
  - MD93e (low): a control for the analogue preload
- **`Handling::Untamed`** is in `crates/game-sim/src/handling.rs`. Its word is "untamed", its label
  "Untamed", and it is in `Handling::ALL`; the default is still `Original`.
  - The shell (`main.rs` `untamed_handling`) reads the pack's files exactly as for MX vs. ATV
    (`mxvatv_phy_ini` sets `MxVsAtv`), then turns the `Phy` to `Untamed` when `--handling untamed` asked for it.
  - `ride_handling_pack` and `lobby::room_handling` take either handling.
  - A room carries only a pack id, so online an Untamed host's room rides as MX vs. ATV's (`main.rs:3174`
    untouched). The help text says so.
- **`ride.rs`** makes the `mxvatv` state for any handling other than `Original`, so Untamed rides get all
  of MX vs. ATV's ported steps. At the preload it calls `untamed::preload` on Untamed, with the axis
  `None`: no control is bound to it yet (MD93e).
- **The ports**, in `crates/game-sim/src/handling/untamed.rs`:
  - `preload`
  - `wreck_entry`, with `rand` and the calls as closures
  - `put_back`, with the ground query as a closure that moves the position and answers the normal
  - Both of the last two report their calls as `untamed::Call`.
- **Context.** I copied `context/mxvatv-ps2/state04.ee.bin` from s3 into this slot's context (not
  committed), so `mxvatv_step`/`mxvatv_air` really ran here instead of skipping.

## What the original does

Read off `./kit dis` with the untamed kit (`u.sh dis`; a copy of s1's untamed kit.toml). The field map is
in research/oracle.txt `# MD93`.

- **Preload.** It starts as SLUS-21104's: a firing preload counts down and pushes its force into the
  body. Then comes a new block.
  - If an action is bound to the analogue preload (the bike's word set from `0x00117c10(0x53) != -1`), the
    preload was not firing on entry, the bike is on the lip, and the axis (pad word 0xe5) is over 0.001:
    - it fires with `force.y = map_clamp(axis, 0, 1, 0, MaxPreload)` (the same adda/madd map, an
      empty-range branch that cannot be taken),
    - the timer goes to `PreloadTime`, and armed is cleared.
  - Then, only if the preload was not firing on entry, comes SLUS-21104's lip and stick path, unchanged.
    So after an axis fire, a stick held up arms again in the same call and zeroes the timer (path "axis
    fired, then armed" in the test).
  - The cue changed. With the latch clear and the preload firing, it sets the latch and posts (8, 14, tag,
    0). Otherwise a timer at or under 0 clears the latch. SLUS-21104 posts on timer > 0, not on firing.
- **Wreck entry** (vt +0x368). The same order of calls as SLUS-21104's, on Untamed's slots.
  - Reason 13's three `rand() × 2⁻³¹` go to the stack and then into the body's `+0x1d0` as one quadword
    whose `w` is 0. SLUS-21104 stores each lane after its draw and leaves `w`.
  - Then it clears the preload's firing and ready, zeroes `+0xdd8`, sets `+0x1714`, runs the base entry,
    sets `+0x17c4 = reason != 0`, and counts `+0x708`.
  - The base entry (0x004cbc60) is hooked, not ported. It differs from SLUS-21104's by a new call for a
    player.
- **Put back.** The position comes from `+0xd30`. The driver gets the three respawn words only when the
  first two are both non-zero.
  - With snap and a ground, the ground query moves the position. Then `position.xyz += normal.xyz × 0.3`
    (`vmulx` then `vadd.xyz`: w kept), the orientation comes from `+0xd40`, and **up = the normal**,
    quadword and all.
  - Otherwise orientation and up come from `+0xd40` and `+0xd50`.
  - Then the settle call. keep: hold ← saved, vt +0x3a0; else hold 0, vt +0x39c.
  - Then the attachment's put-back if `+0x10cc` is set, and post (8, 0x15, tag, 0).
- **"Every stunt-id bound +8"** has nothing to do in these three, because none reads the stunt id.

## How it was proved

`crates/game-sim/tests/untamed_reworked.rs`, off `context/untamed/ram/nat_or.bin` (SLUS-21701 Free Ride
eeMemory). The harness is `tests/untamed_step.rs`'s, plus the body's vtable so the force call runs.

- `untamed_preload_is_the_originals`: 20,000 generated states, the force call running unhooked and the
  event post hooked.
  - All 10 paths are covered at least 100 times each: firing, fired out, off the lip, axis fired, axis
    fired then armed, armed, fired, fired with equal times, holding, waiting.
  - At least 1000 cues were posted.
  - The axis is bound in ⅔ of the states, with values exactly 0.001, 0, negative, over 1, and in between.
- `untamed_wreck_entry_is_the_originals`: 10,000 entries (≥ 1000 each of reason 13, reason 0, and another
  reason).
  - Every vt call and the base entry are hooked, with their arguments checked.
  - `rand` is hooked with generated draws including `i32::MAX`, `i32::MIN` and 0. The draws are logged in
    the same list as the calls, so their place in the order is checked too.
- `untamed_put_back_is_the_originals`: 10,000 states (≥ 500 each of snapped/saved × respawn handed or
  not, kept/not kept, attachment/none).
  - The ground query is hooked to write a generated position and normal.
  - Settle, both ray slots, the attachment and the post are hooked and compared in order.
- In each test the original wrote nothing outside the bike, its body, the driver and the stack. Every
  word of those regions was compared by bits.
- **Hand mutations.** These went red:
  - cue on timer > 0 (SLUS-21104's rule)
  - axis threshold 0
  - stick path gated on firing-now instead of firing-on-entry
  - spin `w` kept
  - no 0.3 lift
  - respawn on the first word only

  Writing the lift's `w` too stayed green. That is right, since `vadd.xyz` never writes `w`; I dropped the
  merge that guarded it.
- **Nothing else moved:**
  - `untamed_step`, `mxvatv_step`, `mxvatv_air` and `mxvatv_ride` pass, with the images present (no SKIPPED
    lines).
  - `game-sim`'s `replay` (121) and `hash` (16) suites pass. The Original and MX vs. ATV paths are the same
    code as before (`!= Original` is `== MxVsAtv` over the old two variants).
  - `cargo check -p openmx --tests` builds, and `openmx`'s handling and settings tests pass (the menu's 8
    settings tests, the lobby's handling test).
  - `./kit audit` shows only the 4 leaks already on main.

## Not verified / not 1:1

- **No ride was watched on Untamed's handling**, and no capture of the original's preload or put-back
  was compared. The proofs are function oracles on generated state.
- **The analogue preload is never fed.** `ride.rs` passes `None` (MD93e). Which pad axis word 0xe5 is
  and which control action 0x53 names were not traced past the input reader (0x004fb7d0).
- **The wreck entry and put-back are not wired into the ride.** SLUS-21104's twins are not either: the
  port's wreck and put-back are its own (`state.rs`, `handling::step::machine`). Their callees are hooked
  and named by what they touch, not ported:
  - the base entry 0x004cbc60, including its new player call 0x001c3d70
  - vt +0x4a0/+0x4a8/+0x398/+0x484
  - the ground query, the settle, the ray slots, the attachment's put-back
- What `+0xdd8`/`+0xddc` ("hold") and `+0x1714`, `+0x17c4` mean was not traced.
- **MD81a's ratios were not rechecked.** These three functions were read whole, so a change MD81a's
  masked diff missed elsewhere in them would still have shown in the oracle.
- **The online room** rides an Untamed host as MX vs. ATV's (no handshake word for it).

## Next

MD93a (backflip) first: it is in the ride already as MX vs. ATV's and has the stunt-id bound (40 → 48).
Wire it in `ride.rs` next to `untamed::preload`'s match arm.
