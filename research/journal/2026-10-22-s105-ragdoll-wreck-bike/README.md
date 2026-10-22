# S105 — ragdoll wreck bike

2026-10-22. Task: `./kit plan S105`.

In a ride, a bone's sphere now hits the wreck bike's mesh (`wreckbikef.col`), answered with the bike's
own body, instead of meeting an open scene. The bones' one search for the bike is ported (the track's
tree, the rebuild bound, the ignore until the bones are off the bike, each sphere's reach). Over the
five s62 wrecks it pairs every sphere with the bike exactly when the original does: 24,822 sphere
tests, 600 pairs and all 64 hits bit-equal. The bike leaving the broad phase late in a wreck is filed
as S106. Details in part 1.
