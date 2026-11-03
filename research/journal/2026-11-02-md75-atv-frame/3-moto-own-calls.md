# The quad's own Moto calls (MD75b)

## What the original does

Read with `./kit dis -p atv`. The field maps are in research/oracle.txt `# MD75b`.

- **Engine update** (atv_engine_update, 0x0049fbb0) is MX Unleashed's engine (`r#move::engine_update`) on
  one or two driven wheels (engine +0x90/+0x94):
  - the curve is seeked with MXU's own curve_seek (0x00134450, the same code). It is read as
    `adda(0, hi) msub frac·(hi − lo)` (0x00134530), or the bottom point at index 0;
  - torque = `clutch_torque_scale·((curve + brake)·min(gas + idle, 1) − brake)`, with
    `brake = rpm·(eb·rpm)`. The gas and clutch are read through two pointers at +0x24/+0x28, which
    point at the quad's limited gas and clutch lever;
  - the road's revs with two wheels are `ratio·(9.549·(½·(s₁ + s₁)))`. The first wheel's spin is taken
    twice, so the second wheel's spin never counts. With one wheel they are `(9.549·s₁)·ratio`;
  - the lock and slip are MXU's. The wheel torque goes to the drive helper (0x004a1eb0, wheel +0x210 +=):
    all of it to one wheel, or half to each of two;
  - the slipping engine's load: with two wheels and both tires' "forward force from torque" flags set,
    `inv_ratio·((1 − c)·(½·(f₂r₂ + f₁r₁)))`, summed on the accumulator. With one wheel and its flag set,
    the same off one tire. Otherwise it is the clutch's own share;
  - revs = `adda(0, rpm) madd 9.549·((inv_inertia·(torque − load))·dt)`, then clamped to min/limit.
- **Rev limit** (0x0049ff40): when the clutch is locked and the road's revs are past the limit, the first
  wheel's spin is set to `(0.1047·limit)·inv_ratio` and the second wheel's is copied from it.
