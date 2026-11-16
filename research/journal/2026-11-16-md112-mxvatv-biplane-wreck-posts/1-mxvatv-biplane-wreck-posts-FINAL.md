# 1 — mxvatv biplane wreck posts (FINAL)

Task MD112. 2026-11-16.

## What I did

- `flight::Posts` on `Flight`: an MX vs. ATV plane counts the messages it posts. `wreck` (11) on every
  wreck entry, `latch` (12) when the thrown plane comes down (`biplane::Crashed::latched`, new), and
  `unwreck` (13) on every recovery (the timed one, the reset chord, a reset). Counters, not flags, so the
  reset chord and resets (which run before the frame) are never cleared by it. `put_back` keeps the
  plane's own count, so a rollback does not count a post twice. Not hashed and not on the wire.
- `Flight::wreck` under `mxvatv`: the inverse inertia is MXVehicle's, `1 / (WreckInertia × Inertia)` per
  lane (`biplane::wreck_inertia_mxvatv`, `inverse_inertia_mxvatv`). MX Unleashed's plane divides the
  inverse by `WreckInertia` instead, which can be an ulp off.
- The damping gate (`+0xbbc` not 0 or 1): `+0xbbc` is the machine kind, and a biplane is always 2
  (the camera defaults' switch: 1 ATV, 2 Biplane, …). So every plane takes the gate, and "damps always"
  was already right. The port has no kind field for the plane. The test still runs kinds 0, 1 and
  random ones against the original, to pin the gate down.
- Shell: the wreck rumble (`pad::WreckRumble::frame_posted`, main.rs) rumbles an MX vs. ATV plane's pad
  on each new wreck entry. Before, it rumbled only when the crash word went up, so it missed a crashed
  plane being thrown back from the edge (reason 1 → 13, a second entry). The rumble strength is
  `Rumble::WRECK` (200, 1 s), which is the rumbler call's (r, 0, 200, 1.0).
- Filed MD181: the MX vs. ATV plane is built with MX Unleashed's inertia (C7's), not biplane3's.
- Nothing taken from opentennis: it has no wreck posts or vehicle rumble like this.

## What the original does

- The plane's wreck entry runs MXVehicle's entry first. That sets the reason, the clock to 0 and the
  latch to 0. It asks the rumble switch when the vehicle state is the player's. For kind not 0/1, it sets
  both body dampings to `WreckDamping` and the inertia to `WreckInertia × Inertia` (w 1). It posts 11.
  Then the plane's own part: rates × 4, the wreck gravity, the throttle and the engine word to 0, and the
  wrecked look. `./kit fn 0x004c0cc0 -p atv`, `./kit fn 0x004a83d0 -p atv`.
- The recovery's MXVehicle part sets the dampings to 1 and the inertia back to its own, then posts 13
  (`./kit dis -p atv 0x004a8580 --len 0x24`).
- The crash test posts 12 when the thrown plane's latch is set (MD101c).
- Who hears the posts: the camera controller, the HUD and others (research/oracle.txt # F31b5l). No sound
  listener is known for 11/12/13.

## How it was proved

- Oracle, `crates/game-sim/tests/mxvatv_biplane_wreck.rs`
  `the_mxvatv_wreck_entry_posts_and_damps_as_the_original`: 2000 generated cases of the real wreck entry
  on biplane3.ee.bin. Inputs: kinds 0, 1, 2 and random; inertia and `WreckInertia` lanes that are
  zero, negative or large; random dampings, spins and `WreckGravity`. Checked bit for bit: the posts
  ([11]), one ask of the rumble switch, the dampings, the inverse inertia
  (`wreck_inertia_mxvatv`), the rates × 4 and the gravity. Then MXVehicle's recovery on the same plane:
  posts [13], dampings 1, inverse inertia `inverse_inertia_mxvatv(Inertia)`.
- The MD101c crash-test oracle (10 000 cases) now also checks `Crashed::latched` against the original's
  12 post.
- `crates/openmx/tests/rumble.rs` `an_mxvatv_plane_rumbles_on_every_wreck_entry`: the shell's rumble
  rule over a sequence of entry counts.

## Not verified / not 1:1

- Not checked on a capture or a live run: that a flown MX vs. ATV plane's inertia after a wreck matches.
  The port builds the plane with MX Unleashed's inertia (C7's `INERTIA`). biplane3's is different
  (5199.0327, 11892.645, 7951.833). So the wreck applies the right rule to the wrong starting inertia
  until MD181 lands.
- The posts do nothing in the shell except the rumble. In the original, 11/12/13 drive the camera
  controller and HUD listeners. The port's plane camera does not hear them. Cameras are "match by eye",
  so no task was filed.
- The rumble's own on/off is the shell's Vibration setting, not a ported `atv_rumble_on`. The test only
  proves the original asks the switch once per entry; the switch is hooked to say off.
- The plane's wrecked look (`+0x12e8` vt `+0x94`) and the engine word were left alone. They are outside
  this task and were not compared.
- The first frame the shell sees an MX vs. ATV plane's count, it does not rumble even if a wreck
  happened that frame. It has no earlier count to compare against.

## Next

Nothing; MD181 is the follow-up.
