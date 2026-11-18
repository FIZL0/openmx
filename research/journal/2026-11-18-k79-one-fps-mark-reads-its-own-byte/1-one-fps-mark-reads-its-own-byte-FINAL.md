# 1 — one fps mark reads its own byte (FINAL)

Task K79. 2026-11-18.

## What I did

- The "1 bytes read" was the counter's own look, not a read of the ride's. The shell's mark
  (`stages.rs` `bytes_read_mark`) read `/proc/self/io` twice and took the size of the first look for the
  size of the next. But the file's text grows a byte whenever any counter in it (`rchar`, `syscr`) gains
  a digit. So when the load's reads ended within about one look (~100 bytes) of a power of ten, the
  second look was one byte longer than the first, and a ride that read nothing said "1 bytes read".
- That is set by how many bytes the load read, which is the same run after run in one checkout: why it
  failed three times in a row in slot 5 and never here.
- Now the mark reads the file once and takes its `rchar` plus the length of the text it read. That is
  exact: `rchar` in the text is the count before the read, and the read takes the whole text.
- Two unit tests in `stages.rs`: the mark's arithmetic on a text with a counter at the edge of a digit,
  and a live mark followed by a look counting nothing.
- `tests/one_fps.rs` is unchanged: the check is still "0 bytes read", which is now exact.
- Nothing taken from `../opentennis`: the bug is in this shell's own counter.

## What the original does

Not applicable: kit/test tooling.

## How it was proved

- `mark_probe.py` (this folder) drives a fresh process's `rchar` to `1000000 - g` for g = 0..300 off
  `/dev/zero`, makes a mark and looks again with nothing read between:
  - the old two-look mark: 96 of 301 said 1 byte read, 205 said 0;
  - the new one-look mark: 301 of 301 said 0.
  Run: `for g in $(seq 0 300); do python3 -I mark_probe.py $((1000000-g)) old; done | sort | uniq -c`.
- Six headless 4-player rides here, then 16 in parallel: all "0 bytes read" (before the change too, as
  this checkout's load does not land near a power of ten).
- `./kit check -- -p openmx --test one_fps` and the `stages::` unit tests pass.

## Not verified / not 1:1

- Not rerun in slot 5 itself: another task's work is in that checkout. The cause is proved by the probe
  above, not by a rerun of the failing slot; the slot's own `rchar` at the mark was not recorded.

## Next

none
