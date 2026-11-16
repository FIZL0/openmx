# 1 — untamed race quad replay (FINAL)

Task MD168. 2026-11-16.

## What I did

- **New test file:** crates/game-sim/tests/untamed_race_quad.rs. It has four tests, and each skips (and says so) without the image, the captures or the pack.
- **The quad is made like the shell makes it.**
  - Files: the `untamed_atv` pack's `atv_Race250P.phy.bpb`, `250r_atv_chasis.psg`, `ATV_R_ChasisM.col` and `WreckATVm.col`.
  - Steps: `quad::Load::read_model`, `Ride::spawn`, then `Ride::rig`.
- **The image's state is laid over it** at Untamed's quad offsets (`u()` maps SLUS-21104's offsets to Untamed's):
  - **What is read:** the keys and words, body, wheels, engine, air state, scene nodes, the tail's surfaces and the hazard clock's words.
  - **Taken from the old reader:** this is tests/atv_quad/image.rs's `read_ride`, read at the new offsets.
  - **Switches:** `Tail::untamed` and `Scene::untamed` are on.
  - **Offset map** (SLUS-21104 → Untamed):

    | SLUS-21104 range | shift |
    |---|---|
    | `+0x2c..+0x413` | +0x30 |
    | `+0x49c..+0x713` | +0x24 |
    | `+0xb00..+0x12bf` | −0x10 |
    | (new in Untamed) | 0x20 B at `+0x12b0` |
    | `+0x12c0..+0x14c3` | +0x10 |
    | `+0x14c4..+0x1533` | +0x20 |
    | `+0x1654..+0x17bf` | +0x40 |

  - **Unchanged layouts:** body, wheels, engine and world keep SLUS-21104's inner layout. The scene nodes' words sit 0x30 further in from `+0x30`.
- **The world (`Rocksbury` in the test):**
  - `ground` calls Untamed's ground query in the interpreter on the image's terrain (the wheel's terrain pointer, `+0x1e0`).
  - The record byte the query writes is the wheel's surface (`wheel_surface`).
  - The area is the terrain's box (896 ft).
  - Nothing is solid.
  - Each row is one `Ride::step`, with the row's brake and gas (the `ctl` column's `z`/`w`) and stick.
- **The hazard states' RAM images:** context/untamed/md147/hazard-time.ee.bin and hazard-on.ee.bin are their `.p2s`' `eeMemory.bin`. The `.p2s` is a zip; I extracted it with python's zipfile. MD147 only kept spawn's.
- **Follow-ups filed:** MD168a (Untamed's put-back) and MD168b (steer past the take-off).

## What the original does

- **The pos column is not the position.** MD147's `pos` column is the body's *last* position (`+0x60`, the start of the last substep). Its `body` hex holds the position at `+0x70`. Comparing the `pos` column made the port look a quarter tick ahead. The body words match bit for bit.
- **The take-off knock.** On steer's take-off (rows 196–198) the original's quad gets a sideways knock in one tick: x velocity −4.05 → +2.36, with every wheel off. Row 198 is read mid-tick, with force (−36, −57, −58) in it. The bare world here does not do this.
- **Untamed's put-back (hazard-time).** The clock passes 1 on the 31st tick in the air, row 180. In the port the stuck time zeroes and the put-back runs on row 180. In the original the body still flies on row 180. Row 181 then has it standing still (last position = position) at (282.32, 6.69, 176.47). That is beside the quad's last-free spot (Untamed `+0xbf0` = (282.29, 7.20, 176.55), the row-150 position, the last on the ground), but lower.
- **Untamed's put-back (hazard-on).** The put-back target is the spawn's x/z, at y 1.29 (the spawn's body y is 0.79), one tick after the port's (row 31 against 30).

## How it was proved

Bit-exact replays against the captures. Every word of the port's rigid body (`handling::seed::body_words`) is compared with the row's `body` hex.
- `untamed_quad_replays_md147_steer`: steer.csv rows 0–196. Row 166 is skipped: it is a copy of row 165 (a late read).
- `untamed_quad_replays_md147_hazard_time_to_the_put_back`: hazard-time.csv rows 0–179, all well read.
  - The port's stuck time equals each row's from row 140. On rows where MD147 saw the column lag, it equals the port's from the tick before.
  - The clock is one tick from passing 1 after row 179.
- `untamed_quad_replays_md147_hazard_on_to_the_put_back`: hazard-on.csv rows 0–29. Row 14 is skipped as a copy. The clock matches from row 1.
- `the_untamed_quad_from_its_files_is_md147s_spawned_quad`: the files' quad against the image's.
  - Bit for bit: params, joystick, clutch keys, air keys, the engine (tables to their counts), each wheel's spring/damper/steer keys, each spin's keys and 16 tire surfaces, the body's keys, the tail's surfaces and stunt speed, and the scene's links.
  - The wheels' mounts within 2e-5.

A row counts as a bad read, and is skipped, if its body equals the row before or after it, or if its force is not zero (read mid-tick).

## Not verified / not 1:1

- **Put-back (MD168a).** hazard-time rows 180–190 and hazard-on past row 29 are not compared. The port puts the quad back on the tick the clock passes its limit, standing still where it is. The original does it a tick later, at a kept spot. The port's put-back is MX Unleashed's (`quad::PutOn`). I did not read Untamed's vehicle put-back.
- **steer rows 197–204 (MD168b).** These are not compared: see the take-off knock above. Not checked: whether a solid struck, or the collision record's heightfield rays on Rocksbury's terrain. The test world has neither, and the track file is not on this disk.
- **No solids, no record terrain.** The world has nothing solid, and the collision record's rays get no terrain (`mxvatv_terrain` is `None`). Through row 196 that was enough to stay bit-exact.
- **Some state comes from the pack, not the image.** The collision record, the riding and wreck rays and the collidables are not read from the image (the record block's Untamed layout is not checked).
- **Buttons offset.** The buttons byte is read at `u(0x14ec)` = `+0x150c`, a guess: every pad here leaves it 0.
- **Struck flag.** The struck-class flag is set false. The image's record touches nothing, and Untamed's struck-mask address is unknown.
- **Mounts.** The wheels' mounts from the files differ from the image's in the last bits (up to ~1e-6). The replay uses the image's.
- **Engine table tails.** The image's engine tables hold leftover words past their counts. The files' quad has zeros there, and the seed test compares to the counts only. The replay uses the image's.
- **The `u()` map** is only as good as the offsets these tests read. Words outside its ranges panic.
- **Hazard images are mine.** The hazard states' `.ee.bin` are my extraction of the `.p2s` (context only; the tests skip without them).

## Next

Nothing here: MD168a (put-back), then MD168b (take-off).
