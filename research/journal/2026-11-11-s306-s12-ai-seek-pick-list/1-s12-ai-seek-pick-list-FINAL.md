# 1 — s12 ai seek pick list (FINAL)

Task S306. 2026-11-11.

## What I did

- Ported the original's pick-list build as `ai::seek::build` (crates/game-sim/src/ai.rs) and wired it
  into the seeker's draw: a set with no pick list builds them all, then draws; still none, or a group
  with no list, draws nothing (as before).
- The brief's "pick list" turned out not to be a list the build fills in. It is a pointer to one of the
  map's **jump groups**, the same group for every set: the group nearest the human rider. So the model
  changed. `Set::picks` is now `Option<usize>`, an index into the new `World::groups`
  (`Group { targets, point, active, open }`). `Set` gained an `owner: Owner { kind, player, position,
  hit_freeride }`, the rider the set belongs to, which is what the build reads.
- Nothing else uses `seek::World`/`Set`: only ai.rs and tests/ai.rs did.
- Nothing taken from opentennis. It has no equivalent of this.

## What the original does

`./kit fn 0x003a9f10` (named `ai_seek_list_build` in research/oracle.txt, now described there in full).
It is called with the root word `ai_seek_sequence_root`, from the pick `./kit fn 0x003a6ca0`, whenever
the asking set's +0x11c is 0. It does this:

1. It walks the root's set list (+0x30 count, +0x38 items) for the first non-null set whose owner
   (+0x54) has +0x34 == 1, then stops looking.
2. It reads that owner's root-node position (+0x3c → [0] → +0x18, then +0x70). It also asks the owner
   (vt+0xac) for the named flag "HitFreeride" (the bit comes from `name_bit`).
3. It goes over the groups (root+0x44 → +0x30/+0x38). A group counts if its +0x78 word is exactly 1,
   and, when the owner said yes, if bit 0 of the group's byte for the owner's player (+0x44 → byte
   [+0x30]) is set.
4. Of those, it keeps the nearest: squared distance in x, y and z (`vsub.xyz`, then
   `mula`/`madda`/`madd`), beaten only by strictly less (`c.olt`), starting from FLT_MAX. On a tie the
   first group wins.
5. It writes the result, or 0, into +0x11c of every non-null set in the list. So with no owner of kind
   1, or no group that counts, every set's list is **cleared**, including sets that had one.

The pick then draws off the group's +0x48 list, as S282a already ported.

## How it was proved

- **Function oracle over 10,000 generated worlds**:
  `the_pick_list_build_matches_over_ten_thousand_worlds` in crates/game-sim/tests/ai.rs calls the
  original build on a laid-out root and compares every set's +0x11c. The worlds have 0–6 groups and 3
  sets, with null entries in the set list. Some groups are out of play (+0x78 = 0, 2, -1, 0x101), some
  have open bytes whose other bits are set, some players fall past the open bytes, some owner kinds are
  not 1, two groups sometimes share a point, and one owner gives a "yes" with only bit 40 set. The
  stubs: `name_bit` gives a fixed bit, and the owner's vt+0xac checks its arguments and returns the
  generated answer. Coverage over the run: no owner 1226, no group 3986, open-only 1376, any-in-play
  3412, a nearer group passed over 1598.
- **The seeker oracle**: `the_target_seeker_matches_over_ten_thousand_ticks` no longer hooks the
  build out. It runs the original's build inside ai_tick_flag and now also compares every set's pick
  list after the tick. Built 237 ticks, built-none 175, picked off the list 756; everything else is as
  S282a left it.
- **Mutation**: making the comparison `<=` fails the build oracle (the shared-point case). The code
  was then restored by cp + touch.

## Not verified / not 1:1

- **What the owner's +0x34 kind and the "HitFreeride" flag mean in the game** is not traced. The port
  takes both as inputs (`Owner::kind`, `Owner::hit_freeride`), and so does the oracle. It is inferred
  from how the game uses them, not checked in a live game, that kind 1 is the human rider in a free
  ride. The same goes for what sets the flag. Whoever fills in `Owner` (S313) must read them from the
  game objects.
- **Nothing builds `World::groups` or `Set::owner` from the disc** or from a save state yet. That is
  S313's load, and I added a note to S313 to say it must fill these too. The build is proved only on
  generated worlds, never on recording2's free09 image.
- **A null group entry, or a set whose owner pointer is null**, is not modelled. The original would
  dereference them; the port has no null groups, and every set has an owner.
- **The owner's position is read with w forced to 1** in the original. The port ignores w (`vsub.xyz`
  only), and that has no effect on the result.
- **A player past a group's open flags** reads "closed" in the port. The original reads whatever
  byte comes next. The oracle lays zeros there.
- **The pick's other branch (`./kit fn 0x003a6ca0` with a course, +0x6e4/+0x6f4)** is unreachable from
  the seeker, which only calls it with no course. It is not ported. Its other caller,
  `./kit fn 0x003834b0`, is not looked at.

## Next

Nothing for S306. S313 loads `groups` and `owners` alongside the sets, targets and jumps.
