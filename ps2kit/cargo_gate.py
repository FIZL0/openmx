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

    [flaky]                        # failures rerun once, every slot held, before they count: load, not the change
    tests = ["-p openmx --test lobby::", "::two_builds_since_the_id"]

    [alone]                        # binaries that time themselves: one by one after the pool; a failure
                                   # reruns once with no other worktree's test run going (every slot held)
    tests = ["-p openmx --test one_fps"]

    [[variant]]                    # binaries built a second time, with features of their own, into a target
    features = "ps2-float/native"  # directory of their own, and run in the same pool, only the tests whose
    target_dir = "target/native"   # names hold `filter`
    filter = "native_tolerance"
    tests = ["-p game-sim --test replay"]

A slow binary runs anyway under `./kit check --all`, which is what a run's merge check uses, and a filtered
check names it directly. `paths` are fnmatch globs (`*` crosses `/`) against what this branch changed since
it left the shared branch, plus what is uncommitted. A flaky entry is a regex against `<rerun args>::<test>`.
A test only gets its lone rerun when every failure in that binary matches an entry, so a binary that also
failed for real is left failed. A variant binary's rerun line carries its features, target directory and
filter (`-p game-sim --features ps2-float/native --target-dir target/native --test replay native_tolerance`),
so `./kit check --rerun` rebuilds and runs just that.

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


def _workspace(cwd: Path, env: dict) -> list[dict] | None:
    """`cargo metadata --no-deps`'s packages, or None if cargo can't say."""
    r = subprocess.run(['cargo', 'metadata', '--no-deps', '--format-version', '1'], cwd=str(cwd), env=env,
                       capture_output=True, text=True)
    if r.returncode:
        return None
    try:
        return json.loads(r.stdout)['packages']
    except (ValueError, KeyError):
        return None


def _rs_files(d: Path) -> list[Path]:
    return [p for p in d.rglob('*.rs') if 'target' not in p.relative_to(d).parts]


class Selection:
    """What a change can reach: `whole` packages (every binary, doc tests too) and, in the others, `part`
    test targets by name. Built by `select`."""
    def __init__(self, everything: list[str]):
        self.everything = everything
        self.whole: set[str] = set()
        self.part: dict[str, set[str]] = {}
        self.libs: set[str] = set()  # packages with a library: the only ones `cargo test --doc -p` takes

    def all(self) -> bool:
        return self.whole >= set(self.everything)

    def keeps(self, pkg: str, kind: str, name: str) -> bool:
        return pkg in self.whole or (kind == 'test' and name in self.part.get(pkg, ()))


