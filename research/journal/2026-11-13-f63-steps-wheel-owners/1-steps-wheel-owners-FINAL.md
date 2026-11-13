# 1 — steps wheel owners (FINAL)

Task F63. 2026-11-13.

## What I did

- `quad::Bike::ground_of`: each wheel's `+0x1f0` owner and `+0x1f4` vehicle, the addresses
  `Bike::ground` was read off. `steps::fields` keeps both words, so `steps::words` writes them back.
- `steps::read` no longer refuses a wheel with an owner or a vehicle. New `steps::ground` reads what
  they stand the wheel on, at the contact velocity's offsets:
  - an owner on a rigid body (`+0xe4`) or a vehicle's chassis (`+0xe0`, transform `+0xe8`);
  - a vehicle alone is a mover. Its velocity and spin are its two virtuals' answers, so the caller
    hands them in. `steps::read_with` takes them; `steps::read` refuses a vehicle alone, as before.
- `quad::Sweep` has three new methods: `tell(owner, i)` (an owner's listener told), `ground(owner,
  vehicle)` (what a newly set wheel stands on), and `object_cofg(owner, cofg_world)` (the contact
  velocity's one write into the world, the object's `+0x90`). The defaults keep the old behaviour for
  worlds that file no owned contacts: a debug assert and still ground for the first two, nothing for
  the third.
- `quad::Steps`:
  - Both contact walks (the bike's two wheels and the quad's four) now tell listeners through the sweep
    and set each wheel's owner, vehicle and ground (`Steps::stand_on`). The refusing debug asserts are
    gone. Each walk starts from the wheel's kept owner and vehicle, since a wheel no contact sets keeps
    them in the original. (The first oracle run failed here: the walk had started from zeros, so an
    unset rear lost its owner.) A wheel whose owner and vehicle don't change keeps its ground.
  - The wheel clear zeroes `ground_of`.
  - A wheel that borrows its partner's load in the spin integrate copies the partner's `ground_of` with
    its ground.
  - The tire forces and the spin integrate tell the sweep the object's centre of mass whenever they
    work out a ground on an object (`Steps::told_cofg`).
- `tests/mxvatv_bike_ride.rs`:
  - `TriggerInRam` takes owned contacts now: it tells listeners in RAM, reads grounds there and writes
    the object's centre of mass. It counts them as `wheel contacts owned or on a vehicle`.
  - `StepsInRam` reads the bike with `read_in_ram`, which asks a vehicle's virtuals in the interpreter.
  - New oracle `the_steps_routes_owned_wheel_contacts_are_the_originals`.

## What the original does

The walk is F52's (oracle.txt `# F52`). Its set-contact writes the wheel's `+0x1f0` (owner) and
`+0x1f4` (vehicle). The wheel clear zeroes both. The contact velocity (`./kit dis 0x004a1150 -p atv`)
reads them only while the wheel's ground is not yet known (`+0x1a0` clear):

- With an owner: the owner's `+0x80` centre of mass turned into the world and written to `+0x90`, on
  a body `+0xe4` or else a chassis `+0xe0` through the transform `+0xe8`.
- With a vehicle alone: the vehicle's vtable at `+4`. Slot `+0xc4` (spin) is asked first, then `+0xc0`
  (velocity). Each is handed a stack quadword and the vehicle, and returns the quadword. Then the
  transform `+0x244` and the origin `+0x180`.

oracle.txt `# MD74` has the rest.

## How it was proved

- `the_steps_routes_owned_wheel_contacts_are_the_originals`, 10 000 laid cases, every byte outside the
  stack and the stand/search/tell calls in order equal. The original side runs `bike_wheel_contacts`
  and then the contact velocity on the front and the rear. The port side runs `steps::read_with`,
  `Steps`' `Call::Contacts` on `OwnedInRam`, `steps::words`, a second read, and
  `atv_wheel::contact_velocity` on each wheel's ground. Tally: ground known 4944, on a body 5650,
  on a chassis 5776, on a mover 3969, on still ground 4605, struck a vehicle alone 1670, struck owned
  8249, struck the world 2410, told 3955.
- F52's tests still pass with the change: its walk oracle and both C15 rides (ride-2, flip-1) with the
  wheel contacts in Rust.
- A test trap on the way: `Ee::call` clears `written` at every call, so a test that makes several
  calls on one side has to keep the earlier writes. That's the new `call_kept`, also used by the
  in-RAM tells and the mover calls. Before it, the original side was judged on its last call alone.

## Not verified / not 1:1

- No C15 ride strikes an owned or vehicle contact (the trigger's rays only meet plant spheres), so
  nothing here is proved over frames of a real ride; the oracle lays the RAM by hand.
- `Steps::told_cofg` in the tire forces and spin integrate arms (the object's `+0x90` write) is
  mirrored in the oracle's port side by hand after `contact_velocity`. It is not run through
  `Steps`' TireForces/SpinIntegrate against the original. The SpinIntegrate partner-load borrow
  copying `ground_of` is read off the code, not tested.
- A mover's virtuals are asked when the ground is read or set (contact time), not when the contact
  velocity runs. The two agree only while the vehicle doesn't change in between. In the oracle the
  hooked virtuals return laid vectors, so the order isn't tested.
- `steps::read` (no closure) refuses a vehicle alone, since it can't call virtuals.
- A transform with a parent or out of date (`+0x100` bit 0 clear) is refused, not brought up to date.
- `Sweep::tell`'s listener calls in the test pass no `f12` (F52's oracle passes `dt`; the hooked tell
  ignores it).
- The defaults of the new `Sweep` methods keep the old debug asserts. A world that files owned
  contacts without overriding them still trips in debug and stands the wheel on still ground in
  release.
- The PC exe was not read.

## Next

- F65 (parked): prove an owned contact over real frames once a capture strikes one (a ride over another vehicle or
  a hinged prop).
