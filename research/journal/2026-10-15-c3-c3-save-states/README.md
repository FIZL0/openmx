# C3 — c3 save states

2026-10-15. Task: `./kit plan C3`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

C3 settled where save states live and who owns which slot, and proved the parts an agent depends on:
a human's state only reaches an agent through `./kit pcsx2 copy N`, a kit copy's states are per-worktree
and disposable, and save → change something → load really does restore the game. `plan/REFERENCE.md` now
says what each slot is for and what must be written down about it; `plan/HUMAN.md` turns the three states
and the acceptance recording into recipes with times and failure notes. The states that need a controller
(the `sx01` start gate, something that runs itself) are still the human's to make; scratch slot 9 holds an
agent-made main-menu state so routine work does not have to boot from cold.