def select(cwd: Path, shared: str, env: dict, sel: dict) -> Selection | None:
    """gate.toml `[select]`: the packages and test binaries the work can change, or None for all of them.

    A file in a package's `tests/` that is a test target's root, or a module in that root's own folder
    (`tests/ai/` of `tests/ai/main.rs`), reaches that binary alone; any other file in a package (src,
    Cargo.toml, a tests/ helper or data) reaches the package and every workspace package that depends on
    it, dev-dependencies included. `oracle`'s edited lines reach the binaries whose sources name
    one of their names in quotes (`addr("name")`); a file under one of `data` reaches the binaries whose
    sources name its path, or its directory below `data`. `ignore` globs reach nothing. Anything else (the
    workspace's Cargo.toml or lock, the kit, the gate's own config) reaches everything."""
    touched = changed(cwd, shared)
    pkgs = _workspace(cwd, env) if touched is not None else None
    if pkgs is None:
        return None
    root = cwd.resolve()
    names = [p['name'] for p in pkgs]
    out = Selection(names)
    out.libs = {p['name'] for p in pkgs if any(k in ('lib', 'rlib') for t in p['targets'] for k in t['kind'])}
    dirs = {p['name']: Path(p['manifest_path']).parent.resolve() for p in pkgs}
    users: dict[str, set[str]] = {n: set() for n in names}
    for p in pkgs:
        for d in p['dependencies']:
            if d['name'] in users and d['name'] != p['name']:
                users[d['name']].add(p['name'])
    tests = {p['name']: {Path(t['src_path']).resolve(): t['name'] for t in p['targets'] if 'test' in t['kind']}
             for p in pkgs}

    def binary(pkg: str, path: Path) -> str | None:
        """The test binary `path` belongs to alone: its root, or a module in the root's own folder
        (`tests/ai/main.rs`'s `tests/ai/`, or `tests/atv_quad.rs`'s `tests/atv_quad/`)."""
        if path in tests[pkg]:
            return tests[pkg][path]
        for root, name in tests[pkg].items():
            own = root.parent if root.name == 'main.rs' else root.with_suffix('')
            if own != dirs[pkg] / 'tests' and path.is_relative_to(own):
                return name
        return None

    def dirty(pkg: str) -> None:
        todo = [pkg]
        while todo:
            n = todo.pop()
            if n not in out.whole:
                out.whole.add(n)
                todo += users[n]

    def owner(path: Path) -> str | None:
        best = None
        for n, d in dirs.items():
            if path.is_relative_to(d) and (best is None or len(d.parts) > len(dirs[best].parts)):
                best = n
        return best

    sources: list[tuple[str, Path, str]] | None = None

    def naming(words: list[str]) -> None:
        """Every source that holds one of `words`: its test binary, else its package."""
        nonlocal sources
        if sources is None:
            sources = [(n, f, f.read_text(encoding='utf-8', errors='replace')) for n, d in dirs.items()
                       for f in _rs_files(d)]
        for n, f, text in sources:
            if not any(w in text for w in words):
                continue
            if (b := binary(n, f)) is not None:
                out.part.setdefault(n, set()).add(b)
            elif f.is_relative_to(dirs[n] / 'tests'):
                out.whole.add(n)
            else:
                dirty(n)

    oracle, data = sel.get('oracle'), [str(d).rstrip('/') + '/' for d in sel.get('data', [])]
    for f in touched:
        if any(fnmatch.fnmatch(f, g) for g in sel.get('ignore', [])):
            continue
        path = (root / f).resolve()
        if f == oracle:
            words = sorted({l[1:].split()[0] for l in _diff_lines(cwd, shared, f)
                            if l[1:].strip() and not l[1:].lstrip().startswith('#')})
            naming([f'"{w}"' for w in words])
        elif (top := next((d for d in data if f.startswith(d)), None)) is not None:
            under = f[len(top):].split('/')[0]
            naming([f, f'{top}{under}', f'"{under}"'])
        elif (pkg := owner(path)) is not None:
            if (b := binary(pkg, path)) is not None:
                out.part.setdefault(pkg, set()).add(b)
            else:
                dirty(pkg)
        else:
            return None
    for n in out.whole:
        out.part.pop(n, None)
    return out


def _pkg_test(args: str) -> tuple[str, str, str]:
    """`-p game-sim --test replay` -> ('game-sim', 'test', 'replay'); a `--lib` or `--bin` -> its kind."""
    w = args.split()
    pkg = w[w.index('-p') + 1] if '-p' in w else ''
    for flag, kind in (('--test', 'test'), ('--bin', 'bin'), ('--lib', 'lib')):
        if flag in w:
            i = w.index(flag)
            return pkg, kind, w[i + 1] if kind != 'lib' and i + 1 < len(w) else ''
    return pkg, '', ''


def _diff_lines(cwd: Path, shared: str, f: str) -> list[str]:
    """The added and removed lines of `f` since the shared branch, uncommitted ones too."""
    g = lambda *a: subprocess.run(['git', *a], cwd=str(cwd), capture_output=True, text=True)
    base = g('merge-base', 'HEAD', shared).stdout.strip()
    r = g('diff', '-U0', base, '--', f)
    return [l for l in r.stdout.splitlines() if l[:1] in '+-' and not l.startswith(('+++', '---'))]


class Binary:
    def __init__(self, exe: str, pkg: str, kind: str, name: str, manifest: str):
        self.exe, self.pkg, self.kind, self.name = exe, pkg, kind, name
        self.dir = str(Path(manifest).parent)
        flag = {'lib': '--lib', 'bin': f'--bin {name}', 'test': f'--test {name}', 'bench': f'--bench {name}',
                'example': f'--example {name}'}.get(kind, f'--test {name}')
        self.args = f'-p {pkg} {flag}'
        self.filter = ''  # a variant's name filter (gate.toml [[variant]])
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


