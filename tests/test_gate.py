#!/usr/bin/env python3
"""Self-test for the whole-workspace gate (ps2kit/cargo_gate.py), on a throwaway cargo workspace.

    python3 tests/test_gate.py          (needs cargo; no kit.toml, no network, about ten seconds)

What it holds the gate to: the binaries really run at the same time (an `[alone]` one after them, a failed one rerun holding the machine), a `[[slow]]` binary is left out
until the work touches its paths (or `--all`), a `[flaky]` failure gets one lone rerun and counts as a
pass only when that rerun passes, and a real failure still fails, with cargo's own rerun line in the log.
"""
from __future__ import annotations

import os
import shutil
import subprocess as sp
import sys
import tempfile
from contextlib import contextmanager
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from ps2kit import cargo_gate  # noqa: E402

ok = fail = 0


def check(cond, what: str) -> None:
    global ok, fail
    print(('  ok   ' if cond else '  FAIL ') + what)
    ok, fail = ok + bool(cond), fail + (not cond)


def g(*a: str, cwd):
    return sp.run(['git', *a], cwd=str(cwd), capture_output=True, text=True)


# Each binary notes when it started and ended, so the test can see whether two of them overlapped.
STAMP = '''
fn stamp(what: &str) {
    use std::io::Write;
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stamps");
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p).unwrap();
    writeln!(f, "{what} {at}").unwrap();
}
'''

FILES = {
    'Cargo.toml': '[workspace]\nmembers = ["a"]\nresolver = "2"\n',
    'a/Cargo.toml': '[package]\nname = "a"\nversion = "0.1.0"\nedition = "2021"\n',
    'a/src/lib.rs': 'pub fn two() -> u32 { 2 }\n',
    'a/tests/one.rs': STAMP + '#[test]\nfn waits() { stamp("one+"); '
                      'std::thread::sleep(std::time::Duration::from_millis(1500)); stamp("one-"); }\n',
    'a/tests/two.rs': STAMP + '#[test]\nfn waits() { stamp("two+"); '
                      'std::thread::sleep(std::time::Duration::from_millis(1500)); stamp("two-"); }\n',
    'a/tests/slow.rs': '#[test]\nfn proves_the_slow_thing() { assert_eq!(a::two(), 2); }\n',
    # Fails while ../flake exists (and removes it), as a test does that loses its port to another slot.
    'a/tests/net.rs': '#[test]\nfn binds_a_port() {\n    let f = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../flake");\n'
                      '    if f.exists() { std::fs::remove_file(&f).unwrap(); panic!("Address already in use"); }\n}\n'
                      '#[test]\nfn is_fine() {}\n',
    'gate.toml': '[[slow]]\ntest = "-p a --test slow"\npaths = ["a/src/*"]\n\n'
                 '[flaky]\ntests = ["--test net::binds_a_port$"]\n',
}


def test_gate() -> None:
    if not shutil.which('cargo'):
        print('  skip  no cargo on PATH')
        return
    with tempfile.TemporaryDirectory() as tmp:
        w = Path(tmp)
        for rel, text in FILES.items():
            (w / rel).parent.mkdir(parents=True, exist_ok=True)
            (w / rel).write_text(text)
        g('init', '-q', '-b', 'main', cwd=w)
        (w / '.gitignore').write_text('target/\nstamps\nflake\ntimes.json\nlog\n')
        g('add', '-A', cwd=w)
        g('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', 'base', cwd=w)
        g('checkout', '-qb', 'task/T1', cwd=w)
        env = {**os.environ, 'CARGO_TARGET_DIR': str(w / 'target'), 'RUST_TEST_THREADS': '4'}

        held = []  # one entry each time the gate takes the machine (checks.alone's place here)

        @contextmanager
        def machine(also=None):
            held.append(1)
            if also:
                also()
            yield

        def gate(all_slow=False, alone=machine):
            (w / 'stamps').unlink(missing_ok=True)
            held.clear()
            return cargo_gate.run('cargo test -q --message-format=short', w, env, 4, w / 'log', 'main',
                                  all_slow, w / 'times.json', alone=alone)

        check(cargo_gate.applies('cargo test -q --message-format=short', []), 'a bare cargo test is a whole run')
        check(not cargo_gate.applies('cargo test -q', ['-p', 'a']), 'a filtered one is left to cargo')

        rc, notes = gate()
        log = (w / 'log').read_text()
        check(rc == 0, f'an untouched branch passes (rc {rc})')
        check('-p a --test slow' not in log, 'the slow binary did not run')
        check(any('not run, untouched: slow' in n for n in notes), 'and the verdict says so')
        s = {k: float(v) for k, v in (l.split() for l in (w / 'stamps').read_text().splitlines())}
        check(s['one+'] < s['two-'] and s['two+'] < s['one-'], 'two binaries ran at the same time')

        (w / 'a/src/lib.rs').write_text('pub fn two() -> u32 { 1 + 1 }\n')
        rc, notes = gate()
        check(rc == 0 and '-p a --test slow' in (w / 'log').read_text(), 'touching its paths runs the slow one')
        g('checkout', '-q', '--', 'a/src/lib.rs', cwd=w)
        gt = (w / 'gate.toml').read_text()
        (w / 'gate.toml').write_text(gt + '\n[alone]\ntests = ["-p a --test two"]\n')
        rc, _ = gate()
        s = {k: float(v) for k, v in (l.split() for l in (w / 'stamps').read_text().splitlines())}
        check(rc == 0 and s['two+'] >= s['one-'], 'an [alone] binary runs after the pool, not beside it')
        check(not held, 'and one that passes never takes the machine')
        (w / 'gate.toml').write_text('[alone]\ntests = ["-p a --test net"]\n')
        (w / 'flake').write_text('')
        rc, notes = gate()
        check(rc == 0 and held == [1], f'an [alone] failure reruns once holding the machine, and passes (rc {rc}, held {held})')
        check(any('machine to itself' in n and 'binds_a_port' in n for n in notes), 'and the verdict names it')
        (w / 'flake').write_text('')
        rc, _ = gate(alone=lambda: machine(lambda: (w / 'flake').write_text('')))
        check(rc == 101 and 'failed again' in (w / 'log').read_text(), 'one that fails with the machine to itself fails the gate')
        (w / 'gate.toml').write_text(gt)
        rc, notes = gate(all_slow=True)
        check(rc == 0 and '-p a --test slow' in (w / 'log').read_text(), '--all runs it untouched')

        (w / 'flake').write_text('')
        rc, notes = gate()
        check(rc == 0, f'a known flake that passes alone passes the gate (rc {rc})')
        check(any('binds_a_port' in n for n in notes), 'and the verdict names it')

        (w / 'gate.toml').write_text('[flaky]\ntests = []\n')
        (w / 'flake').write_text('')
        rc, notes = gate()
        log = (w / 'log').read_text()
        check(rc == 101, f'the same failure, not listed, fails (rc {rc})')
        check('error: test failed, to rerun pass `-p a --test net`' in log, "with cargo's rerun line")
        check('Address already in use' in log, 'and its panic in the log')

        (w / 'a/src/lib.rs').write_text('pub fn two() -> u32 { nope }\n')
        rc, _ = gate()
        log = (w / 'log').read_text()
        check(rc != 0 and 'error[E0425]: cannot find value `nope`' in log, 'a build error fails, its message in the log')
        check('to rerun pass' not in log, 'and no test reads as having run')


if __name__ == '__main__':
    test_gate()
    print(f'\n{ok} passed, {fail} failed')
    sys.exit(1 if fail else 0)
