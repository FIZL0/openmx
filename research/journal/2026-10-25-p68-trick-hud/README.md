# P68 — trick hud

2026-10-25. Task: `./kit plan P68`.

The original's HUD (`--overlay original`) gained what it lacked: the Stunt movie's trick name over its
points as a trick lands (`BackFlip + Taunt`, `10000  X 2`, `2 Stunt Combo Total` / the banked total),
the camera preset's name for 3 s after a press, and the banked score. All of it is per view, so splits
and online rides show each player their own. The texts are the original setter's formats. A replay of
recording2's captured score card proves them, and their size and place match a PCSX2 frame. The work is
done, but the task is not ticked: main's gate is red on S119's replay test, which is just as red on bare
HEAD. Entry 1 is PART until S119 lands.
