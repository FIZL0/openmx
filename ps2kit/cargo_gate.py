"""A whole-workspace `cargo test`, with its test binaries run side by side.

`cargo test` runs a workspace's test binaries one after another. Each binary runs its own tests in parallel,
but the next binary waits for the slowest test of the last, so a gate is the *sum* of its binaries: in the
port this was written for, 102 binaries came to eighteen minutes, while the slowest of them took four and a
half. This builds once (`--no-run`), then runs every binary in a pool, longest first by the last run's
times, so the gate takes about as long as its slowest binary.

It only replaces a bare, whole-workspace run (`./kit check` and `./kit done` with no arguments). A filtered
`./kit check -- -p X --test Y` still runs the configured command as written, since a filter is one binary
anyway.

More that a whole run does, all read from a tracked `gate.toml` at the root of the checkout. It is
tracked rather than in `kit.toml` because every worktree has to agree on it, and `kit.toml` stays per-machine.

    [[slow]]                       # a binary the gate runs only when the work touches what it proves
    test = "-p game-sim --test camera"
    paths = ["crates/game-sim/src/camera.rs", "crates/ps2-float/*"]

    [flaky]                        # failures rerun alone once before they count: load, not the change
    tests = ["-p openmx --test lobby::", "::two_builds_since_the_id"]

    [alone]                        # binaries that time themselves: one by one after the pool; a failure
                                   # reruns once with no other worktree's test run going (every slot held)
    tests = ["-p openmx --test one_fps"]

A slow binary runs anyway under `./kit check --all`, which is what a run's merge check uses, and a filtered
check names it directly. `paths` are fnmatch globs (`*` crosses `/`) against what this branch changed since
it left the shared branch, plus what is uncommitted. A flaky entry is a regex against `<rerun args>::<test>`.
A test only gets its lone rerun when every failure in that binary matches an entry, so a binary that also
failed for real is left failed.

The log keeps cargo's shape: each binary's `running N tests … test result:` block, then cargo's own
`error: test failed, to rerun pass` lines, which `./kit check`'s summary and the runner's retest both read.
"""
from __future__ import annotations

import fnmatch
import json
import os
import re
import shlex
import subprocess
import time
import tomllib
from concurrent.futures import ThreadPoolExecutor
from contextlib import nullcontext
from pathlib import Path

WHOLE = re.compile(r'^\s*cargo\s+test(\s|$)')


def applies(cmd: str, extra: list[str]) -> bool:
    """A bare `cargo test …` with nothing appended: the whole workspace, which is what this is for."""
    return not extra and bool(WHOLE.match(cmd)) and os.environ.get('KIT_CHECK_SERIAL', '') != '1'


def gate_config(cwd: Path) -> dict:
    p = cwd / 'gate.toml'
    if not p.exists():
        return {}
    try:
        return tomllib.loads(p.read_text(encoding='utf-8'))
    except (OSError, tomllib.TOMLDecodeError) as e:
        raise SystemExit(f'gate.toml: {e}')


def changed(cwd: Path, shared: str) -> list[str] | None:
    """What this work changed: commits since the shared branch, then the tree. None if git can't say."""
    def g(*a):
        return subprocess.run(['git', *a], cwd=str(cwd), capture_output=True, text=True)
    base = g('merge-base', 'HEAD', shared)
    if base.returncode:
        return None
    out = set()
    for args in (['diff', '--name-only', base.stdout.strip()], ['ls-files', '--others', '--exclude-standard']):
        r = g(*args)
        if r.returncode:
            return None
        out.update(l for l in r.stdout.splitlines() if l)
    return sorted(out)


class Binary:
    def __init__(self, exe: str, pkg: str, kind: str, name: str, manifest: str):
        self.exe, self.pkg, self.kind, self.name = exe, pkg, kind, name
        self.dir = str(Path(manifest).parent)
        flag = {'lib': '--lib', 'bin': f'--bin {name}', 'test': f'--test {name}', 'bench': f'--bench {name}',
                'example': f'--example {name}'}.get(kind, f'--test {name}')
        self.args = f'-p {pkg} {flag}'
        self.rc, self.out, self.secs = 0, '', 0.0


