# P100 — host changes modifiers online

2026-10-22. Task: `./kit plan P100`.

Online, the host changes the game modifiers from the pause, and every machine takes them at one tick. The
change goes out with a session restart, the way N10's joins do: `Msg::Mods` is sent ahead of it, and
every machine decodes the settled state and then moves it onto the new set. Two netplay tests prove both
peers stay in sync, match an offline ride changed at that tick, and that a later joiner gets the set too.
The shell's pause path is compiled but was not driven live. Done: see 1-FINAL.
