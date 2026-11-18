# 1 — prop raiser (FINAL)

Task V134. 2026-11-18.

## What I did

- Ported the race activity's bale raiser as `game_sim::prop_raise::Raiser`.
  - Bind: each section's spot is put on the paths, the path point is written over it, and the spot is measured to the course line.
  - Tick: walks the cards; each group runs on its own clock.
  - Place: puts each body at the share of the way, or at the end.
- `Race.raiser` field, with its hash groups (`raise_up`, `raise_down`) and its wire entry. The per-tick queue `due` is skipped on the wire.
- The Sim's race frame ticks it after the drop gate sink. It places at once when the Sim holds the props. In split screen, `prop_seats::Shared::finish_flying` places each seat's queue on the shared set before its step.
- `PropSet.names`: each collection's name, filled in `load` and `join`.
- `Collection::place_at`: the placement at another height, done on copies.
- Shell: `race_start::funnel_words` reads `[Funnel_Up]`/`[Funnel_Down]`, and `race_start::prop_raiser` binds the raiser. main.rs binds it after the sink and prints `race: bales rise … sink …`.
- research/oracle.txt `race_prop_raiser`: the full layout, the tick, and the sx01 numbers.

## What the original does

- The race ctor always asks for a raiser (`./kit fn 0x002f9b70`, +0x3a4 = 1).
- The race loader builds it when the scene has both `[Funnel_Up]` and `[Funnel_Down]` and at least one prop collection matches.
- The match is `ObjectNameSubstring` in the collection name, case-sensitive. The defaults are `up_bale` and `down_bale`.
- Each section's `TriggerPosition` is spotted on the race paths and clamped onto the path. It is then measured along the paths to the course line, as the line stands after the supercross loader moved it onto the finish gate.
- Each tick, while either group is armed, every card is asked: kept distance not ≤ spot, and current distance < spot. The first such card starts that group.
- Start: keep each prop's place. Target y = y ± 50. t = 0, T = MoveTime.
- Each tick the down group runs, then the up group. For each prop:
  - if t < T: place at y0 + (target − y0)·(t/T). The place goes through physics_object_place on copies, so the prop's own place stays.
  - otherwise: place at the target, turn the group off, and make the target the prop's own place.
  - Then t += dt.
- sx01:
  - 10 `up_bale` props stand about 50 ft under the floor; 17 `down_bale` props stand on it.
  - Both spots are 3369.951 from the line. MoveTime is 1.
  - So the start funnel's bales sink and the lap's bales rise once the first rider comes round.

## How it was proved

- **A save-state image.** `crates/game-sim/tests/prop_raise.rs` `sx01s_spots_are_measured_as_the_image_holds_them` reads C31's sx01 race image (`context/states/c31-sx01-ai4.ee.bin`) and sx01's race file off the disc. The port's `trigger` from `TriggerPosition=562,120,706` to the image's moved line gives the image's +0x80/+0x84 bits (3369.9512). The written path point's x and z match the image's words. The race file's own line gives 3371.399 instead, which is how the line choice was settled.
- **The image's counts.** The image holds 10 up and 17 down props and MoveTime 1. A headless shell race on sx01 found the same groups (`race: bales rise 3369.95 …, 10 up, 17 down`).
- **A live shell run.** A bot race on sx01 for 1500 frames set off both groups on the same tick. They ran 30 shares and an end each (a temporary print, since removed).
- **Unit tests** in the same file cover the rest:
  - the name match
  - the spot clamp
  - the trigger rule (on the spot or kept-not-farther does not count)
  - linear shares from a fixed y0, with the end making the target the new place
  - down before up
  - place, look and up left alone during the move
  - the hash walk and a snapshot carried through the move in a Sim
- `race_start.rs` tests the section reader's defaults and its need for both sections.
- **The disassembly** of the start (`./kit dis 0x002fe7e0 --len 0x130`) and the tick (`./kit dis 0x002feb40 --len 0x410`) was read line by line for the per-prop math and its order.

## Not verified / not 1:1

- **No capture of the move itself.** The heights tick by tick and the bodies' poses after each place were not compared against the original. They follow the disassembly (sub.s, mul.s, add.s through `fpu::`), but no frame-by-frame recording backs them.
- **The trigger tick was not compared to a recording.** The first card that sets each group off was not checked against a recording. The tick it fires on depends on the cards' to-line distances, which other tasks own.
- **The order against the drop gate sink was not traced.** The raiser runs right after the sink in the race frame. Both are scene objects on the score manager's node. Their state is independent, but the order relative to other pass-2 children was not checked.
- **vt+0x20 at start and vt+0x18 at the up group's end are not ported.** The raiser calls them on itself. What they do was not followed. The props' heights do not go through them.
- **The trigger box is not ported.** TriggerLook, TriggerWidth and TriggerHeight are read by the original's section reader, but the raiser's tick does not use them (only the distances). The port does not read them.
- **Only the race activity's raiser is ported.** The ghost activity's load (0x0031a620) and 0x00349cc0 also build one; neither activity is ported.
- **Split-screen timing is not proved.** Each seat's queue is placed on the shared set in `Shared::finish_flying` after its movers are found, as the one-Sim path places after the search and before the step. No split-screen capture backs this.
- **Rollback mid-race restores the raiser's own state, not the queue.** The queue is empty between ticks, so nothing is lost; but a snapshot taken inside a step (none is) would drop it.
- **The ObjectNameSubstring quotes.** The port strips quotes from the value like other `.toy` names. No disc track sets this key, so how the original reads it was not checked.
- **A world map's tiles get no raiser.** The bind sits inside the shell's no-world branch, as the sink's does. A mod with both sections on a world would not raise its bales.
- **The spot point's y and w are not checked.** The test's race file has no node heights until the shell grounds them. The shell's run printed the image's y (91.2977); w was not compared. The raiser does not use either after the bind.

## Next

Lands with `./kit done V134`.
