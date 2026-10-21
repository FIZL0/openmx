# S74 — putback bones

2026-10-21. Task: `./kit plan S74`.

S74 captured the original's rider body through S66's nine put-backs on free06 and free09: per tick, the
request, the body's mode and machine, the 22 joints and every node. It found that the original holds the
body on the bike for exactly two request fills after a put-back (T and T+1 after the chord, T+1 and T+2
after the wreck clock), and that on a clock put-back's own tick it still draws the thrown body. The port
did neither. Both are now ported as `rider::PutBack` and `Rider::ragdoll_shown`, and proved on all 2695
captured ticks; the held pose matches the original's 22 joints within 1e-4 on all 18 held ticks. The
riding body after the hold is the original's ragdoll on its riding spring, which the port does not draw
(S76). The body's one-tick lag at a wreck's entry is S77. Details in `1-putback-bones-*.md`.
