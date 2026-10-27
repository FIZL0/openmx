# S116 — quat sign flip

2026-10-27. Task: `./kit plan S116`.

The original's chassis quaternion changes sign at tick 130 of S58's turn, and the task was to find what
writes it after the integrate and port it. It was already ported: S67 (285d7f91) added the stunt clocks'
copy-back, which ends every frame on `mat4_to_quat` of the node's matrix. At tick 130 the largest of that
function's four candidates moves from `y` to `w`, so the sign changes. On HEAD the turn replays the flip
bit for bit, quaternion and history, and agrees on every lane through tick 172. No new code: S58's
ratchets are raised (all four cases' slip flags now agree on every tick), and the turn's next parting
(tick 173) is filed as S177.
