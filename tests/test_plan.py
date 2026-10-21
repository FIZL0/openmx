#!/usr/bin/env python3
"""Self-test for the agent layer: the kit's glue to agent-runner, the journal, the check summary and the guard.

    python3 tests/test_plan.py          (no kit.toml needed, no network, no emulator, a few seconds)

The task list and the run loop are agent-runner's, tested there. Here: the settings the kit hands it, the
oracle join, `done`'s plan calls, the journal states, the build-failed/test-failed split and the guard
hook's payload suite.
"""
from __future__ import annotations

import fnmatch
import json
import os
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from agent_runner import plan as P  # noqa: E402
from ps2kit import journal as J  # noqa: E402
from ps2kit import checks  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
FAILED: list[str] = []


def check(ok, what, extra=''):
    print(('  ok   ' if ok else '  FAIL ') + what + ('' if ok else f'\n         {extra}'))
    if not ok:
        FAILED.append(what)


def eq(got, want, what):
    check(got == want, what, f'got {got!r}\n         want {want!r}')


def has(text, pat, what):
    check(re.search(pat, text, re.M) is not None, what, f'no /{pat}/ in:\n' + text[:800])


# --- the task list ------------------------------------------------------------

def test_shipped_backlog():
    """The backlog, whether it is the one the kit ships or one a port has half worked through.

    Most of this holds for the life of a project: the shape of a task line, and the ids `(after X)`
    points at — where a dependency is satisfied by a *finished* task, which is in `DONE.md` and no
    longer here. `pick` already reads it that way; this test used to disagree, and so went red in every
    port repo the moment its first dependency landed.

    The count is the one assertion that belongs to the kit's own repo and not to a port. There it is a
    real guard that the shipped backlog has not been truncated, and nothing ever finishes a task in it;
    here a backlog that shrinks every week is the point. So it runs only where it means something, asked
    the way the kit asks it elsewhere: no `[game]` table at all means this tree is not a port — a port
    has one from the moment `kit.toml` is copied from the example, filled in or not.
    """
    print('plan: the backlog, shipped or worked through')
    from ps2kit.config import cfg
    todo = ROOT / 'plan/TODO.md'
    ls = P.lines(todo)
    ids = [m.group(3) for l in ls if (m := P.ITEM.match(l))]
    done = [m.group(3) for l in P.lines(ROOT / 'plan/DONE.md') if (m := P.ITEM.match(l))]
    eq(len(ids), len(set(ids)), 'every id in plan/TODO.md is unique')
    check(ids or done, 'the backlog has tasks, open or finished')
    if not cfg(False).raw.get('game'):
        check(25 <= len(ids) <= 60, f'{len(ids)} tasks: a session each, enough to start a port')
    check(any(l.startswith('### Stretch') for l in ls), 'there is a ### Stretch section')
    check(all(' → ' in l or ' | ' in l for l in ls if P.ITEM.match(l)),
          'every task line names the files it touches or the test it adds')
    for name in ('DONE.md', 'HUMAN.md', 'REFERENCE.md'):
        check((ROOT / 'plan' / name).exists(), f'plan/{name} exists')
    # the ids on the line are what `(after X)` and the runner match on
    for l in ls:
        if not P.ITEM.match(l):
            continue
        if m := re.search(r'\(after (\w+)\)', l):
            check(m.group(1) in ids or m.group(1) in done,
                  f'(after {m.group(1)}) names a task that exists, open or done')


# --- the journal --------------------------------------------------------------

