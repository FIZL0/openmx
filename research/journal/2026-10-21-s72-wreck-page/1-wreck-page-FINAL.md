# 1 — wreck page (FINAL)

Task S72. 2026-10-21.

## What I did

- Decoded what `Reset::Page` sets off in the original. In ride_reset_request (0x0027b9f0), a page
  together with a screen registers the bike (+0x1340) as a listener on its screen's viewport fader
  (0x001c84b0). It then starts that fader's fade: 100 ms to black, 100 ms back in (0x001ffe80 →
  0x001c8400 → 0x001c8e80).
  - The machine does not put the bike back. The fader's draw (0x001c8f50) does: when alpha reaches
    255 it sets "held", calls every listener's vt +8 (the bike's listener vtable 0x00455128 → 0x00281c10
    → player_respawn) and starts the fade back in.
  - The fade-in's draw at alpha 0 clears "held" and drops the listeners.
  - The machine's "page up" query reads held || fading out.
- Captured it live with `research/s72_page.py` + `watch/s72-page.toml`: copy 5, slot 9 = free09's
  state 7, S59's edge ride, north/east/west. Each capture holds the state's own reason-6 wreck and
  the closing cannonball, and west also has a reason-2 wreck in between. That makes seven pages, all
  asked for. The captures are in context/capture/s72/ and are not committed.
- Ported the page as `crates/game-sim/src/wreck_page.rs`: a `Page` that counts the fader's updates,
  three out and three in.
- Wired it into `rider.rs`:
  - The shell's rider is now a local human with a screen (`Owner { screen: true }`).
  - A `Reset::Page` raises the page. The page then updates after the bike each tick (step 7).
  - The update that reaches black puts the bike back, with the same put-back as the machine's own:
    place, unwreck, crash/ragdoll/tricks cleared, ghost started.
  - The machine's `pages()` reads `page.up()`.
- Added the page to the Rider's snapshot (wire) and to its hash walk. The walk names the page only
  while one is up, so the hashes of riders with no page do not move. The size pin went 9744 → 9752,
  and the riders' wire vector to 12931 bytes / 0x0ba8583e79f78245 (2 bytes a rider).

## What the original does (measured)

- The fader updates after the bike in the same tick, so the request's tick is the fade's first
  update.
- The fader's dt is whole milliseconds of real frame time: 34/33/33 or 34/33/34. Three updates
  always reach t ≥ 0.99999994, which draws as 255. Two updates never do.
- The respawn therefore comes in the draw at the end of tick R+2, and the bike first rides on tick
  R+3. The clock left on the bike is clock_R + dt + dt (5.0995 for the three cannonballs, 2.0947 or
  2.0998 for the others).
- The fade back in is three more updates (elapsed 0.1 or 0.101). After that, held is off and the
  listener count is 0.
- Rows are read at any point in the tick. They show up between the bike's update and the fader's
  (the request row with elapsed 0, west row 258), after the draw but before the next bike update
  (ghost 3.0 and fade-in elapsed 0), and once inside the black draw itself (west's first page: the
  bike already back, "in" not yet set).

## How it was proved

- `tests/wreck_page.rs`:
  - `the_originals_page_puts_back_on_its_third_update`, over the seven live pages:
    - The fade's parameters: 100 ms out, then 100 ms in, two listeners.
    - Every row fading out has one more fader update than ticks since the request, or exactly as
      many on a mid-tick read.
    - Every held row is fading in, with as many updates as the ghost's ticks since the put-back, or
      one fewer.
    - The fade in ends after three updates, with the listeners gone.
    - The port's `state::Ride` + `Page`, started from the row before the request, asks on the
      request's tick, puts the bike back on the third update, and leaves the clock bit-equal to every
      row from the put-back to the end of the fade.
  - `three_updates_out_and_three_in`: the page alone.
  - `the_rider_is_put_back_by_the_page` (the wiring):
    - A screened rider is put back on frames 3, not 1 or 2. It is held for the 3 frames after, then
      let go. The put-back gives the riding speed limit and starts the ghost.
    - A rider with no screen, and the lost-driver reset (reason 14, no page), are put back on the
      request's frame with no page.
    - The chord during the fade gives the page's second put-back (see below).
- `tests/off_the_map.rs` `the_original_rides_off_each_edge`: S59's five captures now step the
  `Page` from the request, with the machine's `pages()` reading it. They are put back on its third
  update, with the clock's bits equal to the put-back row's.
- The full `./kit check` passes.

## Not verified / not 1:1

- **The chord during the fade out.** The original's listener stays on the fader after an L1+R1
  put-back, so its black draw respawns the bike a second time. This comes from the disassembly and
  was not captured. The port puts the bike back again, in place, with the ghost restarted. Where the
  original places it (player_respawn's own placement, perhaps the spawn) is not measured. Filed as
  S75.
- **Milliseconds are modelled as an update count.** The original adds whole ms of real frame time.
  The port counts three updates. They agree for any mix of 33 and 34 ms frames (always three), but a
  frame long enough to skip a tick (a slow frame, 50+ ms) would reach black in two updates in the
  original and still take three in the port. That was not seen and not modelled.
- **The black fade is not drawn by the shell.** It is ported as timing only. Filed as S74.
- **One page per rider.** The original's fader belongs to the screen (`which` = current screen !=
  the player's screen). Split-screen and two riders on one screen are not modelled. In netplay every
  rider is now "a local human with a screen" in the simulation, which is the same on every peer but
  not the original's remote-player owner.
- **The fader during a pause or the plane.** The page updates only when the rider's frame runs. The
  original's fader runs from the front end. A pause inside the 3 ticks is not modelled.
- **The screen's HUD flags** (vt +0xa0 on the no-page path, +0x144/+0x148) are not ported. They are
  presentation only.
- **The whole wreck between the request and the put-back** inherits S59's caveat: the port's wreck
  body is a stand-in, so only the machine, the clock and the timing are bit-proved. Positions are not.
- **The capture's oddities are accepted, not explained one by one:** the row reads inside the tick
  (above) are tolerated within one update.

## Next

Done. Follow-ups: S74 (the shell's fade), S75 (the chord during the fade out).
