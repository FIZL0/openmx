# 1 — udp tests run once (FINAL)

Task K81. 2026-11-21. Builds option A from the concept page (the 2026-11-20 K81 journal,
https://claude.ai/artifact/F6dhNXNk8BPzyksfvYrwr7). The person picked A: run the udp socket tests once.

## What I did

- `crates/openmx/src/net/udp.rs`: a test helper `the_bins_copy()` reads `env!("CARGO_CRATE_NAME") == "openmx"`.
- Each of the 11 `net::udp::tests` returns at once unless it is the openmx bin's copy.
- Why: ten test files (online_put_back among them) and the `openmx_web` lib mount `net`. Each copy bound real
  loopback sockets against a 2 s wall-clock `wait`, about 12 copies per full gate, side by side under load.
  Now only the bin's copy does.
- The other copies still list and pass, as no-ops.

## What the original does

Not applicable (test tooling).

## How it was proved

- `./kit check -- -p openmx --test online_put_back --bin openmx net::udp`: 22 tests passed (11 in each binary).
- `strangers_are_still_dropped` waits the full 2 s when it really runs. Run alone:
  - bin's test binary (`target/debug/deps/openmx-0396db5bb42c3c8f`): finished in 2.00 s, so it runs.
  - online_put_back's: finished in 0.00 s, so it returns early.
- That also proves `CARGO_CRATE_NAME` is `openmx` in the bin and not in the mounted copy.

## Not verified / not 1:1

- Which test failed in P383's merge gate. Its log was overwritten (see the 2026-11-20 entry). The fix rests on
  the code reading: the udp tests are the only socket and wall-clock code in the binary.
- No stress run reproduced the red, before or after. A load gate holds the machine for many minutes.
- The bin's own copy still uses the 2 s wall-clock `wait`. It now runs once per gate instead of ~12 times, so it
  races less, but a very busy machine could still starve it. The person picked A over removing the clock.
- I did not time the `openmx_web` lib's copy. Its crate name is `openmx_web`, so it returns early by the same rule.

## Next

None: K81 lands with this.