def test_journal():
    print('journal: new / state / finish / police')
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp) / 'research/journal'
        root.mkdir(parents=True)
        J._journal = lambda: root  # the only thing in the module that knows where the journal is
        p = J.new('D3', 'boot elf')
        check(p.exists(), 'journal new writes the entry')
        has(p.parent.name, r'^\d{4}-\d\d-\d\d-d3-boot-elf$', 'the directory is dated and names the task')
        has(p.name, r'^1-.*-PART\.md$', 'the first iteration is numbered 1 and PART (the agent continues)')
        body = p.read_text()
        has(body, r'## Not verified / not 1:1', 'the stub demands the not-verified section')
        has(body, r'\bnone\b', 'and says what to write when there is nothing to declare')
        eq(J.state_of(p), 'PART', 'state_of reads the state out of the file name')
        p2 = J.new('D3', 'boot elf', 'overlays')
        eq(p2.name.split('-')[0], '2', 'the next entry in the same directory is iteration 2')
        f = J.finish(p2)
        eq(f.name.split('-')[-1], 'FINAL.md', 'finish renames the newest entry to FINAL')
        check(not p2.exists(), 'and the PART name is gone')
        eq([x.name for x in J.entries('D3')], [p.parent.name], 'entries finds the task\'s directory')
        (p.parent / 'shot.png').write_bytes(b'\x89PNG\r\n\x1a\n')
        bad = J.police()
        check(any('shot.png' in c for c in bad),
              'police spots a binary under the journal (it belongs in context/)', str(bad))


# --- the check summary --------------------------------------------------------

def test_check_summary():
    print('checks: the build-failed / test-failed split')
    with tempfile.TemporaryDirectory() as tmp:
        log = Path(tmp) / 'check.log'
        log.write_text('running 41 tests\ntest result: ok. 41 passed; 0 failed\n', encoding='utf-8')
        out = checks.summarise(0, log)
        eq(len(out), 1, 'a passing run prints exactly one line')
        has(out[0], r'TESTS PASSED: 41 tests', 'and says how many tests passed')

        log.write_text('   Compiling game-sim v0.1.0\nerror[E0308]: mismatched types\n'
                       ' --> crates/game-sim/src/move.rs:12:9\nerror: could not compile `game-sim`\n',
                       encoding='utf-8')
        out = '\n'.join(checks.summarise(1, log))
        has(out, r'^BUILD FAILED', 'a build failure gets its own headline, first line')
        has(out, r'E0308', 'with the compiler error')
        check('test result' not in out, 'and no test output to confuse it with a failing test')

        log.write_text('running 3 tests\ntest fpu::add_masks_guard_bit ... FAILED\n\nfailures:\n'
                       '    fpu::add_masks_guard_bit\n\ntest result: FAILED. 2 passed; 1 failed\n',
                       encoding='utf-8')
        out = '\n'.join(checks.summarise(1, log))
        has(out, r'^TESTS FAILED \(this is a test result, the build is fine\)',
            'a failing test says so unmistakably — it is a different next move from a build failure')
        has(out, r'fpu::add_masks_guard_bit', 'and names the test')

        log.write_text('\n'.join([f'line {i} ' + 'x' * 500 for i in range(400)]) + '\nerror: boom\n',
                       encoding='utf-8')
        out = checks.summarise(1, log)
        check(len(out) <= 62 and max(len(l) for l in out) <= 320,
              'a huge log is capped to a screenful and 300-odd columns', f'{len(out)} lines')
        has('\n'.join(out), r'full log: ', 'and points at the full copy on disk')