- **Auto shift** (0x004e5fe0): if the contact speed is under 7.3333 (bits 0x40eaaa65) with the gas shut,
  the clutch lever goes in. Then the gear goes up one past the up-shift point, or down one under the
  down-shift point (MXU's set_gear, 0x0049fb50, the same code).
- **Head and tail** (0x004e4190):
  - the pose is the control-off one (+0x1140), else the balance one (when +0x1524 == +0x152c), else the
    riding one (+0x1130);
  - its z moves by the stick, the brake's travel mapped off |lean|, and the clutch ramp's travel times
    its weight;
  - the body's set-cofg (vt +0x5c, 0x00493b40) then moves the position *and the last position* (+0x60)
    by `to_world(new − old)`.
- **Gravity** (0x004a4ef0 → 0x00493f60): with the body enabled (body +0): add_force(gravity), then the
  collision record's vt +0x88 with 1, then clear bit 5 of quad +0x45. With the body off it does nothing at
  all, and the flag bit stays.
- **Air drag** (0x004e6c00): `−drag·((½·|v|)·|v|)` along the VU0-normalised velocity, xyz. The drag is the
  world params' +4 (0.025 in the image).
- **Peak rates** (0x004e6c90): the body's correction peak in its own frame, kept per lane in +0x1250 when
  it is not smaller by magnitude.
- **Wheel clear** (0x004a0c30): ten stores. The ray and sweep flags, contact, compression and spring force,
  and both ground pointers (+0x1f0/+0x1f4) are zeroed, along with the ground-known word (+0x1a0). The
  three velocities at +0x170/+0x1b0/+0x1c0 are set to (0,0,0,1).

## The port

- New `crates/game-sim/src/atv_moto.rs` (`pub mod atv_moto`) holds one function per call:
  `engine_update`, `rev_limit`, `auto_shift`, `head_tail`, `gravity`, `air_drag`, `peak_rates` and
  `wheel_clear`. It also holds `Parts`, the words these calls touch that `quad::Bike` had no field for:
  - each wheel's drive words (`Drive`);
  - which wheels are driven;
  - the control-off pose;
  - the body's enabled word and last position;
  - the flag byte;
  - whether there is a collision record.

  The engine itself is `r#move::Engine`. The shift points, poses, travels and drag come from `Params`
  (`Bike::clutch_keys`), as MD75a's clutch does.
- `quad::Bike` gains `parts`, and `quad::Call` gains `GravityRecord` (the record's virtual, which goes on
  to `rest`). `quad::Steps` answers the eight calls.

Only three adds were made outside the new file: the module line, the variant and the field, plus the new
match arms in `Steps::call`.

## Proof

`./kit check -- -p game-sim --test atv_quad`: 36 passed. Ten of those tests are new. Each runs the
original's ground head or ground step with the picked call(s) unhooked, so the image's own code runs. It
then runs the port's `quad::Steps` on the same generated ride and compares every region and the call log.
The new tests, by seed:

- `0x0075_b001` engine update, 10,000 states. The paths are read off the port's own run on a copy:
  - one wheel or two;
  - stays locked, unlocks, locks up, or slips;
  - the load off both tires, off one tire, or the clutch's;
  - held at the bottom or at the limit;
  - the curve's bottom point.

  A third of the states are set up to cross the road's revs, so that the lock-up case occurs.
- `b002` rev limit, 10,000: limits two, limits one, under the limit, slipping, off.
- `b003` gravity, 10,000: pulled, body off, off.
- `b004` air drag, 10,000: dragged, still, off.
- `b005` peak rates, 10,000.
- `b006` wheel clear, 5,000. Each wheel stands on still ground, an object or a mover. The poke writes a
  marker into +0x1f0/+0x1f4, which the clear must zero.
- `b007` auto shift (head), 10,000: crawl with the clutch in, up, down, held at the top, held at the
  bottom, held. Half the states have revs set near a shift point.
- `b008` head and tail (head), 10,000: control off, balancing, riding, stick forward and back, braking,
  with and without a clutch ramp.
- `b009` the whole Force body with all twelve ported calls unhooked, 5,000.
- `b00a` the whole ground head with its four ported calls unhooked, 5,000.

The harness changes:
- the engine moved to 0x01c0_3000, and its region widened to 0x1c0 so that its ratios and curve fit;
- `poke_ride` writes the new words;
- the record's vtable is hooked at a marker address and asserts its arguments (record, 1);
- `address_of` routes the eight calls.

Each path is seen at least 30 times (15 in the combined tests). The image is there and the tests ran.

Mutations, each run against its own test and reverted:
- **17 went red** (16 at once): the curve's msub → madd; the true two-wheel mean for the road's revs; no rev clamp;
  the one-wheel load ignoring its flag; dropping the lock-up's first clause; dropping the rev limit's
  copy; the rev limit's product order; the crawl ignoring the gas; the travel swap; not moving the last
  position; the brake travel off a signed lean; the clutch ramp's ends swapped; keeping the flag bit;
  never telling the record; the peak compare without abs; not clearing the spring force. The wheel
  clear without the ground reset went red once the ground markers were added. Before that the
  generator only made still ground, so that mutation survived.
- **8 survived**. None of them can change a result the generator can reach:
  - `rpm·(eb·rpm)` → `(rpm·eb)·rpm`: these are the same, because multiplication commutes. A probe found
    0 differences in 9,545 states;
  - the order of the two-tire load sum: two rounded products added, which commutes;
  - the unlock's `clutch == 1` test: with the clutch at 1, the static limit is 0, so only a torque of
    exactly 0 tells the two apart;
  - `adda(0, rpm)` → a plain add: adding 0 is exact;
  - the up shift's `!(rpm <= up)` → `!(rpm < up)`: these differ only at equality;
  - the auto shift's rpm product order: the rpm is not stored, and only a different rounding at a
    threshold would show it;
  - the drag's `(½·v)·v` → `½·(v·v)`: halving is exact;
  - the drag's xyz merge: add_force adds xyz only. The original does `vmulx.xyz` too.

## Not done, filed

- MD75b1: spin air (the Force body's 2048-byte 360), vehicle post (the PostProcess's scene-graph update)
  and node position. These go deep into the scene graph and collision, which this task does not reach.
  So they are still hooked log-only in every oracle. The task's scope was cut to the eight calls above.
  That decision was made unattended.

## Not verified / not 1:1

- No real frame of the game has run through the port. Each oracle starts from a generated ride laid out
  in RAM over the image, with the unported calls (and MD76's ground query) hooked.
- The engine paths are classified by running the port's own `engine_update` on a copy. A wrong port could
  therefore mislabel a path, but the region compare still catches the wrong result.
- The engine's tallies assume the gas and clutch words are unchanged between the PreProcess and the
  engine's substep. That holds in the test (the calls between are hooked), but in a real frame the
  preload and backflip run in between.
- Gravity's record call is answered by `rest` as `GravityRecord` and is not ported. The record's
  `+0x88` virtual is unknown.
- The generator always has a collision record (`Parts::record`). The no-record path (quad +0xc8 = 0) is in
  the port and the poke, but no test drives it.
- Ties at the shift points and at a zero torque with the clutch fully in are not generated (see the
  surviving mutations).
- `Parts` is a second home for words that a whole-quad reader will have to fill. There is no reader yet:
  that is still MD75's whole-frame proof.
- Clippy was not run (it cannot lint game-sim). `atv_moto.rs` denies float arithmetic and uses ps2-float
  ops only.
