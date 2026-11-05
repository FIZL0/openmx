# 1 — anim controller (FINAL)

Task F43a. 2026-11-05.

## What I did

- `crates/game-sim/src/handling/anim.rs` (new): `stop`, `play`, `blend`, and the two channel samplers
  `rotation` and `position`. They work at word level over `node::Ram`, plus an `Anim::alloc` for the
  two saved-pose buffers a first stop makes. `Tables` hands in the turn table (`toys::Turns`) and the
  arc cosine table (`air::AcosTable`).
  - Reused: `math::slerp`, `math::quat_to_mat4`, `math::mat4_to_quat`, `toys::decode`,
    `node::moved` and `node::set_position`.
  - Two visibility changes, nothing else touched: `node::set_position` and `Turns::wide_at` are now
    `pub(crate)`.
- `tests/mxvatv_bike_ride.rs`:
  - `UpdateInRam::out` serves `AnimStop`, `AnimPlay` and `AnimBlend` with the port. Before each play
    or blend, `kinds` asserts that every channel of the clips is one of the two sampler kinds.
  - `NodeRam`'s `alloc` goes to the original heap.
  - The interpreter arms are now `unreachable!`.
  - `RIDE_CALLEES` drops from 13 to 10 and `FLIP_CALLEES` from 31 to 28.
- New tests: `the_anim_key_tables_are_the_images` and `the_anim_controller_calls_are_the_originals`.
- `research/oracle.txt # F43a`: the layouts, the addresses, and the stack slots.

## What the original does

Recipes: `./kit dis -p atv <addr>` with the addresses in `research/oracle.txt # F43a`.

- **Stop.** If either saved buffer is missing, it allocates it (a quadword per bone). It clears the
  fade-list bit. Then, for each bone, it saves the node's own rotation as a quaternion (largest
  component first) and the position row's `xyz` with `w` set to 1.
- **Play.** `t = weight × clip length`. For each rotation channel whose bone byte is not 0xff:
  - sample it;
  - fade it: a short-way slerp towards the saved rotation by `fade`. This happens unless the fade
    list is in use and names the bone, or `fade ≤ 0`, or there is no saved buffer;
  - store it in the bone;
  - turn the node if the turn bit is set. That writes the three rotation rows, with `w` 0, and calls
    moved.

  Each position channel is sampled straight into the bone. It is faded as `p + (saved − p)·fade` on
  VU0 (`xyz` only), and the node is placed if the place bit is set. Last, the clip and `t` are kept.
- **Blend.** Channel `k` of each clip is sampled.
  - Rotations: a short-way slerp from the second clip's sample to the first's, by `mix`.
  - Positions: `b − (b − a)·mix`, with `w` taken from `b`.
  - The bone comes from the first clip's map. The position is stored even when that byte is 0xff,
    at bone 255.
  - The fade and the store then work as in play. The counts are the first clip's.
- **Rotation sampler.**
  - One key: that key slerped with itself.
  - Two keys: `u = (t − the word just before the keys) / (t1 − t0)`. This reads outside the list.
  - Three or more:
    - The cursor first gets a quarter/three-quarter reset.
    - It walks forward, then back.
    - It ends on the pair `i − 1, i` and stores `i`.
    - If a walk ran off either end, the result is the end key decoded alone.
  - Decode: `xyz = itof(halves) × (scale ^ sign(cos))`, `w = |cos|`.
- **Position sampler.**
  - When the channel is on and has two or more keys: a binary search for the bracketing pair, or the
    last two past the end. Each lane is `adda(0, a).madd(u, b − a)`; past the end the base is `b`.
  - Otherwise it holds a key, and the original writes `out[0] = key.x`, `out[1] = key.y`, then
    `out[0] = key.z`. So `x` gets the key's `z`, and `z` is never written.
  - `w` is always 1.
