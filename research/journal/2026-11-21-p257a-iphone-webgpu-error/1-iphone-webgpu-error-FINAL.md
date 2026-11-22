# 1 — iphone webgpu error (FINAL)

Task P257a. 2026-11-21.

## What I did

- Drove the iPhone 15 Pro on omarchy-desktop (iFizBox's `tools/ui-15.sh`, lock `P257a`, given back after).
  Safari: iOS 18.7 user agent, Safari 27.0.1. Page: `https://beta.openmx.dev/?fps`, build 998d107, which has P257,
  P257b and P336. No `?webgl2`.
- Read the page's log with no Web Inspector window: go-ios 1.3.2's
  `ios webinspector list` gives the tab's page id, then
  `ios webinspector eval <id> "openmxLog.lines.join(String.fromCharCode(10))"` returns every kept line, the
  worker's included (run from `~/.local/share/ifizbox-rig`, `--udid=00008130-0018450C3661001C`).
- The page needs the disc. Pushed the ISO into openMX's Documents (`ios fsync --app=io.github.fizl0.openmx push`,
  2.2 GB in 43 s). Safari's file picker then finds it under On My iPhone > openMX. The page copied it into its
  own storage; the Documents copy is removed again.
- The page wants landscape. WebDriverAgent turns the phone:
  `POST 127.0.0.1:8100/session/<sid>/orientation {"orientation":"LANDSCAPE"}` (sid from `/status`).
  Put back to portrait after.
- Touch HUD: Start (X) → main menu → Free Ride → Dallas → ride. Held Gas 20 s; the bike rode off in 2nd gear.
- Log: `research/p257a/openmx-log-iphone-2026-11-21.txt`. Adapter limits are the same as P336's log (2D 16384,
  layers 2048, buffer 1 GiB, 44 sampled textures, 22 samplers, 11 bind groups, 31 inter-stage variables).
  `gpu: apple (BrowserWebGpu) into a Bgra8Unorm canvas (webgpu)`, `renderer: gpu, webgpu`, and **no
  `gpu: validation error` line** at all: not in the splash, the menus, the track list, the load or the ride.
  No `page loaded again` line, so the page never fell back to `?webgl2`.

Result: the 11-02 front-end error (`gpu: device mismatch`) does not happen on today's build. There is no failing
call to fix, so no change to `gpu.rs` and no test for one. Something between 11-02 and 998d107 fixed it; likely
candidates are P336 (2180c9990: each frame stage its own encoder and error scope on a browser's WebGPU) and
P377 (1b763cc59: every pipeline warmed at the load), but I did not bisect.

## What the original does

Not applicable: rendering on the web.

## How it was proved

Looked: screenshots of the drawn menu and ride on the phone (not committed, they show the game), and the page's
own log read off the phone (committed above), with no validation error line.

## Not verified / not 1:1

- Which commit fixed the 11-02 error: not bisected. Old builds are not on beta any more, and the original log
  line was never captured (P257b's log came after).
- Only one ride (Dallas Free Ride, about 30 s). P336a's sx01 race error at frame 414 is a separate task and was
  not tried here.
- The home-screen app (installed web app, no address bar) was not opened; only a Safari tab.
- The phone's iOS is reported as 18.7 in the user agent with Safari 27.0.1, the same as the user's 11-12 log;
  other iOS versions not tried.

## Next

None for P257a. P336a can use the same recipe to read a ride's log off the phone.
