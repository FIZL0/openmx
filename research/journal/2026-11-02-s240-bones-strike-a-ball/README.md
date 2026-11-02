# S240 — a thrown rider's bones strike a beach ball

2026-11-02. Task: `./kit plan S240`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

S197b left the rider's ragdoll out of the props. A PCSX2 capture of free09's ball 1 dropped on the
human's lying rider (drop2) shows the original's way: the bones' search lists every awake ball's mesh
collidable (the moving structure), a bone's sphere is paired with the ball's `f.col` on the same record
as the wreck bike and the cars, the push-out measures the bone against the ball's body, and the hit fires
the ball's struck listener (wake). The ball's own rays never strike a bone. Ported as
`ragdoll_scene::KnockedProp` in the bones' scene, fed the props the ride is lent for its frame. The list's
distances, the sphere test and the capture's own contact (bone 2 at 2841) are bit-exact. Details in
`1-bones-on-the-ball-FINAL.md`.
