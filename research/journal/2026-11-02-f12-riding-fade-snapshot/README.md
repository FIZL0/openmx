# F12 — the riding fade's snapshot is the drawn ragdoll

2026-11-02. Task: `./kit plan F12`.

V109 found that the original's restart snapshot copies the joints' transform nodes, which the
riding ragdoll writes over after the blend. The port snapshotted its own blend buffer
(`Set::rewind`, `skeleton.nodes(pose)`). The body now keeps the node tree the shell last drew from
the riding ragdoll (`Body::drawn`). A restart or put snapshots that tree (`Set::rewind_drawn`), and
falls back to the clip pose when the body is drawn from its clips. Details in
`1-riding-fade-snapshot-FINAL.md`.
