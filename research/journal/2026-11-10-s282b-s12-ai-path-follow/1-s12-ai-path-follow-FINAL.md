# 1 — s12 ai path follow (FINAL)

Task S282b. 2026-11-10.

## What I did

- **Narrowed the task, because the run was unattended.** The whole path follower needs six unported
  functions: the net locate, the path reach (vt+0x78, 876 B), the route follower (vt+0x8c, 1236 B), the
  look-ahead behind vt+0x88 (2348 B), the route locate it calls (460 B), and the path locate with a start hint,
  which itself needs the track's start-spot search. Porting and oracling all of that is several sessions. So
  S282b lands the two pieces that stand alone: the net locate and the tick's tail. The rest is filed as S308 (the
  look-ahead, route locate and hinted locate) and S309 (path reach, route follower, the branch itself, the sx01
  image). It is noted on S282b's plan line.
- `ai::follow::spot` (crates/game-sim/src/ai.rs) is the net locate, on S293's `race::TrackPaths` / `Spot`.
- `ai::follow::tail` and its `Brain` / `Tail` types are the tick's tail. They run after either branch (seeker or
  path). Its three callees are closures passed in (`avoid`, `plan`, `assist`), so the caller wires in
  `ai::update_mx` / the trick plan / the rider assist.
- research/oracle.txt: notes under path_spot and ai_seek_tail. New name `ai_tick_end` (the epilogue with v0 = 1).

## What the original does

- **Net locate** (`./kit fn` on path_spot in oracle.txt). Arguments are (track, point, out spot, distance
  pointer or 0, an unused word, holding_only). It walks the track's paths depth first from path 0 (track+0x4c),
  stacking branch paths through the route pool at +0x98. Paths already marked opened (4) or keep-off (0x100) are
  not stacked. Each path is marked opened as it is taken. A path whose only node is the first counts as a single
  place, at along 0. Every other stretch counts by the clamped nearest point on the segment. With holding_only,
  a stretch is only a candidate when `ring_holds` says the point is inside it. Ties keep the first found.
  Afterwards the opened marks are cleared and the pool entries are handed back. If the pool runs dry it returns
  0 and leaves the entries it took and the flags as they are. The distance is written as sqrt(d²) when d² > 0,
  as 0 when it is exactly 0, and left alone when nothing was found.
- **Tail** (lines 727–827 of the tick's decompile, ai_seek_tail to ai_tick_end):
  - Flag 0x1000: copy bit 0x80 of the bike's +0x40 word into the brain's 0x80 flag.
  - Flag 1: if the heading is more than 45° off the bike's forward (`mula(h.z,f.z).madd(h.x,f.x) ≤ cos 45°`),
    call the avoidance vt+0x74 with the brain's +0x150/+0x154/+0xc0/+0x1d0/+0x1e0 fields.
  - If speed > 0 and the rider is off the ground (flag 0x20 clear), pull the heading into the interface's
    velocity cone (+0x150). heading.w becomes vel.w.
  - Always call the trick plan vt+0x84 with dt.
  - Flag 0x800: +0x1c0 = h.x·speed, +0x1c8 = h.z·speed, then interface vt+0x18 with dt.
  - Return 1.

## How it was proved

- `the_path_spot_at_a_point_matches_the_track_objects` (crates/game-sim/tests/race.rs) runs 20,000 random nets
  and points through SLUS-21104's net locate in Ee and through `ai::follow::spot`. It compares the answer, the
  whole out spot, the distance word and both pools bit for bit. Cases hit include refused (dry pool), found,
  none found, holding-only, off path 0 and exactly on the line. Passed.
- `the_ticks_tail_matches_over_ten_thousand_ticks` (crates/game-sim/tests/ai.rs) runs 10,000 random brains
  through the original's tail. The run enters mid-function at the seeker's hand-off with $3/$19/f22 preset and
  ends with a hook at the epilogue. The three virtual callees are hooked. Each hook logs what it saw and then
  makes random changes to the heading, speed and flags, and the Rust closures make the same changes. The test
  compares the brain bits, the call order, every callee's arguments and dt. Minimum counts were met:
  avoidances 3000, turned past 45° 800, sweep flipped 800, air cone 800, assists 3000. Passed.

## Not verified / not 1:1

- The path branch itself (path reach, route follower, look-ahead, route locate, hinted locate, the hand-over to
  the seeker) is not ported. Split to S308/S309.
- No 4-racer sx01 race image was compared, and the emulator was not used. Both oracles run on generated
  inputs, so real track nets are untested. That is S309's job.
- The tail's three callees were hooked, not run alongside it. The avoidance was oracled on its own in S12c and
  the trick plan in S12d. The rider assist (interface vt+0x18) is not ported at all (S281).
- `tail` and `spot` are not wired into the Sim's AI tick yet. That waits on the branch (S309).
- The unused 5th argument of the net locate was assumed dead because the body never reads it. The test feeds it
  random values, and they make no difference.
- What bit 0x80 of the bike's +0x40 word means is not traced. It is copied as is.
- The MX vs. ATV PC exe was not read.

## Next

S308: port the look-ahead and route locate on `race::TrackPaths`. Use the same generated-net oracle as the net
locate test in tests/race.rs (its helpers `put_paths` / `check_paths` are reusable).
