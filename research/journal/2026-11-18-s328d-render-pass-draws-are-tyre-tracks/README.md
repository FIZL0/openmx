# S328d — render pass draws are tyre tracks

2026-11-18. Task: `./kit plan S328d`.

C33's last unknown draws on the shared generator (1–3 a tick from 17) are the tyre tracks. The
interpreter now runs the original's render: it knows vclip, the clip flags and VI registers, madd, and can
treat the hardware registers as storage. Each row a tyre track lays draws one rand for its height
jitter. With the render run after each frame, the whole frame's draws equal C33's generator steps on
every tick from 4 to 63, and the generator agrees bit for bit through 63 (it was 16). The Sim's side is
S448. Details in 1-render-pass-draws-are-tyre-tracks-FINAL.md.
