# MD81d — untamed baja

2026-11-14. Task: `./kit plan MD81d`.

Untamed's baja truck: diff SLUS-21701's car step against SLUS-21104's, port it off by default, capture it,
pack it and replay a captured drive bit-exact. Part 1 diffed and ported the step behind `Load::untamed`;
the capture (MD164), the pack (MD163) and the replay (MD163a) were split off and have landed. Part 3
checked the task's test is met: MD163a's seven baja drives replay bit-exact on the port.
