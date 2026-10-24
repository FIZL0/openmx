# V60 — see through sweep

2026-10-23. Task: `./kit plan V60`.

I swept every track and the bike and rider archives for materials the port drew see-through where the original
draws them solid. I found two rules. First, two-texture (glossy) passes read only `_1`-suffixed keys, so the gloss
mask in the base texture's alpha is not an alpha test. That fixes the helmet, goggles, bike plastics, plow and doors.
Second, a blend with A = B and D = Cs is opaque, which fixes free07's cabins. Both are fixed in the parser, proved on
both renderers by tests/solid.rs, and checked against the original's goggle strap. See 1-see-through-sweep-FINAL.md.
