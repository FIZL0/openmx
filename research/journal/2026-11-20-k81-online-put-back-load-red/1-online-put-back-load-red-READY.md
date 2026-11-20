# 1 — online put back load red (PART)

Task K81. 2026-11-20. Concept task: options page only, no product code.

Page: https://claude.ai/artifact/F6dhNXNk8BPzyksfvYrwr7

## What I did

- Read `crates/openmx/tests/online_put_back.rs` and the code it drives (`src/net`, `third_party/ggrs`).
- `a_headless_online_put_back_resets_the_lean` has no race I could find:
  - ggrs' clock is driven (`ggrs::clock::drive`, thread-local), and every ggrs and `net/` time read goes through it.
  - The wire is an in-memory `Vec`, three rounds late.
  - ggrs' maps are fixed-key `det::` maps (75bbd8d).
  - The only randomness is ggrs' `rand::random` magic and sync nonces. Their size is fixed, so they can't change timing.
  - The desync dump dir is per pid. Desync dumps' `SystemTime` only names files.
- `./kit check -- -p openmx --test online_put_back` passed alone: **37 tests**, not 2.
- `--list` shows why: the binary mounts `src/net/mod.rs`, `audio.rs` and `vehicle.rs`, so their `#[cfg(test)]` modules run in it too.
  - 14 of them are `net::udp::tests`. They bind real loopback sockets and poll with a 2 s `Instant` deadline (`wait`). That is the only real-socket, wall-clock code in the binary.
  - 10 test binaries mount `net` (markers_online, online_wreck_sound, markers, online_atv, online_put_back, online_car, portals, lobby, netplay, netplay_browser), plus the openmx bin and lib. So each udp test runs about 12 times per full gate.
  - `vehicle::in_play_tests` and the vehicle and audio tests only read the disc or constants.
- Looked for the failing gate's log. The merge slot's `context/notes/check.log` was overwritten at 17:49 by a later run. The plan commit 687e8bb33 is from 17:27. No other copy exists.
- `journalctl` shows no OOM kill between 15:30 and 17:30. One tmux scope peaked at 46 GB.
- The same gate filed K80, K82–K85: six binaries red in one merge gate. That looks like one machine-wide event, not six races.

Options on the page:
- A (pick): the udp socket tests return early unless `env!("CARGO_CRATE_NAME") == "openmx"`, so they run once, in the bin.
- B: the gate keeps a red run's log.
- C: add the binary to `[flaky]`. Rejected, because it hides the cause.

## What the original does

Not applicable (test tooling).

## How it was proved

Not proved. This is a research note: one solo run (37/37 green), and the code reading above.

## Not verified / not 1:1

- Which test in the binary failed in P383's merge gate, and why. The log was overwritten 20 minutes later and nothing else kept it. So option A rests on the code reading alone: the udp tests are the only wall-clock and socket code in the binary. It is not backed by the failure message.
- I did not reproduce a red under load. No stress run was made. A gate holds the machine for many minutes, and this concept task changes no code to test.
- That `env!("CARGO_CRATE_NAME")` reads `openmx` in the bin's unit tests and the test file's name in the mounted copies. This is cargo's documented behaviour, but untested here.

## Next

The person picks an option on the page. The build task does it.
