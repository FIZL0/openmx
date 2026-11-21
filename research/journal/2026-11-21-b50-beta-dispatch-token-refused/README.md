# B50 — beta dispatch token refused

2026-11-21. Task: `./kit plan B50`.

pages.yml's beta job failed because GitHub refuses the `BETA_DISPATCH_TOKEN` secret (401). It was already refused
before B43, but then it only warned. The job now names that cause instead of blaming an old deploy.yml. Making it
green needs a new token, which only the owner can make (`./kit wizard B50`). Details in the numbered files; the
newest one's suffix says whose move it is (FINAL · PART · READY).