def _pkg_name(package_id: str) -> str:
    # "path+file:///…/crates/game-sim#0.1.0" or "game-sim 0.1.0 (path+file://…)" or "…#name@0.1.0"
    if ' ' in package_id:
        return package_id.split(' ', 1)[0]
    tail = package_id.rsplit('#', 1)[-1]
    if '@' in tail:
        return tail.split('@', 1)[0]
    return package_id.rsplit('#', 1)[0].rstrip('/').rsplit('/', 1)[-1]


def build(cmd: str, cwd: Path, env: dict, log) -> tuple[int, list[Binary]]:
    """`cmd --no-run`, its diagnostics written to the log as text; the test binaries it made."""
    plain = re.sub(r'\s--message-format(=|\s+)\S+', '', ' ' + cmd).strip()
    full = f'{plain} --no-run --message-format=json-diagnostic-short'
    log.write(f'$ {full}\n')
    log.flush()
    p = subprocess.Popen(full, shell=True, cwd=str(cwd), stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                         env=env, text=True, errors='replace')
    bins = []
    for line in p.stdout:
        if not line.startswith('{'):
            log.write(line)
            continue
        try:
            m = json.loads(line)
        except json.JSONDecodeError:
            log.write(line)
            continue
        if m.get('reason') == 'compiler-message':
            r = (m.get('message') or {}).get('rendered')
            if r:
                log.write(r if r.endswith('\n') else r + '\n')
        elif m.get('reason') == 'compiler-artifact' and m.get('executable') and (m.get('profile') or {}).get('test'):
            t = m['target']
            bins.append(Binary(m['executable'], _pkg_name(m['package_id']), t['kind'][0], t['name'],
                               m.get('manifest_path', str(cwd / 'Cargo.toml'))))
    return p.wait(), bins


def _run_one(b: Binary, env: dict, filters: list[str] | None = None) -> None:
    e = {**env, 'CARGO_MANIFEST_DIR': b.dir, 'CARGO_PKG_NAME': b.pkg, 'CARGO_CRATE_NAME': b.name.replace('-', '_')}
    args = [b.exe, '-q'] + (['--exact', *filters] if filters else [])
    t = time.monotonic()
    r = subprocess.run(args, cwd=b.dir, env=e, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                       errors='replace')
    b.rc, b.out, b.secs = r.returncode, r.stdout, time.monotonic() - t


def failed_tests(out: str) -> list[str]:
    """The names in the `failures:` list at the end of a libtest run."""
    lines = out.splitlines()
    names = []
    for i, l in enumerate(lines):
        if l.strip() == 'failures:' and i + 1 < len(lines) and lines[i + 1].startswith('    '):
            for k in lines[i + 1:]:
                if not k.startswith('    '):
                    break
                names.append(k.strip())
    return names


def alone_tests(cwd: Path) -> list[str]:
    """gate.toml's `[alone]` binaries, each as its rerun args (`-p openmx --test one_fps`)."""
    return [' '.join(x.split()) for x in (gate_config(cwd).get('alone') or {}).get('tests', [])]


