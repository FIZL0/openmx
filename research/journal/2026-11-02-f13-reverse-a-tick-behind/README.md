# F13 — the body asks for reverse a tick behind the pad read

2026-11-02. Task: `./kit plan F13`.

P1's 2p reverse restarts in C11's eased capture (34 at 140354, back to 30 at 140397) came 2 ticks
early in the port, so its fade ramp was 0.7997 where the original's was 0.9333. The original's body
asks with the reverse command its pad read raised a tick earlier. The port now does the same
(`PutBack::reverse`), and the 2p replay reads reverse off the pad and speed of two rows back.
Details in `1-reverse-a-tick-behind-FINAL.md`.
