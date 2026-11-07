# P315a — fliers preview

2026-11-07. Task: `./kit plan P315a`.

The Garage's Fliers tab now turns the seat's flier on the stage, for the kind under the cursor (the plane from the Player row). A pack's skin is drawn in its `_ui` garage model where the pack has one, else in its ride model; MX Unleashed's is drawn in the disc's model. Round arrows in the stage's corners step the skin on a tap or click. The front end now passes window clicks to `Menu::click`, which also makes P277's row taps work on the main menu. Checked by menu shots of `--screen fliers` on a renamed pack's skin and on MX Unleashed's, and by menu and dress tests. No phone run.