def variants(cmd: str, cwd: Path, env: dict, log, gc: dict, pick: Selection | None = None) -> tuple[int, list[Binary]]:
    """gate.toml's `[[variant]]`s: each one's binaries built with its features into its target directory,
    named by the args that rerun them, filter and all; with a `pick`, only the ones it keeps."""
    out = []
    for v in gc.get('variant') or []:
        tests = [' '.join(str(t).split()) for t in v.get('tests', [])]
        if pick is not None:
            tests = [t for t in tests if pick.keeps(*_pkg_test(t))]
        if not tests:
            continue
        flags = (f' --features {v["features"]}' if v.get('features') else '') + \
                (f' --target-dir {shlex.quote(str(v["target_dir"]))}' if v.get('target_dir') else '')
        rc, bins = build(f'{cmd}{flags} {" ".join(tests)}', cwd, env, log)
        if rc:
            return rc, out
        for b in bins:
            pkg, flag = b.args.split(' ', 2)[1], b.args.split(' ', 2)[2]
            b.filter = str(v.get('filter', ''))
            b.args = f'-p {pkg}{flags} {flag}' + (f' {b.filter}' if b.filter else '')
            out.append(b)
    return 0, out


def _run_one(b: Binary, env: dict, filters: list[str] | None = None) -> None:
    e = {**env, 'CARGO_MANIFEST_DIR': b.dir, 'CARGO_PKG_NAME': b.pkg, 'CARGO_CRATE_NAME': b.name.replace('-', '_')}
    args = [b.exe, '-q'] + (['--exact', *filters] if filters else [b.filter] if b.filter else [])
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

    `alone` is a context manager a failed `[alone]` binary's rerun, and a `[flaky]` one's, runs inside: `./kit check` passes one
    that holds every check slot on the machine (checks.alone). Only a failure takes the machine — a budget
    test that passes beside the other worktrees' gates has no reason to stop them."""
    gc = gate_config(cwd)
    notes = []
    with open(log_path, 'w', encoding='utf-8') as log:
        log.write(f'(cwd {cwd}, {jobs} jobs, test binaries side by side)\n\n')
        pick = select(cwd, shared, env, gc['select']) if gc.get('select') and not all_slow else None
        if pick is not None and pick.all():
            pick = None
        if pick is None:
            rc, bins = build(cmd, cwd, env, log)
        else:
            rc, bins = 0, []
            builds = ([' '.join(f'-p {n}' for n in sorted(pick.whole))] if pick.whole else []) + \
                [f'-p {n} ' + ' '.join(f'--test {t}' for t in sorted(ts)) for n, ts in sorted(pick.part.items())]
            for b in builds:
                rc, got = build(f'{cmd} {b}', cwd, env, log)
                bins += got
                if rc:
                    return rc, notes
        if rc:
            return rc, notes
        rc, more = variants(cmd, cwd, env, log, gc, pick)
        if rc:
            return rc, notes
        if pick is not None:
            notes.append(f'ran {len(bins) + len(more)} test binaries, the ones this work can reach'
                         f'{" (" + ", ".join(sorted(pick.whole)) + " whole)" if pick.whole else ""}'
                         ' (./kit check --all runs every one)')

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
        bins += more

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

        doc_pkgs = '' if pick is None else ' '.join(f'-p {n}' for n in sorted(pick.whole & pick.libs))

        def doc_tests():
            if pick is not None and not doc_pkgs:
                return 0, ''
            r = subprocess.run(f'{re.sub(r"--message-format(=|\s+)\S+", "", cmd)} --doc {doc_pkgs}', shell=True,
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

        # A failure that is load and not the change gets one more go, now the pool is quiet, holding
        # every check slot on the machine as an [alone] rerun does: rerun beside the other worktrees'
        # gates it failed again (fixed_rate at load 70-100 parked V111 and S216, 10-29).
        flaky = [re.compile(p) for p in (gc.get('flaky') or {}).get('tests', [])]
        reran = []
        for b in bins:
            if not b.rc or not flaky:
                continue
            names = failed_tests(b.out)
            if names and all(any(f.search(f'{b.args}::{n}') for f in flaky) for n in names):
                again = Binary(b.exe, b.pkg, b.kind, b.name, str(Path(b.dir) / 'Cargo.toml'))
                with alone():
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
