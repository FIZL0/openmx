# 1 — webkit stage log (READY)

Task P336a. 2026-11-15.

## What I did

- Looked for a page log saved from a ride on beta after P336 (merged 2026-11-13 02:10), the one with a
  `gpu: validation error in <stage>, frame N` line.
- None exists. `~/Downloads/phone-222552-openmx-log.txt` (2026-11-12 22:25, same as
  `context/p336/openmx-log-iphone-2026-11-12.txt`) is the pre-P336 log: it says only
  `validation error in frame 414: encoder state is invalid`, with no stage. The later `phone-1*` logs
  (2026-11-13) are the native iOS app on Metal, which has no such error.
- No code change: without the stage, any fix would be a guess, and P336 already found Chrome accepts every call.

## What the original does

Not applicable (renderer, not simulation).

## How it was proved

Not proved; blocked on the user's log.

## Not verified / not 1:1

- The failing WebKit WebGPU call is not found or fixed: it needs a Safari page log from a post-P336 build,
  and WebKit's WebGPU runs only on Apple hardware, which this machine cannot run.
- Whether the WebGL2 fallback ride works on the iPhone: no log reports it.

## Next

Human: follow HUMAN.md's *Which stage of a ride's frame the iPhone's Safari breaks (P336a)* — ride sx01 on
beta.openmx.dev without `?webgl2`, Save log, put `openmx-log.txt` in `context/p336a/`. Then the next session
reads the `in <stage>` line and fixes that stage's call.