def test_audit():
    print('checks: the address audit, which is the only thing enforcing "no retail address in the port"')
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        known = {0x0035bc50: 'walk_ramp (gameplay)', 0x1f4a20: 'frame'}
        files = {
            'crates/sim/src/walk.rs': 'let t = ramp(0x0035bc50);\n',          # the leak that matters
            'crates/sim/src/turn.rs': 'FUN_0012abcd(x);  // and a placeholder name\n',
            'crates/sim/src/ok.rs':   'let mask = 0x3f800000;  // 1.0f\n',     # a constant, not an address
            'docs/NOTES.md':          'the walker is at 35bc50\n',            # prose counts too
            'ps2kit/decomp.py':       "HEADER = re.compile('FUN_([0-9a-f]+)')\n",  # the dump's own format
            'research/oracle.txt':    'walk_ramp gameplay 0x0035bc50\n',      # where it belongs
            'research/journal/2026-10-15-t1-walk/README.md': 'ported 0x0035bc50\n',
        }
        for rel, text in files.items():
            f = root / rel
            f.parent.mkdir(parents=True, exist_ok=True)
            f.write_text(text, encoding='utf-8')
        hits = checks.audit(root, list(files), known)
        where = {h.split(':')[0] for h in hits}
        eq(sorted(where), ['crates/sim/src/turn.rs', 'crates/sim/src/walk.rs', 'docs/NOTES.md'],
           'an address or a placeholder name is flagged wherever it is committed, prose included')
        check(not [h for h in hits if 'research/' in h],
              'research notes and the journal are where addresses are *supposed* to live')
        check(not [h for h in hits if 'ok.rs' in h], 'a float bit pattern is not an address')
        check(not [h for h in hits if 'ps2kit/' in h],
              "the kit's own modules parse FUN_ names for a living")
        has('\n'.join(hits), r'walk\.rs:1: 0x0035bc50 is walk_ramp', 'a hit says what the address is')
        has('\n'.join(hits), r'NOTES\.md:1: 35bc50 is walk_ramp', 'however it was spelled')

        bare = checks.audit(root, list(files), {})
        eq([h.split(':')[0] for h in bare], ['crates/sim/src/turn.rs'],
           'with nothing written down no address can be recognised, and nothing is guessed from a hex '
           'literal — only the placeholder name, whose shape is unmistakable, is still caught')

        (root / 'crates/sim/src/near.rs').write_text('let x = 0x0035bc500;\n', encoding='utf-8')
        eq(checks.audit(root, ['crates/sim/src/near.rs'], known), [],
           'a longer literal that merely starts with an address is not a hit')


# --- the hooks ----------------------------------------------------------------

def hook(payload, *args):
    r = subprocess.run([sys.executable, str(ROOT / 'ps2kit/hooks/guard.py'), *args],
                       input=json.dumps(payload), capture_output=True, text=True)
    return r.returncode, r.stdout + r.stderr


def test_guard():
    print('guard: its own payload suite, plus the cases a session hits daily')
    r = subprocess.run([sys.executable, str(ROOT / 'ps2kit/hooks/guard.py'), '--self-test'],
                       capture_output=True, text=True)
    check(r.returncode == 0, 'guard.py --self-test passes', r.stdout[-1500:] + r.stderr[-500:])

    rc, out = hook({'tool_name': 'Read', 'tool_input': {'file_path': 'docs/PS2.md'}})
    eq(rc, 0, 'an ordinary Read is allowed through')
    rc, out = hook({})
    eq(rc, 0, 'an empty payload is allowed (a hook must never crash the session)')
    rc, out = hook({'tool_name': 'Bash', 'tool_input': 'not a dict'})
    eq(rc, 0, 'a payload of the wrong shape is allowed, not an exception')

    rc, out = hook({'tool_name': 'Bash', 'tool_input': {'command': 'pg' + 'rep -f record_live'}})
    eq(rc, 0, 'a pattern search is not blocked...')
    check('--pgrep' in out, '...it is rewritten to the wrapper that skips the caller itself', out[:400])

    rc, out = hook({'tool_name': 'Bash', 'tool_input': {'command': './kit check -p ps2-float'}})
    eq(rc, 0, './kit check is allowed')
    check('3600000' in out, 'and its Bash timeout is raised, so a cold build is not killed at two minutes',
          out[:400])


def test_inbox():
    print('inbox: a ticket is handed over once, and holds a stop')
    with tempfile.TemporaryDirectory() as tmp:
        env = dict(os.environ, KIT_INBOX=str(Path(tmp) / 'inbox.md'), KIT_INBOX_SEEN=str(Path(tmp) / 'seen'))
        Path(env['KIT_INBOX']).write_text('- [ ] check the serve timing\n', encoding='utf-8')

        def run(payload):
            r = subprocess.run([sys.executable, str(ROOT / 'ps2kit/hooks/inbox.py')],
                               input=json.dumps(payload), capture_output=True, text=True, env=env)
            return r.returncode, r.stdout

        rc, out = run({'hook_event_name': 'PostToolUse'})
        check('serve timing' in out, 'a new ticket is handed to the agent', out[:300])
        rc, out = run({'hook_event_name': 'PostToolUse'})
        eq(out.strip(), '', 'the same ticket is never handed over twice')
        rc, out = run({'hook_event_name': 'Stop', 'stop_hook_active': False})
        check('"block"' in out, 'an agent stopping with an open ticket is told to keep going', out[:300])
        rc, out = run({'hook_event_name': 'Stop', 'stop_hook_active': True})
        eq((rc, out.strip()), (0, ''), 'stop_hook_active is honoured — a hook never loops a session')


