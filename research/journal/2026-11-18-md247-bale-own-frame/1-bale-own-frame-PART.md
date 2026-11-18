# 1 — bale own frame (PART)

Task MD247. 2026-11-18.

## What I did

- `quad::KnockFrame`: a knockable body's own frame (rays and their stored tree, gravity, its world's rate,
  its held resolution). `Knockable::with_frame` gives one; `Knockable::own` holds it.
- `Ride::record_among` (new `hull` argument): a body with a frame that the quad's pass strikes wakes. Each
  awake body is fully cleared before the quad's pass. After the quad's pass, each awake body runs its own
  pass against the world and the quad's hull (the chassis frame mesh at the quad's scene node, with the
  quad's record swapped in). Its mirrors land on the quad's list after the quad's own hits.
- `RecordAmong`: the bodies' hits now keep the probe's order (the descent's), not ray order. A new `tree`
  field walks the bale's stored `m.col` tree instead of a built one. That gives the original's order: by
  hull face, each face's hits in ray order.
- `Ride::frame_among`: after the quad's world step, each awake body's own world step (`step_own`):
  correction peak cleared; per substep gravity, then the record's resolution against the quad; then
  integrate and end. The quad's body and collider take the push back. It runs before the tail.
- Test: `bales()` reads every knockable from the image (vtable scan), with mesh, `m.col` rays and tree,
  record, body, gravity (object `+0x118` `+0x40`) and world rate (`+0x110` `+4`).
  - `untamed_quad_replays_md147_steer` now rides with them, still to row 196. No bale wakes before then.
  - New `untamed_bale_wakes_and_files_its_hits_on_md224_steer` pins both lists at tick 197.
- Gravity was first read from the stepping world (`+0x110`), which gave garbage forces and a blow-up. The
  force listener reads `+0x118`.

## What the original does

- Sim frame (`./kit fn 0x00204ab0`), in order:
  - every record's clear;
  - every record's pass, on a re-read list, so a bale woken during the quad's pass gets its pass that tick
    but not its clear;
  - the quad world's step;
  - the tail. The tail's first listener (`./kit fn 0x0051c9f0`) steps the props world (rate 55, 2
    substeps at 30 Hz).
- Wake (`./kit fn 0x004bad80`, from the collidable's listener during the quad's search): the body goes
  into the props world and its awake word is set.
- Props substep force listener (`./kit fn 0x004ba390`): force += gravity, += 0, then the record's
  resolution with impulses on. Then integrate and end substep.
- Full list in research/oracle.txt `# MD247`.

## How it was proved

- A throwaway probe (not committed) stepped MD147's spawn image in the interpreter on MD224's steer rows
  and dumped:
  - both lists before and after the props step;
  - both bodies around each resolve call (0x00206170) and each quad integrate (0x004bc670);
  - the wheel words.
- Against it at tick 197, all 31 list entries (quad 11, bale 20) match bit for bit: kind, other, rays,
  normal, point.
- The quad's first resolve that tick matches bit for bit: force and moment after it.
- `untamed_bale_wakes_and_files_its_hits_on_md224_steer` pins the lists' (kind, other, ray) against the
  original's. A swapped expected ray makes it fail (tried).
- `./kit check -p game-sim --test untamed_race_quad`: 7 pass, none skipped.

## Not verified / not 1:1

- **The quad and the bale are off from tick 197.** The original's left front wheel sweep lands on the
  bale that tick (wheel `+0xdc` and `+0xd4` set on all 4 substeps). That adds about 220 lb of up force
  after the first resolve, and the port's wheel sweep does not see knockables. So the quad's velocity
  entering the props step is off (x −0.736 against −0.677). The bale's hit normal speeds (−9.4 to −8.7)
  then miss the −10 impulse line that the original's pass. The quad gets no impulse, and the bale's body
  is off. Follow-up MD253. The brief's "extend to row 204" waits on it.
- The props step itself (`step_own`) is not proved bit for bit: its inputs were never right at 197. Its
  order and calls follow the decompile and the probe's call trace.
- Not ported in the props step:
  - the end listener's in-world test and vt `+0x124`;
  - the body ever going back to sleep.
- Each awake body steps its own world alone. Two awake bodies sharing Untamed's props world are not
  stepped together or against each other. Only one bale wakes on this ride.
- A bale's search covers the quad's hull and the world solids only, not other bales.
- A body's world and body hits on one ray: the nearer is kept, in the body's place. Never exercised here
  (no ray hit both at 197).
- The quad's own hits on a body keep the probe's order too (built tree). That matches at 197 only
  because there are two.
- Rows 197 and 198 of MD224's steer are mid-tick reads (MD224's `bad_read`), so they cannot check the
  bale's body anyway.

## Next

MD253: sweep the wheel trigger segments against the knockables' meshes in `frame_among`. The contact's
owner is the bale's record, its listeners are told, and the wheel's ground is the bale's body. Then
extend `untamed_quad_replays_md147_steer` to 204 and compare `bale_body` on the well-read rows after 197.
The probe recipe is in this file's "How it was proved". Its hooks wrap the call with `ee.call` and put
`pc` back.
