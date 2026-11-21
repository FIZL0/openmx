# 1 — link failure not flaky (FINAL)

Task K82. 2026-11-20. Concept task: options page https://claude.ai/artifact/HRQTe7bGEtuj49GARrjLdy

## What I did

- Read the P383 merge gate's output: `context/notes/merge-gate.log`, `check-merge2.log`,
  `gates/e02372818c50…-3dc977c3977c.failed`, and `run.log` 11-20 17:17–17:27.
- Finding: car_modifiers never ran. The build-ahead gate in merge2 printed `BUILD FAILED`, and nine
  openmx test binaries failed at the link step (`linking with x86_64-linux-gnu-gcc failed: exit status: 1`):
  bays, ride_hud, online_put_back, car_modifiers, freestyle_event, overlay, netplay, lobby and loading.
  The repair session called them "a flaky test, no clash". The rerun linked and passed, so the runner
  filed K80–K85.
- merge2's `car_modifiers-e947c02f1e01db23` objects are dated 11-20 17:20, but the binary is dated 11-18
  22:22. So the link really did fail in merge2.
- `tests/car_modifiers.rs` and the vehicle.rs unit tests it mounts (27 tests) are pure: a fixed-step
  sim, disc files read-only through `Disc::open_configured`, no clock, ports or written files. It passes
  alone: `./kit check -- -p openmx --test car_modifiers` gave 27 passed.
- The linker's own stderr is lost. `ps2kit/cargo_gate.py` `build` uses
  `--message-format=json-diagnostic-short` and writes each message's short `rendered` text. That drops
  the link error's child notes, which hold the linker's output.
- Recurrence: 7 `gates/*.failed` files since 11-15 hold link errors. 3 of them (11-18 07:50, 11-18
  10:29, 11-20 17:21) came while a merge2 "built ahead" gate ran beside the merge's gate.
- Ruled out: kernel OOM (`journalctl -k` has no oom lines on 11-20), the runner's memory watch (no
  "ended pid" line then), disk space (1.1 TB free), the runner's temp sweep (no sweep near any failure),
  rlimits (none; overcommit 0). Test binaries are about 37 MB (`debug = "line-tables-only"`).
- Options on the page: A, log the linker stderr, relink once when only links failed, and give it its own
  headline (recommended). B, log only. C, a machine-wide lock around the gate's build step.

## What the original does

Not applicable (kit tooling).

## How it was proved

- Gate logs and the binaries' file times, as listed above. No code changed.

## Not verified / not 1:1

- Why the linker failed: unknown. The gate's short message format dropped the linker's stderr, and no
  other record of it exists. Memory, two gates linking at once, and files changing under ld are guesses.
- Whether the four failures with no merge2 gate beside them overlapped a slot's `done` gate: not checked.
  The slots' check logs are overwritten.
- I did not try to reproduce the link failure under load. A reproduction needs two full gates at once,
  and those belong to the runner, not to this slot.

## Next

A person picks an option on the page. The build task then changes `ps2kit/cargo_gate.py`,
`ps2kit/checks.py` and `tests/test_gate.py`. K80, K81, K83, K84 and K85 share this cause: close them with
it, or note it on them.
