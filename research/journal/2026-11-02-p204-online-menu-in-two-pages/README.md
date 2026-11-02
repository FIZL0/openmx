# P204 — online from the game's own menu, in two browser pages

2026-11-02. Task: `./kit plan P204`.

A test now drives the shell's own Online menu in the browser, not the `?join=`/`?host=` page. Two headless
chromium pages, each in its own browser context, pick the disc and start on `--screen online`. With keys
only, one page hosts and the other joins by the room code, typed on the on-screen keyboard. The guest
readies, the host starts, and both ride with the gas held through a loopback relay. They say the same
settled hash before ticks 300 and 600, with no desyncs. It passed on its first run, in 80 s. To see the
room code and the hashes, the browser build now writes the lobby's events and the settled ride's hash
to the page console. Details are in the numbered file.
