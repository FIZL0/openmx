# S290 — s13 start hold

2026-11-11. Task: `./kit plan S290`.

The start hold: the supercross countdown listener pins every rider (free 0,1,0) from the countdown's zero to the warning window. Part 1 ported and proved the hold on a chassis (oracle and sx01 image). S304 wired it into `Sim::race`. Part 2 makes the shell's race listen, and proves a held rider cannot creep and is caught by the gate after the warning.
