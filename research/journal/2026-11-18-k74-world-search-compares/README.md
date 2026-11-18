# K74 — world search compares

2026-11-18. Task: `./kit plan K74`.

The fade search's and the sweeps' global search's oracles (mxvatv_bike_ride) spent their time in the
test, not the oracle. Their two whole-RAM compares a case now look only at the pages either machine
wrote since the last match (a whole compare every 100th case still runs), and the world write-back no
longer clones every entry to check it. Each test's CPU time went down 3-26%. What is left is the port's
own read of the 15,417-entry world from RAM every case, and the compare of that world. Details in 1-….
