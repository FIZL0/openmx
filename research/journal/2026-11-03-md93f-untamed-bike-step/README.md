# MD93f — untamed bike step

2026-11-03. Task: `./kit plan MD93f`.

This task traced the input that Untamed's (SLUS-21701) backflip and 360 read to the game's shipped
bindings. The ride's feed in `untamed::ride_backflip` and `ride_spin` now has a traced reason for each part:

- the arming button is `trick[2]`, because Untamed's fourth trick action defaults to R1;
- the hold is the step's flick, because it is the same analogue control as SLUS-21104's hold;
- there is no bound control, because no shipped stunt maps one to the backflip.

Before this task those were stand-ins. The code is unchanged and the doc comments now give the reasons.

A new test, `untamed_ride_input_is_the_shipped_bindings`, checks the bindings on the RAM image. One question
is still open: what makes the lip's stick words differ from the limited ones upstream of the bike. Details
in [1-untamed-bike-step-FINAL.md](1-untamed-bike-step-FINAL.md).
