# S150 — trick landing spin

2026-10-26. Task: `./kit plan S150`.

The user saw a badly landed trick fling the bike round to face the other way instead of bailing. The
original does the same. Its landing crossup is an arcsine, so a landing more than about 135° crossed up
reads as nearly straight against the 45° limit and rides out, and the tyres then swing the bike round
135–167° over three seconds. The port already matches it: three captures (118° wrecks with reason 3;
148° and 169° ride out and swing) replay in the port with the same wreck ±1 tick, and the heading stays
within 6° of the original's. There is a new test in tests/replay.rs and no simulation change. S153 is
filed for a bail option, if the user wants one. Details in 1-trick-landing-spin-FINAL.md.
