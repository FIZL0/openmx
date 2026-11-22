# MD250 — part reader box

2026-11-21. Task: `./kit plan MD250`.

The Untamed race quad made from its files had its collision record's inverse inertia (+0x70) about 36 ulps
off MD147's spawn image, so the bale test skipped that field. The first session
(research/journal/2026-11-18-md250-start-spot) found the start spot the part reader reads the model at,
which fixed the mounts but not the box. This session hooked Untamed's box function live, at the moment the
part reader calls it for the quad, and found the cause: Untamed's world update makes each model node's
frame orthonormal again, which `quad::Model::half_box` did not do. `quad::Model::untamed_half_box` does,
and the record now matches bit for bit; the bale test's exception is gone. Details in
1-part-reader-box-FINAL.md.
