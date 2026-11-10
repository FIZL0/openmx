# S282a — s12 ai target seeker

2026-11-10. Task: `./kit plan S282a`.

This task ported the free-ride target seeker inside `ai_tick_flag` as `ai::seek`: how an AI rider with no
path picks a jump, rides to its setup point, lines up, launches, and learns from where it landed. It
covers target picks from a sequence, a course or a pick list, approach states 1/2/3, the landing learning,
the crossing test, and the vt+0x80/vt+0x94 reach and hand-off. It matches the original bit for bit over
10,000 generated ticks. It is not yet compared on a real world (recording2) and not yet wired into the
tick. Details in `1-s12-ai-target-seeker-FINAL.md`.