def run(cmd: str, cwd: Path, env: dict, jobs: int, log_path: Path, shared: str, all_slow: bool,
        times_path: Path, alone=nullcontext) -> tuple[int, list[str]]:
    """The whole gate. Returns cargo's exit code shape (0 passed, 101 a test failed, else the build's) and
    the notes `./kit check` adds to its one line: slow binaries left out, flakes that passed alone.

    `alone` is a context manager a failed `[alone]` binary's rerun runs inside: `./kit check` passes one
    that holds every check slot on the machine (checks.alone). Only a failure takes the machine — a budget
    test that passes beside the other worktrees' gates has no reason to stop them."""
    gc = gate_config(cwd)
    notes = []
    with open(log_path, 'w', encoding='utf-8') as log:
        log.write(f'(cwd {cwd}, {jobs} jobs, test binaries side by side)\n\n')
        rc, bins = build(cmd, cwd, env, log)
        if rc:
            return rc, notes

        skipped = []
        if not all_slow and gc.get('slow'):
            touched = changed(cwd, shared)
            for s in gc['slow']:
                want = ' '.join(str(s.get('test', '')).split())
                if touched is not None and not any(fnmatch.fnmatch(f, p) for f in touched for p in s.get('paths', [])):
                    hit = [b for b in bins if b.args == want]
                    if hit:
                        bins = [b for b in bins if b.args != want]
                        skipped.append(want.rsplit(' ', 1)[-1])
        if skipped:
            notes.append(f'not run, untouched: {", ".join(skipped)} (./kit check --all runs them)')

        try:
            last = json.loads(times_path.read_text())
        except (OSError, ValueError):
            last = {}
        bins.sort(key=lambda b: -float(last.get(b.args, 0)))
        # A binary that holds itself to a frame budget measures the machine as much as the code, and a pool
        # of other binaries is a busy machine: those run after it, one at a time.
        solo = set(alone_tests(cwd))
        later = [b for b in bins if b.args in solo]
        pooled = [b for b in bins if b.args not in solo]
        workers = max(1, jobs)
        benv = {**env, 'RUST_TEST_THREADS': env.get('RUST_TEST_THREADS', str(jobs))}

        def doc_tests():
            r = subprocess.run(f'{re.sub(r"--message-format(=|\s+)\S+", "", cmd)} --doc', shell=True,
                               cwd=str(cwd), stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                               env=benv, errors='replace')
            return r.returncode, r.stdout

        with ThreadPoolExecutor(max_workers=workers) as pool:
            dfut = pool.submit(doc_tests)
            list(pool.map(lambda b: _run_one(b, benv), pooled))
            drc, dout = dfut.result()
        for b in later:
            _run_one(b, benv)
        # Over budget may be the other worktrees' gates, not the change: one more go with the machine to
        # itself, and only that one counts.
        for b in later:
            if b.rc and (names := failed_tests(b.out)):
                again = Binary(b.exe, b.pkg, b.kind, b.name, str(Path(b.dir) / 'Cargo.toml'))
                with alone():
                    _run_one(again, benv, names)
                b.out += f'\n(kit: {len(names)} time-budget test(s) failed; rerun with no other test run going: ' \
                         f'{"passed" if not again.rc else "failed again"})\n' + again.out
                if not again.rc:
                    b.rc = 0
                    notes.append(f'passed on a rerun with the machine to itself (time budget, gate.toml): '
                                 f'{", ".join(n.rsplit("::", 1)[-1] for n in names)}')

        # A failure that is load and not the change gets one more go, alone, now the pool is quiet.
        flaky = [re.compile(p) for p in (gc.get('flaky') or {}).get('tests', [])]
        reran = []
        for b in bins:
            if not b.rc or not flaky:
                continue
            names = failed_tests(b.out)
            if names and all(any(f.search(f'{b.args}::{n}') for f in flaky) for n in names):
                again = Binary(b.exe, b.pkg, b.kind, b.name, str(Path(b.dir) / 'Cargo.toml'))
                _run_one(again, benv, names)
                b.out += f'\n(kit: {len(names)} known load flake(s) failed; rerun alone: ' \
                         f'{"passed" if not again.rc else "failed again"})\n' + again.out
                if not again.rc:
                    b.rc = 0
                    reran += names
        if reran:
            notes.append(f'passed on a lone rerun (load flakes, gate.toml): {", ".join(n.rsplit("::", 1)[-1] for n in reran)}')

        for b in bins:
            log.write(f'     Running {os.path.relpath(b.exe, cwd)} ({b.args}, {b.secs:.1f}s)\n{b.out}\n')
        if dout:
            log.write(f'   Doc-tests\n{dout}\n')
        failed = [b for b in bins if b.rc]
        for b in failed:
            log.write(f'error: test failed, to rerun pass `{b.args}`\n')
        if drc:
            log.write('error: test failed, to rerun pass `--doc`\n')

    try:
        times_path.write_text(json.dumps({**last, **{b.args: round(b.secs, 1) for b in bins}}, indent=0))
    except OSError:
        pass
    return (101 if failed or drc else 0), notes
