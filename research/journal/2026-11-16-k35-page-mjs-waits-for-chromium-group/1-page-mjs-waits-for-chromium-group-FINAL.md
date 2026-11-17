# 1 — page mjs waits for chromium group (FINAL)

Task K35. 2026-11-16.

## What I did

- `crates/openmx/tests/web/page.mjs` now quits as `online.mjs` does since K33: chromium runs in its own process
  group (`detached`), quit sends `Browser.close` over DevTools (or a kill before DevTools is up), waits for the
  whole group to go, SIGKILLs what is left, then removes the profile. A removal that still throws is said on
  stderr and does not change the exit code.
- Waits are 3 s + 1 s, not online.mjs's 10 s + 5 s: `netplay_browser.rs`'s `Node` drop kills page.mjs 5 s after
  closing its stdin, and a killed page.mjs would leave the detached group running.
- `online.mjs` unchanged: it already had the wait; sharing the code would mean a new module both import, for
  ~30 lines.

## What the original does

Not applicable: test tooling.

## How it was proved

`./kit check -- -p openmx --test netplay_browser a_desktop_and_a_browser`: passed, 1 test. `/tmp` had 45
`openmx-page-*` dirs before and 45 after, so this run left no profile behind (the 45 are from earlier runs).

## Not verified / not 1:1

Everything this task left unproven or inexact goes here, each with its reason — or the single word `none`.
A reader must be able to tell from this section alone what they may not trust yet.

- The ENOTEMPTY itself was not reproduced for page.mjs: it showed under load in online.mjs (K33), and the fix is
  the same one; one run on a busy machine (3 other check slots) passed.
- Whether that run skipped (no pkg/ or chromium) is not shown by a passing check; the profile count is unchanged
  either way.

## Next

Nothing.
