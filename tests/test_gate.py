#!/usr/bin/env python3
"""Self-test for the whole-workspace gate (ps2kit/cargo_gate.py), on a throwaway cargo workspace.

    python3 tests/test_gate.py          (needs cargo; no kit.toml, no network, about ten seconds)

What it holds the gate to: the binaries really run at the same time (an `[alone]` one after them, a failed one rerun holding the machine), a `[[slow]]` binary is left out
until the work touches its paths (or `--all`), a `[flaky]` failure gets one rerun holding the machine and counts as a
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
    // one write per line: writeln! may split it, and two binaries' lines then interleave mid-line
    f.write_all(format!("{what} {at}\n").as_bytes()).unwrap();
}
'''

FILES = {
    'Cargo.toml': '[workspace]\nmembers = ["a"]\nresolver = "2"\n',
    'a/Cargo.toml': '[package]\nname = "a"\nversion = "0.1.0"\nedition = "2021"\n\n[features]\nnative = []\n',
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
    # The variant's own tests run only where it has its feature; the other one would fail there, so it
    # failing says the filter let it in. ../perturb makes the variant's test fail, as a changed constant would.
    'a/tests/var.rs': '#[test]\nfn native_tolerance_bound() {\n    if !cfg!(feature = "native") { return; }\n'
                      '    assert!(!std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../perturb").exists(), "past the bound");\n}\n'
                      '#[test]\nfn exact_only() { assert!(!cfg!(feature = "native")); }\n',
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
        check(held == [1], f'and its rerun held the machine (held {held})')

        (w / 'gate.toml').write_text('[flaky]\ntests = []\n')
        (w / 'flake').write_text('')
        rc, notes = gate()
        log = (w / 'log').read_text()
        check(rc == 101, f'the same failure, not listed, fails (rc {rc})')
        check('error: test failed, to rerun pass `-p a --test net`' in log, "with cargo's rerun line")
        check('Address already in use' in log, 'and its panic in the log')

        variant = '[[variant]]\nfeatures = "a/native"\ntarget_dir = "target/var"\nfilter = "native_tolerance"\n' \
                  'tests = ["-p a --test var"]\n'
        (w / 'gate.toml').write_text(variant)
        rc, _ = gate()
        log = (w / 'log').read_text()
        args = '-p a --features a/native --target-dir target/var --test var native_tolerance'
        check(rc == 0 and f'({args},' in log, f'a [[variant]] binary runs, filtered, with its features (rc {rc})')
        check((w / 'target/var').is_dir(), 'built into its own target directory')
        (w / 'perturb').write_text('')
        rc, _ = gate()
        log = (w / 'log').read_text()
        (w / 'perturb').unlink()
        check(rc == 101 and f'error: test failed, to rerun pass `{args}`' in log and 'past the bound' in log,
              'a variant test that fails fails the gate, with a rerun line that rebuilds just it')
        check('rerun pass `-p a --test var`' not in log, 'and the plain build of the same binary still passes')

        (w / 'a/src/lib.rs').write_text('pub fn two() -> u32 { nope }\n')
        rc, _ = gate()
        log = (w / 'log').read_text()
        check(rc != 0 and 'error[E0425]: cannot find value `nope`' in log, 'a build error fails, its message in the log')
        check('to rerun pass' not in log, 'and no test reads as having run')


# `[select]`: a uses nothing, b uses a, c stands alone; b's `names` test reads oracle.txt's "alpha".
PICK = {
    'Cargo.toml': '[workspace]\nmembers = ["a", "b", "c"]\nresolver = "2"\n',
    'a/Cargo.toml': '[package]\nname = "a"\nversion = "0.1.0"\nedition = "2021"\n',
    'a/src/lib.rs': '/// ```\n/// assert_eq!(a::two(), 2);\n/// ```\npub fn two() -> u32 { 2 }\n',
    'a/tests/one.rs': '#[test]\nfn one() { assert_eq!(a::two(), 2); }\n',
    'a/tests/other.rs': '#[test]\nfn other() {}\n',
    'b/Cargo.toml': '[package]\nname = "b"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\na = { path = "../a" }\n',
    'b/src/lib.rs': 'pub fn four() -> u32 { a::two() * 2 }\n',
    'b/tests/uses.rs': '#[test]\nfn uses() { assert_eq!(b::four(), 4); }\n',
    'b/tests/names.rs': '#[test]\nfn names() { let _ = "alpha"; }\n',
    'c/Cargo.toml': '[package]\nname = "c"\nversion = "0.1.0"\nedition = "2021"\n',
    'c/src/lib.rs': 'pub fn one() -> u32 { 1 }\n',
    'c/tests/alone.rs': '#[test]\nfn alone() { assert_eq!(c::one(), 1); }\n',
    'research/oracle.txt': '# names\nalpha boot 0x00100000 one\nbeta boot 0x00100010 two\n',
    'research/c9/rows.csv': 'a,b\n',
    'gate.toml': '[select]\noracle = "research/oracle.txt"\ndata = ["research"]\nignore = ["*.md"]\n',
}


def test_select() -> None:
    if not shutil.which('cargo'):
        print('  skip  no cargo on PATH')
        return
    with tempfile.TemporaryDirectory() as tmp:
        w = Path(tmp)
        for rel, text in PICK.items():
            (w / rel).parent.mkdir(parents=True, exist_ok=True)
            (w / rel).write_text(text)
        g('init', '-q', '-b', 'main', cwd=w)
        (w / '.gitignore').write_text('target/\ntimes.json\nlog\nCargo.lock\n')
        g('add', '-A', cwd=w)
        g('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', 'base', cwd=w)
        g('checkout', '-qb', 'task/T2', cwd=w)
        env = {**os.environ, 'CARGO_TARGET_DIR': str(w / 'target'), 'RUST_TEST_THREADS': '4'}

        def ran(edit: dict, all_slow=False, since='main', notes=None, keep_slow=False) -> set[str]:
            for rel, text in edit.items():
                (w / rel).write_text(text)
            rc, got_notes = cargo_gate.run('cargo test -q --message-format=short', w, env, 4, w / 'log', since,
                                           all_slow, w / 'times.json', keep_slow=keep_slow)
            if notes is not None:
                notes[:] = got_notes
            log = (w / 'log').read_text()
            g('checkout', '-q', '--', '.', cwd=w)
            for rel in edit:
                if not g('ls-files', '--error-unmatch', rel, cwd=w).returncode == 0:
                    (w / rel).unlink()
            check(rc == 0, f'passes (rc {rc})')
            got = {l.split('(', 1)[1].split(',')[0] for l in log.splitlines() if l.startswith('     Running ')}
            return got | ({'doc'} if '   Doc-tests' in log else set())

        got = ran({'c/src/lib.rs': 'pub fn one() -> u32 { 2 - 1 }\n'})
        check(got == {'-p c --lib', '-p c --test alone', 'doc'}, f'a crate nothing uses: its own binaries ({sorted(got)})')
        got = ran({'a/tests/one.rs': '#[test]\nfn one() { assert_eq!(a::two(), 1 + 1); }\n'})
        check(got == {'-p a --test one'}, f'a test file: that binary alone, no doc tests ({sorted(got)})')
        got = ran({'a/src/lib.rs': PICK['a/src/lib.rs'] + '\npub fn three() -> u32 { 3 }\n'})
        check({'-p a --test one', '-p a --test other', '-p b --test uses', '-p b --test names', 'doc'} <= got
              and not any(a.startswith('-p c') for a in got), f'a used crate: it and its users, not c ({sorted(got)})')
        got = ran({'research/oracle.txt': PICK['research/oracle.txt'].replace('0x00100000', '0x00100004')})
        check(got == {'-p b --test names'}, f"an oracle line: the binary naming it ({sorted(got)})")
        got = ran({'research/c9/rows.csv': 'a,b\n1,2\n'})
        check(got == set(), f'a research file no test names reaches nothing ({sorted(got)})')
        got = ran({'notes.md': 'hi\n'})
        check(got == set(), f'an ignored file reaches nothing ({sorted(got)})')
        got = ran({'Cargo.toml': PICK['Cargo.toml'] + '\n'})
        check({'-p a --test one', '-p b --test uses', '-p c --test alone'} <= got, f'the workspace manifest: everything ({sorted(got)})')
        got = ran({'c/src/lib.rs': 'pub fn one() -> u32 { 2 - 1 }\n'}, all_slow=True)
        check({'-p a --test one', '-p b --test uses', '-p c --test alone'} <= got, f'--all: everything ({sorted(got)})')
        notes = []
        got = ran({'gate.toml': PICK['gate.toml'] + '\n'}, notes=notes)
        check({'-p a --test one', '-p b --test uses', '-p c --test alone'} <= got, f'gate.toml: everything ({sorted(got)})')
        check(any('gate.toml is outside every package' in n for n in notes), f'and says why ({notes})')

        # `./kit check --since <rev>`, a merge's gate: only what changed after rev, not since the shared branch.
        commit = lambda m: g('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qam', m, cwd=w)
        (w / 'a/src/lib.rs').write_text(PICK['a/src/lib.rs'] + '\npub fn three() -> u32 { 3 }\n')
        commit('a library change, already gated')
        passed = g('rev-parse', 'HEAD', cwd=w).stdout.strip()
        (w / 'c/tests/alone.rs').write_text('#[test]\nfn alone() { assert_eq!(c::one(), 1 + 0); }\n')
        commit('a test-only change on top')
        got = ran({}, since=passed)
        check(got == {'-p c --test alone'}, f'--since: what changed after it alone ({sorted(got)})')
        got = ran({})
        check({'-p a --test one', '-p b --test uses', '-p c --test alone'} <= got,
              f'against the shared branch: the library change too ({sorted(got)})')
        # A [[slow]] binary whose paths the change misses: `done` leaves it out, a merge's --since runs it.
        slow = PICK['gate.toml'] + '\n[[slow]]\ntest = "-p b --test uses"\npaths = ["b/tests/uses.rs"]\n'
        (w / 'gate.toml').write_text(slow)
        commit('b/tests/uses is slow')
        gated = g('rev-parse', 'HEAD', cwd=w).stdout.strip()
        (w / 'a/src/lib.rs').write_text(PICK['a/src/lib.rs'] + '\npub fn three() -> u32 { 2 + 1 }\n')
        commit('a library change b uses')
        got = ran({}, since=gated)
        check('-p a --test one' in got and '-p b --test uses' not in got, f'done: the slow binary left out ({sorted(got)})')
        got = ran({}, since=gated, keep_slow=True)
        check({'-p a --test one', '-p b --test uses'} <= got, f'a merge\'s --since: the slow binary runs ({sorted(got)})')
        notes = []
        got = ran({}, since='0' * 40, notes=notes)
        check({'-p a --test one', '-p b --test uses', '-p c --test alone'} <= got, f'--since nothing known: everything ({sorted(got)})')
        check(any('cannot say' in n for n in notes), f'and says why ({notes})')


def test_threads() -> None:
    import threading, time
    t = cargo_gate.Threads(4)
    busy, peak, got, lock = [0], [0], [], threading.Lock()

    def one(want, tag):
        with t.take(want) as k:
            with lock:
                busy[0] += k
                peak[0] = max(peak[0], busy[0])
                got.append((tag, k))
            time.sleep(0.05)
            with lock:
                busy[0] -= k

    runs = [threading.Thread(target=one, args=a) for a in [(3, 'a'), (9, 'b'), (1, 'c'), (0, 'd'), (2, 'e')]]
    for r in runs:
        r.start()
        time.sleep(0.005)
    for r in runs:
        r.join()
    check(peak[0] <= 4, f'never more threads than the budget (peak {peak[0]})')
    check(dict(got) == {'a': 3, 'b': 4, 'c': 1, 'd': 1, 'e': 2}, f'each takes its tests, 1 to all ({dict(got)})')
    check([x for x, _ in got] == list('abcde'), f'in the order they came ({[x for x, _ in got]})')


def test_give_way() -> None:
    import threading, time
    want, events, lock = threading.Event(), [], threading.Lock()
    running = [0]

    def give_way():
        with lock:
            events.append(('gave way', running[0]))
        time.sleep(0.1)
        want.clear()

    way = cargo_gate.GiveWay(want.is_set, give_way)

    def binary(tag, secs):
        with way.turn():
            with lock:
                running[0] += 1
                events.append((tag, running[0]))
            time.sleep(secs)
            with lock:
                running[0] -= 1

    first = [threading.Thread(target=binary, args=(t, 0.2)) for t in 'ab']
    for t in first:
        t.start()
    time.sleep(0.05)
    want.set()  # another gate's lone rerun now waits for this one's slot
    later = [threading.Thread(target=binary, args=(t, 0.01)) for t in 'cd']
    for t in later:
        t.start()
    for t in first + later:
        t.join()
    tags = [e[0] for e in events]
    check(tags[:2] == ['a', 'b'] and tags[2] == 'gave way', f'the binaries running finish first ({events})')
    check(events[2] == ('gave way', 0), f'and the slot goes with nothing of this gate running ({events})')
    check(sorted(tags[3:]) == ['c', 'd'], f'then the waiting binaries start ({events})')
    events.clear()
    binary('e', 0)
    check(events == [('e', 1)], f'no rerun waiting: no pause ({events})')

    fails = cargo_gate.GiveWay(lambda: True, lambda: (_ for _ in ()).throw(KeyboardInterrupt))
    try:
        with fails.turn():
            pass
    except KeyboardInterrupt:
        pass
    check(not fails.paused, 'a give_way that raises leaves the gate unpaused')


if __name__ == '__main__':
    test_gate()
    test_select()
    test_threads()
    test_give_way()
    print(f'\n{ok} passed, {fail} failed')
    sys.exit(1 if fail else 0)
