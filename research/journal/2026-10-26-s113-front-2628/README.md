# S113 — front 2628

2026-10-26. Task: `./kit plan S113`.

The s62 front wreck parted from its capture at 2628, and the cause was the replay, not the game. Laying
each captured row into the Ee moves the wreck bike, but its place in the track's broad-phase tree stays
where the first state had it. From 2619 the laid-in pools also empty its old node. At 2628 the bones'
search is rebuilt, the tree holds the bike nowhere, and no bone is paired with it. In the game the bones
hit it. `place_bike` now puts the bike in the tree each row using the original's own bounds, key and
insert. With that, the original's update holds front's capture through 2888 (it was 2627), and the port
replays it bit for bit. A new test shows the lead. 2889 parts for another reason and is filed as S139.

Details: 1-front-2628-FINAL.md.
