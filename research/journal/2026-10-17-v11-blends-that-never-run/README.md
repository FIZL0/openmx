# V11 — blends that never run

2026-10-17. Task: `./kit plan V11`.

Two writers in the image ask for a four-clip blend that the port could never reach, and the question
was whether the rider's select was missing an arm or those writers were not wired up. It is the
second: the select has no such arm in the original either, and the machine that does write modes 4
and 5 is the sixty-state one S25 ported, which in the port was a leaf — because the only writer of
its `want` is a method of the body actor itself, and S25 left it out. That chooser is now ported
(`vehicle_anim::Update`), and with it the body asks for the pose grid, which is where both mode-4
writers live; the constructor and the build also gave the grid's thresholds, which turn out to be 1,
5, 10 and 15 mph exactly. The drawing side then grew the two entries it had refused to guess at:
`Clip::pose_four_faded` (a nested slerp and a bilinear lerp over four clips) and
`Clip::pose_pair_faded` (two clips each at its own phase, keeping the original's joint-map
asymmetry), reached through `Set::apply` and the new `Set::apply_vehicle`. 475 tests pass, 4 of them
new: the chooser is oracle-compared over twelve thousand generated frames, the two drawing entries are
proved on fixture clips through the real entry points, and a bike on a hill
with a pad script goes from the start line through the gate and across the grid's bands — two
four-clip blends naming the grid's own rows, weights strictly between them — while a gesture handed
to a gesture reaches the pair at two phases, which has no other caller anywhere in the image. What is
not proved, chiefly the request block's writer and the fade rates, is in the entry's last section.
