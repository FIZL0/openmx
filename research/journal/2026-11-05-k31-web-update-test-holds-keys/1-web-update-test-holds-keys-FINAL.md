# 1 — web update test holds keys (FINAL)

Task K31. 2026-11-05.

## What I did

`tests/web.rs`'s `a_new_build_shows_on_the_menu_not_the_ride` went red in K25's gate: the main menu said both
of its lines ("offers v0.0.2", "Updated to …") but Square did not reload the page into the newer build. The
merge-gate log cuts the JSON off before `reloaded`, so what the footer read is not known. The driver,
`tests/web/update.mjs`, had two wall-clock races on that path, and both are gone now:

- **Enter past the splash was a tap repeated on a clock**: press, 1.5 s rest, press again until the main
  menu's two console lines reach node. If those lines lag a gap behind the screen (a busy machine, the
  worker → CDP → node path), the next Enter lands on the main menu and opens Free Ride (`MAIN_FREE`, row 0).
  Square there is not the reload, which is this exact failure. Now `hold()` keeps Enter down until both
  lines have come, then lets it go. Menu presses are edges and only the d-pad repeats (`menu.rs`
  `REPEAT_AFTER_US`/`DIRS`), so a held key is one press however slow the frames are, and nothing presses again.
- **The reload was detected with two evaluates**: `window.openmxBefore` and then the footer's text. An
  evaluate that fails as the navigation starts gives `undefined`, the same answer as a new document. The
  footer read that followed could then still hit the old document (its `Build v0.0.1 1111111.`), and the
  loop broke out on that. Now one expression reads both: `null` in the old document, a string in the new
  one, and nothing (a failed or lost answer, raced against 2 s) is never taken as the new document. Square
  (V) is held down the same way until the new footer reads, so a frame too slow to see a short tap cannot
  lose it either. The key-up then goes to the new document, which ignores it (`started()` is false).

The old `key()` tap helper is gone: it had no other callers. The timeouts are not wider. Each wait is now
on the event it needs, and its deadline (60 s) only bounds a dead page.

Nothing taken from opentennis: its page tests have no update notice of this shape.

## What the original does

Nothing here is the original's: a test driver for the page's front end.

## How it was proved

- The old script passes alone, pinned to 2 cores, and on 1 core shared with two busy loops; with the
  re-press gap cut to 900 ms and to 200 ms it passes too. So the Enter race needs a console line late by
  more than a gap. I could not bring that about here, and the K25 run is the only time it was seen.
- The new script passes alone and on 1 core shared with two busy loops (`taskset -c 0`). The JSON has
  `"reloaded":"Build v0.0.2 2222222."` and the main menu's two lines. This also proves that an Enter held
  from the splash onto the main menu is not read there as a new press: if it were, Free Ride would open and
  the Square reload would fail.
- `./kit check --all` through `./kit done`.

## Not verified / not 1:1

- Which of the two races K25's run hit: the gate log truncates the JSON before `reloaded`, and neither race
  showed up on demand here (see above). Both are removed, so either way the test no longer depends on the
  clock. A third cause outside the script (the reload itself taking over the old 20 s, under load) would
  now be bounded at 60 s rather than fixed.
- The failed-evaluate-during-navigation path was reasoned from CDP's behaviour (an evaluate in a context
  the navigation destroys answers with an error, which `evaluate()` maps to `undefined`). It was not
  forced in a run.

## Next

-