# --- the glue to agent-runner -------------------------------------------------

def oracle_join(text):
    with tempfile.TemporaryDirectory() as tmp:
        u, o = Path(tmp) / 'union', Path(tmp) / 'out'
        u.write_text(text, encoding='utf-8')
        r = subprocess.run([sys.executable, str(ROOT / 'kit'), 'oracle', 'join', str(u), str(o)],
                           capture_output=True, text=True)
        return o.read_text(encoding='utf-8') if r.returncode == 0 else None


def test_oracle_join():
    print('oracle join: the [run.join] command for the addresses file')
    # The one joinable file where both sides can be wrong together: nothing downstream picks between two
    # addresses for one name, so a clash must not be resolved here.
    both = 'angle 0x1004c0 the stick angle\nspeed 0x100500\n'
    eq(oracle_join(both + 'speed 0x100500\n'), both, 'a name both sides added at the same address is kept once')
    eq(oracle_join('angle 0x1004c0\nangle 0x2004c0\n'), None, 'the same name at two addresses exits 1')
    eq(oracle_join('angle main 0x1004c0\nangle menu 0x1004c0\n').count('angle'), 2,
       'the same name in two overlays is two addresses and both stay')
    eq(oracle_join('angle\n'), None, 'a line that is not an address stops the join too')
    text = '# the stick\nangle 0x1004c0\n\n# the bike\nspeed 0x100500\n'
    eq(oracle_join(text), text, 'comments and blank lines survive the join')


def kit_py(code, toml, **env):
    """Run `code` in a fresh interpreter with KIT_CONFIG at `toml`; its stdout."""
    r = subprocess.run([sys.executable, '-c', f'import sys; sys.path.insert(0, {str(ROOT)!r}); {code}'],
                       capture_output=True, text=True, env={**os.environ, 'KIT_CONFIG': str(toml), **env})
    check(r.returncode == 0, f'{code[:40]} runs', r.stderr[-400:])
    return r.stdout


