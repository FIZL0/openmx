# P222 — browser upload cors

2026-11-04. Task: `./kit plan P222`.

Browser Upload for the mod store. HF's CORS lets a page make every request the upload sends (whoami-v2, preupload, commit, LFS batch/verify, multipart close; the S3 PUTs expose ETag). But the browser holds no mods of the player's own to upload until P203, so the task is blocked after P203 and no product code was written. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).