- **Stack slots, which decide what a skipped write leaves behind:**
  - Play keeps one quaternion slot across channels.
  - Blend keeps four slots across channels: the first clip's rotation, the mixed rotation, the second
    clip's position, and the bone's own position. The fade copies the saved rotation into the first
    clip's rotation slot, and the saved position into the second clip's position slot. So an off
    channel or a held `z` in the next iteration reads those saved words. The oracle found this:
    case 24 was a 1-ulp `z` miss until the port carried it.

## How it was proved

- **Function oracle, 10 000 cases.** `the_anim_controller_calls_are_the_originals`, in
  tests/mxvatv_bike_ride.rs.
  - Setup (`lay_controller`): bones on a random F43 node forest, with every word random first. The
    saved buffers are each present or missing. The fade list is present or not, with random entries.
    The flag bytes are random. There are two clips, each with up to four rotation channels and three
    position channels. Each channel has a random map byte (one in five is 0xff), is on or off, and has
    −1 to 20 keys. Times rise, now and then repeating a time. Rotation keys are mostly unit axes, with
    turns within the table. Lengths, weights, mixes and fades include 0, 1, negative values and
    values past 1.
  - Each case is a stop, a play, a blend, or one sampler on its own. The original runs in the
    interpreter with its stack zeroed first, then the port runs on the same RAM. Every word of the
    controller region and of the forest must match, and so must any saved buffers made. The original
    must write nothing outside those and its stack.
  - A hook stands in for the heap: it hands out two fixed buffers and asserts the size, file and line
    arguments. The real heap reaches a syscall the interpreter lacks.
  - Path counts must each reach 20 or more: stop with and without allocation; play and blend faded
    and not, with and without a fade list, by node bits; each sampler on and off, with 0, 1 or 2 keys,
    and before, inside and past its keys.
- **Tables.** `the_anim_key_tables_are_the_images`:
  - `Turns` (4096 wide plus 256 narrow entries) matches the image's table word for word.
  - `AcosTable` matches the table the image's pointer names, and the scale is 14999.5.
- **Replays.** `ride_replays_bit_exact_on_the_ports_record_pass` (ride-2) and
  `flip_wrecks_and_is_put_back_bit_exact_on_the_ports_record_pass` (flip-1) stay bit-exact with the
  port serving the update.
  - To show they really ran (a pass hides a skip), I raised their row-count asserts temporarily. Both
    then failed on the count (459 rows for ride-2), so they had run, and I put the asserts back.
  - The census tests pass with the three labels gone.
  - All 51 tests in mxvatv_bike_ride pass.
- **Mutant.** I deleted blend's carry of the saved rotation into the first clip's slot. The oracle
  caught it (bone rotation words differed), and I restored the line.

## Not verified / not 1:1

- **Only the update's call shapes.** The following are not ported: stop with `a1 ≠ 0` (it sets up the
  fade list); play and blend with mirroring (`a3`), a saved-buffer override (`t0`/`t1`), the
  alternate start (`t2`, `+0x58`), clip events (`t3`), and a channel-mask list (`a2` on play). The
  update passes zeros for all of them.
- **Only the two channel kinds.** The rig asserts the samplers; the replays' clips hold only these
  two.
- **The first channel's leftover stack.** When the first rotation channel is off or empty, or the
  first held position leaves `z`, the original reads stale stack. The port reads zeros. The oracle
  zeroes the stack, so it proves only that case. On the console the value depends on earlier calls.
  The C15 replays never reach it: all their channels are on with 16 keys.
- **The cursor and turn ranges.** The game stores cursors from 1 to the last key. The oracle draws
  −1 to the last. Beyond that, the original reads a key past the end, and its turn may index past
  the 4352-entry table, where the port wraps instead (`Turns::wide_at`).
- **A negative position-channel count.** The original's search never ends, so the oracle draws none.
- **The two-key read before the list.** It is ported and proved on random words. Real clips' word
  there is whatever precedes the key array.
- **The heap.** Allocation is proved only through a hook; the real allocator is not run. In the rig,
  `alloc` calls it, but the replays never allocate (their buffers exist).
- **Mutants.** Only one was run.

## Next

(FINAL.) None for this task. The rest of the update's interpreter callees (search and attachment move)
are F43b.