def test_runner_config():
    print('run: the settings ./kit run hands agent-runner')
    from ps2kit import runner as K
    with tempfile.TemporaryDirectory() as tmp:
        toml = Path(tmp) / 'kit.toml'
        toml.write_text(f'[paths]\ncontext = "ctx"\njournal = "notes/j"\n[game]\niso = "game/x.iso"\n'
                        f'[git]\npush_merge = false\n'
                        f'[agent-runner.run]\nslots = 5\n', encoding='utf-8')
        s = json.loads(kit_py('import json; from ps2kit import runner; print(json.dumps(runner.settings()))', toml))
        r = s['run']
        eq(r['command'], './kit', 'sessions are told ./kit, not agent-runner')
        eq(r['test'], './kit check', 'the gate is ./kit check')
        eq(r['slot_env'], ['KIT_PCSX2={n}', 'KIT_RUN=1'], 'each slot gets its own emulator copy')
        check({'KIT_CONFIG', 'KIT_CHECK_SLOTS', 'AGENT_RUNNER_IDLE_BG'} <= set(r['pass_env']),
              'and the kit settings the runner was started with', str(r['pass_env']))
        eq(r['links'], ['ctx', '*.iso', 'game/x.iso'], 'context and the disc are linked into every worktree')
        eq(r['notes'], 'ctx/notes', 'the runner logs into context/notes')
        eq(s['plan'], {'dir': 'plan', 'journal': 'notes/j'}, 'plan and journal come from kit.toml')
        eq(r['joinable'], ['plan/*.md', 'notes/j/*.md', 'research/oracle.txt'], 'joinable: plan, journal, oracle')
        eq(r['join'], {'research/oracle.txt': './kit oracle join {union} {out}'}, 'the oracle file joins via ./kit')
        eq(r['untested'], ['*.md', '.github/*'], 'docs and CI files do not need a re-test')
        eq((s['git']['push'], s['git']['push_merge']), ('origin', False), '[git] carries over')
        eq(r['slots'], 5, 'a kit.toml [agent-runner] table wins')
        eq(tomllib.loads('\n'.join(K._toml(s))), s, 'the generated file reads back as the same settings')
        for f in ('plan/TODO.md', 'plan/DONE.md', 'notes/j/2026-01-01-t7-walker/README.md', 'research/oracle.txt'):
            check(any(fnmatch.fnmatch(f, g) for g in r['joinable']), f'{f} is joinable')
        for f in ('PLAN.md', 'AGENT.md', 'docs/WORKFLOW.md', 'crates/game-sim/src/tick.rs', 'plan/notes.txt'):
            check(not any(fnmatch.fnmatch(f, g) for g in r['joinable']), f'{f} needs reading, so not joinable')
    eq(K._slots([], 3), 3, 'slots: the default')
    eq(K._slots(['--mode', 'p', '5'], 3), 5, 'slots: after an option with a value')
    eq(K._slots(['once'], 3), 1, 'slots: once is one')


def test_done_plan_calls():
    print('done: ticks and blocks through agent-runner\'s plan')
    with tempfile.TemporaryDirectory() as tmp:
        repo = Path(tmp)
        (repo / 'plan').mkdir()
        (repo / 'plan/TODO.md').write_text('# TODO\n\n### Next\n\n- [ ] **T1** one\n- [ ] **T2** two\n'
                                           '- [ ] **T3** three\n', encoding='utf-8')
        (repo / 'plan/DONE.md').write_text('# DONE\n', encoding='utf-8')
        subprocess.run(['git', 'init', '-q', str(repo)], check=True)
        toml = repo / 'kit.toml'
        toml.write_text(f'[paths]\ncontext = {json.dumps(str(repo / "ctx"))}\n', encoding='utf-8')
        env = {'AGENT_RUNNER_REPO': str(repo)}
        kit_py("from ps2kit import plan; plan.tick('T1'); plan.block('T2', 'needs T3 first', after=['T3'])", toml,
               **env)
        todo = (repo / 'plan/TODO.md').read_text(encoding='utf-8')
        check('**T1**' not in todo and '**T1**' in (repo / 'plan/DONE.md').read_text(encoding='utf-8'),
              'tick moves the task to DONE.md')
        has(todo, r'- \[~\] \*\*T2\*\* two .*BLOCKED: \(after T3\) needs T3 first', 'block --after records the wait')
        check((repo / 'ctx/notes/agent-runner.toml').exists(), 'with the generated settings')

def test_cli_help():
    print('cli: every command of this layer explains itself with no kit.toml')
    for cmd in ('plan', 'journal', 'check', 'done', 'run', 'stop', 'oracle'):
        r = subprocess.run([sys.executable, str(ROOT / 'kit'), cmd, '--help'],
                           capture_output=True, text=True, cwd=str(ROOT))
        check(r.returncode == 0 and len(r.stdout) > 40,
              f'./kit {cmd} --help prints help', (r.stdout + r.stderr)[:400])


def main() -> int:
    for t in (test_shipped_backlog, test_journal, test_check_summary, test_audit, test_guard, test_inbox,
              test_oracle_join, test_runner_config, test_done_plan_calls, test_cli_help):
        t()
    print()
    if FAILED:
        print(f'{len(FAILED)} FAILED:')
        for f in FAILED:
            print(f'  - {f}')
        return 1
    print('all checks passed')
    return 0


if __name__ == '__main__':
    sys.exit(main())
