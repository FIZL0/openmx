# 1 — atv rider row (FINAL)

Task MD165. 2026-11-16.

## What I did

- Found the reader MD138a could not: the rider's clip block is read by id through a vtable-only
  accessor, so no decompiled text names the offsets. A disassembly grep for the block pointer
  (`0xb18(`) found it. Addresses are in research/oracle.txt `# MD165`.
- The reader is the rider's idle layer (`./kit fn 0x004eb380 -p atv`, its callee 0x004eb6f0). Task F62a
  already ported this same code on RAM, as `game_sim::handling::rider::idle_state` / `idle_clips`.
  So the ATV rider runs the same step as the MX vs. ATV bike rider.
- Found what the quad hands the rider: the quad's state pack (`./kit fn 0x004ba690 -p atv`) fills the
  rider's record at quad +0x730. The pose and fore-aft words are the quad's own two blends (MD65c's
  `Anim::crossup_blend` / `fore_aft_blend`), the speed is `speed_len`, the reverse is `push_gate`, the
  gas is `gas_limited`, the wreck word is `control_off`, airborne is `!on_ground`.
- Ported it typed, render-side, in `game_render::quad`:
  - `Ask`: the record words.
  - `Motion`: the state choice, the two eased axes, the fade, the reverse lap and the pause clock.
  - `Seat`: the clips picked.
  - `seat_motion` / `rider_palette_motion`: lay them with the existing two- and four-clip samplers.
- `Driver` (MD138a) gets a `motion` field and ticks it after the chassis' animate.
- `quad_draw::row` / `rider` / `trace` now use it in place of MD138's own speed/air choice. The
  trace's `row` word is the row the rider is mostly on (its middle clip), then the eased lean and
  fore-aft (both 0..1).
- `AtvRide::rider_pause` carries the get-ready clip's length (the pause ends with it); main sets it
  from the look, beside `stunts`.

## What the original does

Each frame the rider picks a state:
- wrecked: the flail;
- a stunt asked: that stunt;
- in the air: the air row;
- reversing: reverse left or right by the lean;
- otherwise the still grid. After 15–45 s sat still (under 1 mph, both blends exactly a half, no gas)
  the get-ready pause plays once.

Two eased axes follow the record (fore-aft tau 0.125 s, lean tau 0.15 s, from a half):
- The fore-aft picks the pair: forward+middle under a half, middle+back over; mix = twice the
  distance into the half.
- The lean is where in the clips (each turn clip is a sweep of the lean).

The grid's row comes from the speed:
- under 5 mph: stopped and still rows, crossed by speed;
- 5–10 mph: the still row;
- 10–15 mph: still and riding rows, crossed;
- over 15 mph: the riding row.

The reverse pair runs on a lap of 0.03 × speed per second, wrapped to 0..1. A change of state stops
the controller and fades in from the stopped pose at a per-state rate.

## How it was proved

- Read off the disassembly and checked against F62a's port of the same function, clause by clause.
- Read on RAM images: the record words at quad +0x730 in `context/mxvatv-ps2/md37a/c28-atv-s-air`
  (state 0x3d, airborne, pose 0.4991, speed 53.97) and `c28-atv-s-spawn` (state 0x38, speed 0.73).
  The fade rates 0.25, 0.25, 2, 3, 2 were read in every md37a quad-rider image.
- Unit tests in `game-render` `quad::tests` (5 new):
  - the bands and crosses;
  - the pair choice and the eases;
  - air, reverse, stunt and wreck states;
  - the pause once;
  - the driver handing the quad's blends over.
- `atv_picture` passes on both renderers. A 380-frame ride on free09 printed:
  - stopped → still → riding rows as it sped up;
  - the air row over a jump;
  - the lean easing to 0 with the stick held left.
  The shot shows the rider seated, leaning left.

## Not verified / not 1:1

- **No capture compares the drawn rider with the original's.** The rows, weights and eases are
  ported, but no frame was matched against PCSX2.
- **Render-side float:** plain f32, not the PS2 accumulator rounding F62a keeps. This is a render
  path, so it was not worth the cost.
- **The pause's draw uses its own generator** (an LCG in `Motion`), not the original's rand and seed.
  Rendering must not touch the ride's random numbers. So the 15–45 s wait differs run to run from
  the original.
- **The pause's end is assumed to be its clip's end.** The word that ends it (rider +0x24c) is set
  somewhere not traced.
- **Wrecks always play the flail.** The original switches to one of five random hold poses once the
  ragdoll's slow word (rider +0x638) is set; the render has no such word.
- **The wreck/pause clip's cross-in is approximated.** The original's clip start cross-fades over the
  rate, then advances freely. The port plays the clip from the change, held on its last frame and
  faded by the same fade.
- **The stop snapshot is approximated.** The fade's "from" pose is the last seat sampled frozen, not
  a saved pose buffer. A stunt state draws that frozen seat with MD138a's stunt clip over it; the
  original plays the stunt clip alone, at the record's progress word.
- **The record's held / respawn / hold words (+0x10, +0xc, +0x170) are not fed.** They are false for a
  local ride. A net-held quad would snap the axes in the original.
- **The read order is a guess.** The pose and fore-aft are this frame's animate output. The pack may
  hand the rider the previous frame's (one tick of lag).
- **The flip/360 ids that keep the stunt byte** are matched by name (`FLIPS[..3]`), not by the
  original's three kept-id words.

## Next

Landed. A PCSX2 comparison of the rider's pose on a turn and over a jump would close the first item.
