"""Unattended runs: keep N agent sessions working through the task list, for as long as the work lasts.

A port is months of small, provable steps, and most of them do not need a human in the loop — they need the
backlog, the emulator, the tests and a commit. `./kit run` is the loop that does that: it picks the next task
nobody is on, opens a git worktree and a tmux pane for it, starts an agent session with the task's brief
already pasted into the first turn, waits for it to finish, merges the branch, and starts the next one.

    ./kit run once            one task, then stop (the way to try it the first time)
    ./kit run                 3 agents, mode d (different files), in tmux
    ./kit run 5 --mode p      5 agents, strictly in plan/TODO.md order, files may overlap
    ./kit run 1               one at a time (the sequential run; also what you get without tmux)
    ./kit run --force         keep starting tasks past the weekly usage cap

Environment: `KIT_MODEL` the agents' model, `KIT_MASTER_MODEL` the master's (both default to opus),
`KIT_AGENT` the CLI to run, `KIT_LEAN=0` to give the agents the full tool set.

What it has to get right, and why each of these is here rather than left to the agent:

**Task picking** goes down `plan/TODO.md` in order and skips: blocked tasks (`[~]`), split parents (a task
whose own subtasks are open), tasks whose `(after X)` dependency is still open, the `### Stretch` section,
anything already tried this run, and anything whose branch is finished and waiting to be merged. Siblings
(`L3a`, `L3b` — same prefix+number) never run at once in any mode, because they are the same code by
definition. Mode `d` additionally keeps two running tasks off the same files, which are named on each task's
line; mode `p` ignores that and just follows priority order.

**One worktree per slot**, kept between tasks so the build directory stays warm: the first build in a fresh
worktree is the slowest thing in the whole run. `context/` and the disc image are symlinked in, never copied.

**One emulator per slot** (`KIT_PCSX2=N`): every kit tool follows that variable, so two agents never load a
save state into each other's PCSX2.

**An idle stop** (a Stop hook, `--stop-hook` below). A finished agent sits at its prompt forever otherwise,
and the loop cannot start the next task. The hook ends the session once the transcript has been quiet for
`--idle` seconds — and typing into the pane resets that, so you can take a session over by hand.

**A usage limit is not a failed task.** A session that only hit the limit did no work: its transcript is
discarded, the task goes back in the pool, and the slot sleeps until the reset. Counting it as "tried" would
quietly drop a task from the night.

**A stall watchdog**, because the one thing worse than a crashed session is a live one that is not moving:
no transcript activity for `--stall` seconds at a prompt (or past the Bash timeout with a tool running) is
logged and belled, with the pane to attach to.

**A master for the merges it cannot do.** A branch that conflicts in anything but append-only text needs
somebody to read both sides and decide, and for most of a run there is nobody. So the run keeps one more
session — a long-lived agent in the main checkout, in its own tmux window — and hands it those branches one
at a time, clearing its context in between. It is the difference between a night that finishes and a night
that stops: `pick` skips tasks whose branch is waiting to be merged, and anything `(after X)` on one of
them, so a few unmergeable branches can hold up the whole critical path until a human wakes up. The master
may not touch the worktrees, do task work, or publish the shared branch — that last one stays a decision a
person makes. Its ledger is `context/notes/master.md`, which is also where the runner's own merges go.

Without tmux it degrades to a single sequential session and says so — and then there is no master because
there is nothing for one to do: `sequential` runs in this checkout on the current branch, so a task commits
straight onto it and no merge happens at all. (`finish`'s `has_master=False` branch is the vestige of an
earlier shape where it could; nothing calls it — see K2.) Linux/macOS only — the Windows edition of the kit
does this differently.

Internal modes (used by the sessions it starts, not by you):
    --stop-hook            Stop hook: arm the idle timer for the session that is stopping
    --stop-waiter P T N S  the detached timer itself
"""
from __future__ import annotations

import argparse
import datetime as dt
import glob
import json
import os
import re
import shlex
import subprocess as sp
import sys
import threading
import time
import uuid
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))  # run directly (the Stop hook does)

from ps2kit.config import cfg, ROOT
from ps2kit import platform_ as plat

SLOTS_DIR = ROOT.parent / f'{ROOT.name}-slots'
TASK = re.compile(r'^- \[ \] \*\*([^*\s]+)\*\*(.*)')
ANY_TASK = re.compile(r'^- \[.\] \*\*([^*\s]+)\*\*')
# A task that blocked itself on something this run can in fact do unattended: the emulator, a capture, a
# screenshot. Worth a second look rather than a human's morning, so it is flagged in the master's notes.
SUSPECT = re.compile(r'BLOCKED:.*(pcsx2|pine|capture|record|screenshot|save state|unattended)', re.I)
WEEKLY_MAX = 90  # % of the account's 7-day usage; at or past it, no new task starts unless --force

PROMPT = """Unattended run: nobody will answer questions, so never ask one — decide, write down why, and go on.

Do task {tid} and nothing else. You are in a git worktree on branch task/{tid}: commit here only, never touch
the main checkout and never merge, rebase or push the main branch — the runner merges your branch when
your session ends, and whether that merge goes out is the user's call, not yours or mine. `./kit done`
pushes `task/{tid}` for you, which is the one copy of your work that is not on this disk.
{others}
You have your own emulator copy: KIT_PCSX2={n} is set, so `./kit pcsx2 start`, `./kit pine`, `./kit pad` and
`./kit shot` all talk to copy {n} and nobody else's. Run anything that drives the game in more than one step
as a single job under `./kit pcsx2 with -- <cmd>` so it holds the emulator for the whole job; keep each one
short. Close your copy when you are done with it: `./kit pcsx2 stop` (never kill the process by name — the
other slots' copies are running too).

Rules: AGENT.md and docs/. Tests only through `./kit check` (one line when they pass; it says plainly whether
the build broke or a test failed). Read decompilation with `./kit fn` / `./kit xref`, never whole files.

Write the task's journal entry as you go (`./kit journal new {tid} <slug>`), and under **Not verified / not
1:1** list everything you could not prove against the original or could not match exactly, each with its
reason — or `none`. A skipped proof that is written down is useful; a skipped proof that is silent is a bug
nobody will find.

Finish with `./kit done {tid} "<commit message>"`. If you are stuck (a tool hangs, the same fix fails twice,
it needs the human, ~20 minutes with no progress): commit what is solid and `./kit done {tid} "<msg>"
--blocked "<why + what would unblock it>"`, then stop. A usage limit is not stuck: commit what is solid, do
not block the task and do not stop the emulator — the session waits for the reset and carries on.

Its brief (`./kit plan brief {tid}`) follows, so don't gather it again.

{brief}"""

OTHERS = ('Other agents are working on other tasks at the same time, in other worktrees. Keep your changes '
          'local (add functions and new modules; don\'t move, rename or reformat code you did not come for) '
          'so the merges stay clean.\n')


# --- small helpers -------------------------------------------------------------

def log(msg: str) -> None:
    line = f'=== {dt.datetime.now().isoformat(timespec="seconds")} {msg}'
    print(line, flush=True)
    try:
        with open(cfg(False).ctx('notes/run.log'), 'a', encoding='utf-8') as f:
            f.write(line + '\n')
    except OSError:
        pass


def git(*a: str, cwd=None) -> sp.CompletedProcess:
    return sp.run(['git', *a], cwd=str(cwd or ROOT), capture_output=True, text=True)


def note(msg: str) -> None:
    """Append one line to `context/notes/master.md`, the ledger of what happened to each branch.

    One file, whether the runner merged a branch itself or the master did, because the question a human
    asks in the morning is "what happened to my eight tasks" and not "which of you did it".
    """
    try:
        with open(cfg(False).ctx('notes/master.md'), 'a', encoding='utf-8') as f:
            f.write(msg + '\n')
    except OSError:
        pass


def default_branch() -> str:
    """The branch worktrees start from and finished work is merged into."""
    r = git('symbolic-ref', '--quiet', '--short', 'refs/remotes/origin/HEAD')
    if r.returncode == 0 and r.stdout.strip():
        return r.stdout.strip().split('/', 1)[-1]
    for b in ('main', 'master'):
        if git('rev-parse', '--verify', '--quiet', b).returncode == 0:
            return b
    return git('branch', '--show-current').stdout.strip() or 'main'


def plan_text(branch: str) -> list[str]:
    """TODO.md as the merge target has it — not as a worktree has it, so a branch mid-merge can't skew picks."""
    rel = os.path.relpath(Path(cfg(False).plan) / 'TODO.md', ROOT)
    r = git('show', f'{branch}:{rel}')
    if r.returncode == 0:
        return r.stdout.splitlines()
    from ps2kit import plan as P  # function-local: plan.py is a leaf, so this adds no import cycle
    return P.lines(Path(cfg(False).plan) / 'TODO.md')


# --- picking the next task -----------------------------------------------------

def tasks(branch: str) -> list[tuple[str, str, set[str]]]:
    """Open tasks in plan/TODO.md order: (id, section, files named on the line)."""
    from ps2kit import plan as P
    ls = plan_text(branch)
    open_ids = {m.group(1) for l in ls if (m := TASK.match(l))}
    out, section = [], ''
    for l in ls:
        if l.startswith('### '):
            section = l[4:]
            continue
        m = TASK.match(l)
        if not m or section.startswith('Stretch') or 'split into' in l:
            continue
        tid, rest = m.group(1), m.group(2)
        # a parent whose own subtasks are open: those run instead of it
        if any(o != tid and o.startswith(tid) and o[len(tid)].isdigit() != tid[-1].isdigit()
               for o in open_ids):
            continue
        after = re.search(r'\(after (\S+?)\)', l)
        if after and after.group(1).rstrip(')') in open_ids:
            continue
        out.append((tid, section, set(P.files_on_line(rest))))
    return out


def family(tid: str) -> str:
    """L3a, L3b, L3 -> L3; A11k4 -> A11. Siblings share it and never run side by side, in any mode."""
    m = re.match(r'[A-Za-z]*\d*', tid)
    return m.group() if m else tid


def pick(running: dict, tried: set, mode: str, branch: str):
    """The next task a free slot may take, or None. See the module docstring for the rules."""
    busy = list(running.values())
    # A section whose heading says its tasks are ordered ("Next (in this order)") runs one task at a time:
    # its second task usually depends on the first having landed, whatever the files say.
    busy_sections = {b[1] for b in busy}
    for tid, section, files in tasks(branch):
        if all(tid != b[0] for b in busy) and git('rev-list', '-1', f'{branch}..task/{tid}').stdout.strip():
            continue  # finished, its branch is waiting to be merged: restarting it would reset the branch
        if any(family(tid) == family(b[0]) for b in busy):
            continue
        if tid in tried or any(tid == b[0] for b in busy):
            continue
        if mode == 'p':
            return tid, section, files
        if (section.startswith('Next') or 'in this order' in section) and section in busy_sections:
            continue
        if all(not (files & b[2]) for b in busy):
            return tid, section, files
    return None


# --- worktrees and panes -------------------------------------------------------

def worktree(n: int, branch: str) -> Path:
    """Slot n's worktree, created once and kept: a warm build directory is worth more than a clean one."""
    d = SLOTS_DIR / f's{n}'
    if not d.is_dir():
        SLOTS_DIR.mkdir(parents=True, exist_ok=True)
        git('worktree', 'add', '-q', '--detach', str(d), branch)
    private = ROOT / (cfg(False).raw.get('paths', {}).get('context') or 'context')
    links = [private, *[Path(p) for p in glob.glob(str(ROOT / '*.iso'))]]
    iso = cfg(False).raw.get('game', {}).get('iso')
    if iso and not os.path.isabs(iso):
        links.append(ROOT / iso)
    for src in links:
        dst = d / src.name
        if src.exists() and not dst.is_symlink() and not dst.exists():
            try:
                dst.symlink_to(src)
            except OSError:
                pass
    return d


def tmux(*a: str, **kw) -> sp.CompletedProcess:
    return sp.run(['tmux', *a], capture_output=True, text=True, **kw)


def pane(name: str, cwd: Path, cmd: str, env=()) -> int:
    """Open cmd in a pane of the window "agents", tagged @kit=name; returns the pane's pid.

    The master gets a window of its own ("master"), because it is the one session a human reads from the
    top rather than glances at: it holds a whole merge conversation, not a task.

    Panes are found by their @kit option rather than their position, so you can break them out, join them or
    swap them around while the run goes on.
    """
    envs = [a for e in env for a in ('-e', e)]
    w = 'master' if name == 'master' else 'agents'
    windows = tmux('list-windows', '-F', '#{window_name}').stdout.split()
    where = ['new-window', '-n', w]
    if w in windows:
        here = sorted((int(h[1:]), p) for p, h in
                      (l.split('\t') for l in tmux('list-panes', '-t', w, '-F',
                                                   '#{pane_id}\t#{@kit}').stdout.splitlines())
                      if re.fullmatch(r's\d+', h))
        k = int(name[1:]) if re.fullmatch(r's\d+', name) else 0
        lower = [p for i, p in here if i < k]
        where = (['split-window', '-h', '-t', lower[-1]] if lower else
                 ['split-window', '-h', '-b', '-t', here[0][1] if here else w])
    r = tmux(*where, '-d', '-c', str(cwd), *envs, '-P', '-F', '#{pane_id} #{pane_pid}', cmd)
    if r.returncode or not r.stdout.split():
        raise SystemExit(f'tmux could not open a pane: {r.stderr.strip()}')
    pane_id, pid = r.stdout.split()[:2]
    tmux('set-option', '-p', '-t', pane_id, '@kit', name)
    tmux('set-option', '-w', '-t', w, 'pane-border-status', 'top')
    tmux('set-option', '-w', '-t', w, 'pane-border-format', ' #{@kit} ')
    tmux('select-layout', '-t', w, 'even-horizontal')
    return int(pid)


def panes() -> dict[str, tuple[str, int]]:
    """{name: (pane id, pid)} for every slot and master pane in this tmux session."""
    out: dict[str, tuple[str, int]] = {}
    for l in tmux('list-panes', '-s', '-F',
                  '#{pane_id}\t#{pane_pid}\t#{@kit}\t#{window_name}').stdout.splitlines():
        pane_id, pid, name, window = (l.split('\t') + ['', '', '', ''])[:4]
        name = name or ('master' if window == 'master' else '')
        if name:
            out[name] = (pane_id, int(pid))
    return out


# --- starting a session --------------------------------------------------------

def agent_cmd(prompt: str, sid: str, model: str, idle: int) -> str:
    """The agent command line for one task. The Stop hook is this very module in --stop-hook mode."""
    stop = [{'hooks': [{'type': 'command',
                        'command': f'python3 {Path(__file__).resolve()} --stop-hook --idle {idle}'}]}]
    lean = [] if os.environ.get('KIT_LEAN') == '0' else \
        ['--tools', 'Bash,Read,Edit,Write', '--setting-sources', 'project,local', '--strict-mcp-config']
    cli = os.environ.get('KIT_AGENT', 'claude')
    return shlex.join([cli, prompt, '--session-id', sid, '--permission-mode', 'bypassPermissions',
                       '--disallowedTools', 'AskUserQuestion', '--model', model, *lean,
                       '--settings', json.dumps({'hooks': {'Stop': stop, 'StopFailure': stop}})])


def brief_for(tid: str, cwd: Path) -> str:
    r = sp.run([sys.executable, str(ROOT / 'kit'), 'plan', 'brief', tid], cwd=str(cwd), capture_output=True, text=True)
    return r.stdout or f'(./kit plan brief {tid} produced nothing: {r.stderr.strip()[:200]})'


def start(n: int, task, branch: str, model: str, idle: int, slots: int):
    tid = task[0]
    d = worktree(n, branch)
    git('reset', '-q', '--hard', cwd=d)
    git('clean', '-fdq', cwd=d)
    co = git('checkout', '-q', '-B', f'task/{tid}', branch, cwd=d)
    if co.returncode:  # e.g. another worktree still holds task/<ID>: never start an agent on the wrong branch
        log(f'slot {n}: checkout task/{tid} failed: {co.stderr.strip()[:200]}')
        return None
    sid = str(uuid.uuid4())
    others = OTHERS if slots > 1 else ''
    prompt = PROMPT.format(tid=tid, n=n, others=others, brief=brief_for(tid, d))
    logfile = cfg(False).ctx(f'notes/slot{n}.log')
    since = logfile.stat().st_size if logfile.exists() else 0  # in append mode tell() is not that
    with open(logfile, 'a', encoding='utf-8') as f:
        f.write(f'\n=== {dt.datetime.now().isoformat(timespec="seconds")} {tid} session {sid}\n')
    env = [f'KIT_PCSX2={n}', 'KIT_RUN=1', f'KIT_CHECK_SLOTS={os.environ.get("KIT_CHECK_SLOTS", max(2, min(slots, 3)))}']
    pid = pane(f's{n}', d, agent_cmd(prompt, sid, model, idle), env)
    log(f'slot {n}: {tid} ({task[1]}; {", ".join(sorted(task[2])) or "no files listed"})')
    return pid, sid, since


MERGE = threading.Lock()  # one merge into the default branch at a time: the runner's, or the master's


# --- the master ----------------------------------------------------------------
#
# The runner can merge a branch that merges cleanly, and it can join both sides of an append-only text
# conflict (`union` below). Everything else needs somebody to read both sides and decide — and for most of
# a run there is nobody. So the run keeps one more session: a long-lived agent in the main checkout whose
# only job is the merges the runner could not do. It is handed one report at a time, with its context
# cleared in between, so report number twenty costs what report number one did.
#
# The alternative is what this kit used to do: log LEFT FOR A HUMAN, push the branch and carry on. That is
# correct and it loses the night — `pick` skips tasks whose branch has unmerged commits, and anything
# `(after X)` on one of them, so a handful of unmergeable branches stops the whole critical path until
# somebody wakes up. Eight of them did exactly that here on 2026-10-15.
#
# What it is *not* allowed to do: touch a worktree, do task work, or publish the shared branch. The last
# one is the one difference from the project this came from, and it is deliberate — see `merge`.

MASTER = """You are the master of an unattended `./kit run`: nobody will answer questions, so never ask one \
— decide, write down why, and go on. Up to {slots} agents are working on `plan/TODO.md` tasks in git \
worktrees under {slots_dir}, each on its own branch `task/<ID>`. The runner merges the branches it can and \
hands you the ones it cannot. It clears your context and sends you one message at a time: `REPORT <ID> slot \
<n>: <session, its commits>` as each one ends. What you decided about earlier reports is at the end of \
`context/notes/master.md`. Handle the report here, in the main checkout, on `{branch}`:

1. If `{branch}` has uncommitted changes that are not yours (a human edits `PLAN.md`, or notes in \
`plan/`), `git stash push -m "master <ID>"` them, and `git stash pop` that entry back when you are done \
whatever happened — merged, aborted or left. Never leave a stash behind.
2. `git merge --no-edit task/<ID>` (if the branch has no commits, just say so). Resolve every conflict by \
keeping both sides' intent, which means reading both sides: `git log {branch}..task/<ID>`, the task's \
journal entry under `research/journal/`, and the journal of whatever landed on `{branch}` meanwhile. Tasks \
run in parallel, so both sides' `plan/TODO.md` and `plan/DONE.md` moves belong, both sides' new struct \
fields belong, and both sides' `PLAN.md` code-map lines belong. Watch for one side being simply older: a \
branch that predates a paragraph on `{branch}` must not revert it, and joining two versions of the same \
sentence leaves half a sentence behind. If a branch ticked or blocked its task by hand, redo that as `./kit \
plan tick <ID>` or `./kit plan block <ID> "<why>"` — never edit a checkbox yourself.
3. Then `./kit plan archive` (sweeps any `[x]` the merge brought in through to `plan/DONE.md`), and `./kit \
check` in the foreground. The agent already passed it on its branch, so you can skip it when `git \
diff --name-only task/<ID> HEAD` lists only `.md` files. Never wait on another `./kit check`: the slots run \
their own. Fix what the merge broke; never "fix" the task's own work — if its journal says it deliberately \
changed a value, the stale thing is the other side. Then commit.
4. `git branch -d task/<ID>`. If you cannot make it pass: `git merge --abort`, leave the branch alone, and \
say why. A branch left unmerged is a result; a broken `{branch}` is not.
5. Append one line to `context/notes/master.md`: `<ID>: merged / conflicts fixed (what) / left (why)`, the \
task's state (ticked, part, blocked), and anything a human must do.
6. Commit every change you made, so `git status` shows only the human's popped-back edits (`context/` is \
git-ignored, so `master.md` itself stays uncommitted). **Do not push `{branch}`.** Publishing the branch \
everyone reads is a decision a person makes after reading the merge, not a side effect of a run finishing \
a task at 4am; the task branches are already on the remote, so nothing is at risk of being lost. If you \
think the merge is worth publishing, put the one-line `git push origin {branch}` in your `master.md` line \
and leave it for them.

Never touch the worktrees under {slots_dir}, and never do a task's own work. When the runner says `RUN \
DONE`, write a short summary for the human at the *top* of `context/notes/master.md` and stop."""

OUTBOX = 'notes/master.outbox'   # reports waiting for the master, one per line, on disk
LOCK = 'notes/master.lock'       # "<ID> <master.md size> <time>" while the master is on a report
MARK = 'notes/master.pane'       # the pane id of the master this code started

_idle_since: float | None = None  # when the master's session was first seen idle while holding the lock


def report(msg: str) -> None:
    """Queue a line for the master. On disk, so a restarted run still delivers it."""
    with open(cfg(False).ctx(OUTBOX), 'a', encoding='utf-8') as f:
        f.write(msg + '\n')


def queued() -> list[str]:
    p = cfg(False).ctx(OUTBOX)
    return p.read_text(encoding='utf-8').splitlines() if p.exists() else []


def lock_holder() -> str:
    try:
        return cfg(False).ctx(LOCK).read_text(encoding='utf-8').split()[0]
    except (OSError, IndexError):
        return ''


def master(branch: str, slots: int) -> None:
    """Open the master's session if it is not up. Started once and adopted by a restarted run.

    One it did not start is replaced — an older run's master has no idle hooks, so there is no way to tell
    when it is ready for the next report — but only while it is holding nothing: no lock, no merge in flight.
    """
    mark = cfg(False).ctx(MARK)
    old = panes().get('master')
    if old and (mark.read_text(encoding='utf-8') if mark.exists() else '') != old[0] \
            and not cfg(False).ctx(LOCK).exists() and not (ROOT / '.git' / 'MERGE_HEAD').exists():
        tmux('kill-pane', '-t', old[0])
        log('the master session in that pane was not this run\'s (no idle hooks); replaced it')
    if 'master' in panes():
        return
    # Its hooks stamp a file when a prompt goes in and another when a turn ends with no background task
    # left to wake it. `master_idle` compares the two — reading the screen instead is how you mistake a
    # diff with "come back... (14 s)" in it for a session that is still working.
    def stamp(name: str, guard: str = '') -> list:
        return [{'hooks': [{'type': 'command',
                            'command': guard + shlex.join(['touch', str(cfg(False).ctx(f'notes/{name}'))])}]}]
    done = stamp('master.idle', "jq -e '(.background_tasks // []) | length == 0' >/dev/null && ")
    hooks = {'UserPromptSubmit': stamp('master.busy'), 'Stop': done, 'StopFailure': done}
    cli = os.environ.get('KIT_AGENT', 'claude')
    cmd = shlex.join([cli, '--model', os.environ.get('KIT_MASTER_MODEL', 'opus'),
                      '--permission-mode', 'bypassPermissions', '--disallowedTools', 'AskUserQuestion',
                      '--settings', json.dumps({'hooks': hooks})])
    for name in ('master.busy', 'master.idle'):  # a killed master's last stamp would read busy forever
        cfg(False).ctx(f'notes/{name}').unlink(missing_ok=True)
    pane('master', ROOT, cmd)
    if up := panes().get('master'):
        mark.write_text(up[0], encoding='utf-8')
    log('master session started (window "master"): the merges this run cannot do go to it')
    time.sleep(20)  # let the session come up before the first report is typed into it


def master_idle() -> bool:
    """Did the master's last turn end after its last prompt? Its own Stop/UserPromptSubmit hooks say so.

    A turn that ends with background tasks still running does not count: they wake it again with no prompt,
    and a report typed in on top of that lands in the middle of someone else's turn.
    """
    def when(name: str) -> float:
        p = cfg(False).ctx(f'notes/{name}')
        return p.stat().st_mtime if p.exists() else 0.0
    return when('master.idle') >= when('master.busy')


def master_busy() -> bool:
    """True while the master is on a report: until it writes its `<ID>: ...` line and the checkout is clean.

    Stale after 90 minutes — it died, or it forgot — at which point the lock is logged and freed, because a
    master that never finishes must not hold every later merge back.
    """
    global _idle_since
    lock = cfg(False).ctx(LOCK)
    try:
        tid, size, since = lock.read_text(encoding='utf-8').split()
    except (OSError, ValueError):
        return (ROOT / '.git' / 'MERGE_HEAD').exists()  # a merge in the checkout we did not start
    md = cfg(False).ctx('notes/master.md')
    new = md.read_bytes()[int(size):].decode(errors='replace') if md.exists() else ''
    _idle_since = (_idle_since or time.time()) if master_idle() else None
    # Done when it has written its line — or when it has sat idle two minutes, which is a report it had
    # nothing to do for (already merged, no commits). Never while a merge is still open in the checkout.
    done = (re.search(rf'^{re.escape(tid)}:', new, re.M) or
            time.time() - (_idle_since or time.time()) >= 120) \
        and not (ROOT / '.git' / 'MERGE_HEAD').exists()
    if not done and time.time() - float(since) < 90 * 60:
        return True
    if not done:
        log(f'{tid}: the master has held the {ROOT.name} checkout for 90 min with no master.md line; '
            f'freeing it')
    lock.unlink(missing_ok=True)
    return False


def enter(pane_id: str, text: str) -> bool:
    """Type text into an agent's pane and submit it; True once its input box is empty again.

    Keys sent straight after one another get mixed up — a leftover character before `/clear`, a report that
    arrives as `/resume` — so: clear the box (Ctrl-U), paste the text as one bracketed paste, give the
    session a moment to take it, then press Enter until the box is empty.
    """
    tmux('send-keys', '-t', pane_id, 'C-u')
    tmux('set-buffer', '-b', 'kit', text)
    tmux('paste-buffer', '-p', '-d', '-b', 'kit', '-t', pane_id)
    for _ in range(5):
        time.sleep(2)
        tmux('send-keys', '-t', pane_id, 'Enter')
        time.sleep(3)
        screen = tmux('capture-pane', '-p', '-t', pane_id).stdout
        box = [l for l in screen.splitlines() if l.startswith('❯')]
        if box and box[-1].strip() == '❯':
            return True
    return False


def hand(q: list[str], branch: str, slots: int) -> None:
    """Clear the master's context and give it its instructions plus the next queued report."""
    global _idle_since
    m = panes().get('master')
    if not m or not master_idle():
        return
    text = ' '.join(MASTER.format(slots=slots, branch=branch, slots_dir=SLOTS_DIR).split('\n'))
    if not (enter(m[0], '/clear') and enter(m[0], text + ' ' + q[0])):
        return  # not taken: the line stays queued for the next look
    cfg(False).ctx(OUTBOX).write_text(''.join(l + '\n' for l in q[1:]), encoding='utf-8')
    _idle_since = None
    if r := re.match(r'REPORT (\S+)', q[0]):
        md = cfg(False).ctx('notes/master.md')
        size = md.stat().st_size if md.exists() else 0
        cfg(False).ctx(LOCK).write_text(f'{r.group(1)} {size} {time.time()}', encoding='utf-8')
        log(f'{r.group(1)}: handed to the master')


def flush(branch: str, slots: int) -> None:
    """Hand the master its next report, if it has one and is ready for it."""
    if not (q := queued()):
        return
    master(branch, slots)  # bring it up at once, even while a merge holds the checkout
    if master_busy() or not MERGE.acquire(blocking=False):
        return  # one thing at a time in the checkout: the master's report, or the runner's own merge
    try:
        hand(q, branch, slots)
    finally:
        MERGE.release()

# --- merging a finished branch -------------------------------------------------

def joinable(f: str) -> bool:
    """True when the runner may join both sides of a conflict in `f` itself, rather than ask a human.

    Line-oriented text, where a line is a whole record and two agents each added records: the task list
    and its archive, the journal, and the oracle's address list.

    Prose stays off the list even where it is only ever appended to, `PLAN.md`'s code map included. A
    union joins *lines*, and two sides that both reflowed one paragraph give it no line to agree on, so
    it stitches them into something that reads like a sentence and is not one — a tail left stranded in
    the middle of a bullet, which a reader believes. The port repo had to go back and repair exactly that
    (openmx 10dcc05) after one such merge. A paragraph needs reading, so it goes to whoever can read it.

    Off config, not off this repo's layout: `[paths]` and `[oracle] addresses` all move.
    """
    c = cfg(False)

    def rel(p) -> str | None:
        p = Path(p)
        return str(p.relative_to(ROOT)) if p.is_relative_to(ROOT) else None

    if f == rel(c.oracle_file):
        return True
    return any(f.startswith(f'{d}/') and f.endswith('.md')
               for d in (rel(c.plan), rel(c.journal)) if d)


def addresses(text: str, where: str) -> str | None:
    """The oracle's address list after a union join: drop a line both sides added, refuse a real clash.

    An address list is append-only, so joining both sides is right — while the two sides agree. The
    same name at two different addresses is the case where they do not, and nothing downstream picks
    between them: `Kit::addr` in Rust and `oracle.entries` here both take the first line that matches,
    in file order. Resolving it by picking a side would point the oracle at whichever worktree the run
    merged first and say nothing about it — a wrong-address oracle call, which is the failure this kit
    exists to catch. So that one goes to a human instead. `./kit oracle add` refuses the same clash; a
    merge must not become the way around it.
    """
    from ps2kit import oracle as O
    try:
        rows = O.parse(text, where)
    except ValueError as e:
        log(f'{e}')
        return None
    at: dict[tuple[str, str], int] = {}
    for name, prog, addr, _note in rows:
        if at.setdefault((name, prog), addr) != addr:
            log(f'{where}: both sides name {name}{f" in {prog}" if prog else ""} and disagree about its '
                f'address ({at[(name, prog)]:#x} / {addr:#x}) — a human picks that one')
            return None
    kept, seen = [], set()
    for line in text.split('\n'):
        entry = ' '.join(line.partition('#')[0].split())
        if entry and entry in seen:   # the same name and address added on both sides
            continue
        seen.add(entry)
        kept.append(line)
    return '\n'.join(kept)


def union(files: list[str], tid: str, branch: str) -> bool:
    """Join both sides of a conflict in every file `joinable` allows: two agents both added lines, and
    both sets belong. Then drop a task line that came out twice, any TODO line whose task DONE.md already
    has, and — in the address list, which cannot be joined blind — a name both sides gave the same address.
    """
    from ps2kit import plan as P
    orel = str(Path(cfg(False).oracle_file).relative_to(ROOT)) \
        if Path(cfg(False).oracle_file).is_relative_to(ROOT) else None
    for f in sorted(files, key=lambda f: not f.endswith('DONE.md')):  # DONE.md first: TODO.md reads it
        sides = []
        for stage in (2, 1, 3):  # ours, base, theirs
            t = cfg(False).ctx(f'notes/union.{stage}')
            t.write_text(git('show', f':{stage}:{f}').stdout, encoding='utf-8')
            sides.append(str(t))
        out = git('merge-file', '-p', '--union', *sides)
        if out.returncode < 0:
            return False
        text = out.stdout
        if f.endswith(('TODO.md', 'DONE.md')):
            done_path = Path(cfg(False).plan) / 'DONE.md'
            done = {m.group(1).rstrip('.') for l in P.lines(done_path)
                    if (m := ANY_TASK.match(l)) and '- [x]' in l} if f.endswith('TODO.md') else set()
            mine = git('show', f'task/{tid}:{f}').stdout.splitlines()
            kept, seen = [], {}
            for l in text.split('\n'):
                if m := ANY_TASK.match(l):
                    k = m.group(1).rstrip('.')
                    if k in done:
                        continue
                    if k in seen:  # keep the branch's copy of its own task, the target's copy of the rest
                        if (k == tid) == (l in mine):
                            kept[seen[k]] = l
                        continue
                    seen[k] = len(kept)
                kept.append(l)
            text = '\n'.join(kept)
        elif f == orel and (text := addresses(text, f)) is None:
            return False
        (ROOT / f).write_text(text, encoding='utf-8')
        git('add', f)
    return True


def merge(tid: str, branch: str) -> str | None:
    """Merge task/<tid> into the default branch: merge, archive ticks, run the tests, delete the branch.

    Returns None when it landed, else why it was left alone (the target branch is then as it was, and the
    branch is still there for a human to look at).
    """
    # The master's queued reports go first: back-to-back runner merges starve it, and a report it never
    # gets is a branch nobody looks at. But never wait on it longer than 30 minutes — a master that has
    # stopped taking its line must not hold every later merge back.
    t0 = time.time()
    while queued() and time.time() - t0 < 30 * 60:
        time.sleep(10)
    if queued():
        log(f'{tid}: the master has not taken its queued reports in 30 min; merging without waiting')
    with MERGE:
        # The master is mid-merge in the checkout, or somebody left conflicts there (a half-applied stash
        # pop): queue behind it. Merging over unmerged paths "resolves" theirs and commits that instead of
        # the branch, which is a wrong merge that looks like a clean one.
        t0 = time.time()
        while master_busy() or git('diff', '--name-only', '--diff-filter=U').stdout.strip():
            if time.time() - t0 > 60 * 60:  # hard stop: hand it over rather than stall the run
                return 'the main checkout stayed busy (the master, or unmerged paths) for 60 min'
            time.sleep(10)
        # Merges go into the shared branch whatever the checkout happens to be on: worktrees start from it
        # and `pick` reads its task list, so a merge onto somebody's topic branch leaves the shared branch
        # stale and its tasks open.
        cur = git('branch', '--show-current').stdout.strip()
        if cur != branch:
            if git('checkout', '-q', branch).returncode:
                return f'the main checkout is on {cur or "a detached HEAD"} and would not switch to {branch}'
            # Commits made over there meanwhile (the master's fixes, a plan edit) belong on the shared
            # branch too, when they fast-forward onto it.
            ff = bool(cur) and not git('merge', '-q', '--ff-only', cur).returncode
            log(f'{tid}: the main checkout was on {cur or "a detached HEAD"}; switched it to {branch} for '
                f'the merge' + (f' ({branch} fast-forwarded to {cur})' if ff else
                                f' ({cur} does not fast-forward, so its commits stay there)'))
        # somebody's uncommitted edits in the checkout: out of the way for the merge, back right after
        stashed = bool(git('status', '--porcelain', '--untracked-files=no').stdout.strip()) and \
            not git('stash', 'push', '-q', '-m', f'kit run {tid}').returncode
        pre = git('rev-parse', 'HEAD').stdout.strip()
        try:
            if git('merge', '--no-edit', f'task/{tid}').returncode:
                files = git('diff', '--name-only', '--diff-filter=U').stdout.split()
                if [f for f in files if not joinable(f)] or not union(files, tid, branch):
                    git('merge', '--abort')
                    return f'conflicts in {", ".join(files)}'
                git('commit', '-q', '--no-edit')
                log(f'{tid}: joined both sides of {", ".join(files)}')
            sp.run([sys.executable, str(ROOT / 'kit'), 'plan', 'archive'], cwd=str(ROOT), capture_output=True)
            plan_files = [str(Path(cfg(False).plan).relative_to(ROOT) / n) for n in ('TODO.md', 'DONE.md')]
            if git('status', '--porcelain', '--', *plan_files).stdout.strip():
                git('commit', '-qm', f'{tid}: archive ticked tasks', '--', *plan_files)
        finally:
            if stashed and git('stash', 'pop', '-q').returncode:
                git('reset', '-q', '--hard')
                log(f'{tid}: the checkout\'s own edits clash with the merge; kept in git stash "kit run {tid}"')
        if git('merge-base', '--is-ancestor', f'task/{tid}', 'HEAD').returncode:
            return f'the merge did not land in {branch} (it changed underneath); merge it again'
        # the agent's ./kit done already passed the tests on the branch: rerun them only if the merged code
        # differs from what it tested (a markdown-only difference is not code)
        merged_diff = git('diff', '--name-only', f'task/{tid}', 'HEAD').stdout.split()
        rc, out = 0, ''
        if not all(f.endswith('.md') for f in merged_diff):
            chk = sp.run([sys.executable, str(ROOT / 'kit'), 'check'], cwd=str(ROOT), capture_output=True, text=True)
            rc, out = chk.returncode, chk.stdout
        if rc:
            git('reset', '-q', '--hard', pre)
            return 'the tests fail after the merge: ' + ' / '.join(out.splitlines()[:4])[:400]
        git('branch', '-d', f'task/{tid}')
        line = next((l for l in plan_text(branch) if f'**{tid}**' in l), '')
        state = 'blocked' if line.startswith('- [~]') else 'part' if line else 'ticked'
        added = git('diff', pre, 'HEAD', '--', *plan_files).stdout
        sus = [l[1:].strip()[:200] for l in added.splitlines() if l.startswith('+') and SUSPECT.search(l)]
        msg = f'{tid}: merged by the runner, {state}' + \
              (f'; SUSPECT BLOCK (the run drives the emulator itself), recheck: {sus[0]}' if sus else '')
        note(msg)
        log(msg)
        # The merge target is never pushed from here: publishing the branch everyone reads is a thing
        # somebody chooses to do, not a side effect of a run finishing a task at 4am. What the run does
        # instead is make sure the task's own branch is on the remote, and drop it only once the remote
        # has the merge — see `drop_if_merged`. A failed push is logged, not fatal: the merge landed.
        from ps2kit.checks import drop_if_merged
        if out := drop_if_merged(ROOT, f'task/{tid}'):
            log(f'{tid}: {out}')
        return None


HANDED: dict[str, bool] = {}  # branches given to the master -> whether the remote copy has been swept


def finish(tid: str, n: int, sid: str, branch: str, has_master: bool = True) -> None:
    """After a slot's session: merge its branch here. What this cannot merge goes to the master."""
    commits = git('log', '--format=%s', f'{branch}..task/{tid}').stdout.splitlines()
    if not commits:
        git('branch', '-D', f'task/{tid}')
        note(f'{tid}: no commits — nothing to merge')
        log(f'{tid}: no commits — nothing to merge')
        return
    if not (why := merge(tid, branch)):
        return
    from ps2kit.checks import push  # an unmerged branch is the one copy of that work: get it off this disk
    if out := push(ROOT, f'task/{tid}'):
        log(f'{tid}: {out}')
    if not has_master:  # no tmux to put one in: the old behaviour, and the branch is on the remote
        note(f'{tid}: LEFT FOR A HUMAN — {why}')
        log(f'{tid}: LEFT FOR A HUMAN — {why}. Its {len(commits)} commits are on task/{tid}: '
            f'{" / ".join(commits)[:300]}')
        return
    HANDED.setdefault(tid, False)
    report(f'REPORT {tid} slot {n}: session {sid} ended; {len(commits)} commits: '
           f'{" / ".join(commits)[:600]}. The runner could not merge it: {why}')
    log(f'{tid}: the runner could not merge it ({why}); queued for the master')



def sweep_handed() -> None:
    """Drop the remote copy of a branch the master merged, once the remote can be seen to hold the merge.

    The runner's own merges do this at the end of `merge`. The master's cannot: it is told not to push, and
    it has no business deciding what the remote looks like. So the runner watches for the branch to go —
    the master deletes it only when the merge passed — and then applies the same rule. A branch the master
    left alone stays on the remote, which is exactly where you want the one copy of unmerged work.
    """
    from ps2kit.checks import drop_if_merged
    for tid in [t for t, swept in HANDED.items() if not swept]:
        if lock_holder() == tid or \
                not git('rev-parse', '--verify', '--quiet', f'refs/heads/task/{tid}').returncode:
            continue  # still with the master, or still a branch here: not merged yet
        HANDED[tid] = True
        if out := drop_if_merged(ROOT, f'task/{tid}'):
            log(f'{tid}: {out}')


# --- watching a session --------------------------------------------------------

def transcript(sid: str) -> str | None:
    return next(iter(glob.glob(os.path.expanduser(f'~/.claude/projects/*/{sid}.jsonl'))), None)


def stats(sid: str) -> str:
    """'N min, N calls, context Nk first, Nk peak' for the exit line. Watch the peak over a run: if it keeps
    creeping up, or more tasks come back blocked, a context cut went too far."""
    calls, ctx, ts = 0, [], []
    try:
        for l in open(transcript(sid) or os.devnull, encoding='utf-8', errors='replace'):
            d = json.loads(l)
            ts += [d['timestamp']] if 'timestamp' in d else []
            if d.get('type') == 'assistant' and (u := d.get('message', {}).get('usage')):
                calls += 1
                ctx.append(sum(u.get(k, 0) for k in ('input_tokens', 'cache_read_input_tokens',
                                                     'cache_creation_input_tokens')))
    except (OSError, ValueError, KeyError):
        pass
    if not ctx or not ts:
        return 'no transcript'
    span = (dt.datetime.fromisoformat(ts[-1].replace('Z', '+00:00')) -
            dt.datetime.fromisoformat(ts[0].replace('Z', '+00:00')))
    return f'{int(span.total_seconds()) // 60} min, {calls} calls, context {ctx[0] // 1000}k first, {max(ctx) // 1000}k peak'


def stalled(pid: int, sid: str, limit: int) -> str | None:
    """Why a live session looks stuck, else None: quiet for `limit` seconds with no tool running (a prompt or
    a dialog the idle hook never sees), or quiet for longer than the Bash timeout even with one (a hung tool).
    """
    t = transcript(sid)
    quiet = time.time() - os.path.getmtime(t) if t and os.path.exists(t) else 0
    busy = sp.run(['pgrep', '-P', str(pid)], capture_output=True).returncode == 0
    if quiet > 65 * 60 or (quiet > limit and not busy):
        return f'no transcript activity for {int(quiet // 60)} min' + \
               (' with a tool still running' if busy else ' at a prompt')
    return None


def limit_only(sid: str, slot_log: Path, since: int) -> dt.datetime | None:
    """If the session did nothing but hit the usage limit: discard its transcript, return the reset time.

    This matters more than it looks. A limited session produced no work, so counting it as an attempt would
    quietly drop the task from the night; and keeping its transcript would make the next run resume a session
    that never started.
    """
    t = transcript(sid)
    text = open(t, encoding='utf-8', errors='replace').read() if t else ''
    try:
        with open(slot_log, encoding='utf-8', errors='replace') as f:
            f.seek(since)
            text += f.read()
    except OSError:
        pass
    m = re.findall(r'limit\D{0,12}resets? (\d+(?::\d+)?\s?[ap]m)', text, re.I)
    if not m or '"type":"tool_use"' in text:
        return None
    if t:
        import shutil
        os.remove(t)
        shutil.rmtree(t[:-6], ignore_errors=True)  # and its sidecar dir, if this build keeps one
    s = m[-1].replace(' ', '').lower()
    when = dt.datetime.strptime(s, '%I:%M%p' if ':' in s else '%I%p')
    now = dt.datetime.now()
    at = now.replace(hour=when.hour, minute=when.minute, second=0, microsecond=0)
    if at < now - dt.timedelta(minutes=10):
        at += dt.timedelta(days=1)
    return max(at, now)


_weekly = [-1e9, None]


def weekly() -> float | None:
    """The account's 7-day usage %, from the endpoint `/usage` reads (the local login's OAuth token; costs no
    tokens), checked at most every 5 minutes. None when it can't be read, which means "don't cap"."""
    if time.time() - _weekly[0] < 300:
        return _weekly[1]
    import urllib.request
    try:
        tok = json.load(open(os.path.expanduser('~/.claude/.credentials.json')))['claudeAiOauth']['accessToken']
        req = urllib.request.Request('https://api.anthropic.com/api/oauth/usage',
                                     headers={'Authorization': f'Bearer {tok}',
                                              'anthropic-beta': 'oauth-2025-04-20'})
        _weekly[1] = json.load(urllib.request.urlopen(req, timeout=10))['seven_day']['utilization']
    except Exception:
        _weekly[1] = None
    _weekly[0] = time.time()
    return _weekly[1]


# --- the idle stop hook --------------------------------------------------------

def stop_hook(idle: int) -> int:
    """Stop hook for a session this runner started: end it once it has been idle for `idle` seconds.

    A finished agent otherwise sits at its prompt and the loop can never start the next task. "Idle" means no
    new user or tool-result entry in the transcript — so typing into the pane keeps the session alive and you
    can take it over by hand. A session with a background task still running gets a much longer grace period,
    because a background task normally ends and wakes the session by itself.

    A usage limit is deliberately *not* idleness: Claude Code prints "continuing automatically at <reset>" and
    resumes the session itself, so the waiter leaves such a session alone.
    """
    try:
        ev = json.load(sys.stdin)
    except Exception:
        return 0
    t = ev.get('transcript_path')
    if not t or not os.path.exists(t):
        return 0
    if ev.get('background_tasks'):
        idle = max(idle, int(os.environ.get('KIT_IDLE_BG', 1800)))
    try:
        n = _user_entries(t)
        plat.spawn_detached([sys.executable, str(Path(__file__).resolve()), '--stop-waiter',
                             str(os.getppid()), t, str(n), str(idle)])
    except Exception:
        pass
    return 0


def _user_entries(t: str) -> int:
    try:
        with open(t, encoding='utf-8', errors='replace') as f:
            return sum(1 for l in f if '"type":"user"' in l)
    except OSError:
        return -1


def stop_waiter(pid: int, t: str, n: int, idle: int) -> int:
    time.sleep(idle)
    if _user_entries(t) != n:
        return 0  # the human typed, or a tool came back: the session is alive, leave it
    try:
        text = open(t, encoding='utf-8', errors='replace').read()
    except OSError:
        text = ''
    tail = text[text.rfind('continuing automatically'):] if 'continuing automatically' in text else ''
    if tail and '"tool_use"' not in tail:
        return 0  # waiting out a usage limit; Claude Code resumes it at the reset
    plat.terminate(pid)
    return 0


# --- sequential fallback -------------------------------------------------------

def sequential(branch: str, model: str, idle: int, pause: int, force: bool, once: bool = False) -> int:
    """No tmux (or `./kit run 1 --no-tmux`): one session at a time, in this checkout, on the current branch.

    Honest about the trade: no worktrees, so a task commits straight onto the branch you are on, and the
    emulator copy is slot 1. `once` stops after the first task, which is how you try the runner out
    before letting it loose on the backlog overnight.
    """
    log(f'sequential run on {branch} ({"one task, then stop" if once else "one session at a time"}, '
        f'committing in this checkout)')
    tried: set[str] = set()
    while (task := pick({}, tried, 'p', branch)):
        if not force and (w := weekly()) is not None and w >= WEEKLY_MAX:
            log(f'weekly usage {w}% >= {WEEKLY_MAX}%: waiting for the reset (--force to go on)')
            time.sleep(300)
            continue
        tid = task[0]
        sid = str(uuid.uuid4())
        prompt = PROMPT.format(tid=tid, n=1, others='', brief=brief_for(tid, ROOT))
        log(f'{tid}: starting ({task[1]})')
        sp.run(agent_cmd(prompt, sid, model, idle), shell=True, cwd=str(ROOT),
               env={**os.environ, 'KIT_PCSX2': '1', 'KIT_RUN': '1'})
        log(f'{tid}: session ended ({stats(sid)})')
        if r := limit_only(sid, cfg(False).ctx('notes/slot1.log'), 0):
            log(f'{tid}: only hit the usage limit; sleeping until {r:%H:%M}')
            time.sleep(max(0, (r - dt.datetime.now()).total_seconds()) + pause)
            continue
        tried.add(tid)
        if once:
            log(f'{tid}: done (--once)')
            return 0
        time.sleep(pause)
    log('sequential run done: no task left that it may start')
    return 0


# --- the loop ------------------------------------------------------------------

def cmd_run(argv: list[str]) -> int:
    from ps2kit.cli import parser  # function-local: nothing in the kit imports cli at module level
    ap = parser('run', __doc__)
    ap.add_argument('slots', nargs='?', default='3',
                    help="how many agents at once (default 3), or 'once' for a single task then stop")
    ap.add_argument('--mode', choices=['p', 'd'], default='d',
                    help="p = plan order, overlapping files allowed; d = different files (default)")
    ap.add_argument('--force', action='store_true', help='keep starting tasks past the weekly usage cap')
    ap.add_argument('--idle', type=int, default=90, help='seconds of quiet before a session is ended (90)')
    ap.add_argument('--stall', type=int, default=20 * 60, help='seconds of quiet that counts as stalled (1200)')
    ap.add_argument('--pause', type=int, default=60, help='seconds between a session ending and the next (60)')
    ap.add_argument('--model', default=os.environ.get('KIT_MODEL', 'opus'), help='the agent model (opus)')
    ap.add_argument('--no-tmux', action='store_true', help='force the single sequential session')
    ap.add_argument('--stop-hook', action='store_true', help=argparse.SUPPRESS)
    ap.add_argument('--stop-waiter', nargs=4, metavar=('PID', 'TRANSCRIPT', 'N', 'IDLE'),
                    help=argparse.SUPPRESS)
    a = ap.parse_args(argv)

    once = str(a.slots) == 'once'
    if once:
        a.slots = 1
    else:
        try:
            a.slots = int(a.slots)
        except ValueError:
            ap.error(f"slots must be a number or 'once', not {a.slots!r}")

    if a.stop_hook:
        return stop_hook(a.idle)
    if a.stop_waiter:
        pid, t, n, idle = a.stop_waiter
        return stop_waiter(int(pid), t, int(n), int(idle))

    if plat.WINDOWS:
        print('./kit run drives tmux and git worktrees; the Windows edition of the kit does this '
              'differently. Run tasks one at a time there.', file=sys.stderr)
        return 2
    branch = default_branch()
    if git('rev-parse', '--verify', '--quiet', branch).returncode:
        print(f'no branch {branch!r} to merge into — commit something first', file=sys.stderr)
        return 2
    cfg(False).ctx('notes/.keep').write_text('', encoding='utf-8')

    have_tmux = bool(sp.run(['which', 'tmux'], capture_output=True).returncode == 0)
    if once or a.no_tmux or not have_tmux or a.slots <= 1:
        if not have_tmux and not a.no_tmux:
            log('tmux is not installed: falling back to one sequential session '
                '(install tmux for parallel slots, panes you can attach to, and worktrees)')
        return sequential(branch, a.model, a.idle, a.pause, a.force, once)
    if not os.environ.get('TMUX'):
        print(f'Run me inside tmux, so the sessions get panes you can attach to:\n'
              f'    tmux new -s kit ./kit run {a.slots} --mode {a.mode}', file=sys.stderr)
        return 2

    running: dict[int, tuple] = {}
    procs: dict[int, tuple] = {}
    tried: set[str] = set()
    free_at = {n: dt.datetime.min for n in range(1, a.slots + 1)}
    merging: list[threading.Thread] = []
    stall_said: set[str] = set()
    cap_said = False
    log(f'run: {a.slots} slots, mode {a.mode}, merging into {branch}'
        + (f', weekly usage {weekly()}%' if not a.force else ' (weekly cap ignored)'))

    done = False  # RUN DONE has gone to the master
    if queued():
        master(branch, a.slots)  # reports a previous run left; otherwise it starts when a merge needs it

    # branches a previous run never merged (it was stopped, or a session ended while nothing watched)
    held = {l.removeprefix('branch refs/heads/') for l in
            git('worktree', 'list', '--porcelain').stdout.splitlines() if l.startswith('branch ')}
    open_ids = {m.group(1) for l in plan_text(branch) if (m := TASK.match(l))}
    runlog = cfg(False).ctx('notes/run.log')
    handed_before = runlog.read_text(encoding='utf-8', errors='replace') if runlog.exists() else ''
    for b in git('branch', '--format=%(refname:short)', '--list', 'task/*').stdout.split():
        tid = b.removeprefix('task/')
        if b in held or tid not in open_ids:
            continue
        if f'{tid}: queued for the master' in handed_before or f'{tid}: handed to the master' in handed_before:
            HANDED.setdefault(tid, False)  # it has it, or it left it alone: either way not ours to retry
            continue
        if git('log', '--format=%s', f'{branch}..{b}').stdout.strip():
            log(f'{tid}: finished while no run was up; merging it now')
            tried.add(tid)
            t = threading.Thread(target=finish, args=(tid, 0, '(unknown)', branch))
            t.start()
            merging.append(t)
    for n in free_at:  # idle slots must let go of old task/<ID> branches, or start() cannot take them
        if (SLOTS_DIR / f's{n}').is_dir():
            git('checkout', '-q', '--detach', cwd=SLOTS_DIR / f's{n}')

    while True:
        for n in list(free_at):
            if n in procs and not plat.alive(procs[n][0]):
                (pid, sid, since), task = procs.pop(n), running.pop(n)
                log(f'slot {n}: {task[0]} exited ({stats(sid)})')
                sp.Popen([sys.executable, str(ROOT / 'kit'), 'pcsx2', 'stop'], env={**os.environ, 'KIT_PCSX2': str(n)},
                         stdout=sp.DEVNULL, stderr=sp.DEVNULL)
                if reset := limit_only(sid, cfg(False).ctx(f'notes/slot{n}.log'), since):
                    free_at[n] = reset + dt.timedelta(seconds=a.pause)
                    log(f'slot {n}: {task[0]} only hit the usage limit; transcript dropped, the task goes '
                        f'back in the pool and the slot sleeps until {reset:%H:%M}')
                else:
                    tried.add(task[0])  # one attempt per run: ticked, blocked or failed, a human looks next
                    git('checkout', '-q', '--detach', cwd=SLOTS_DIR / f's{n}')
                    t = threading.Thread(target=finish, args=(task[0], n, sid, branch))
                    t.start()
                    merging.append(t)
                    free_at[n] = dt.datetime.now() + dt.timedelta(seconds=a.pause)
            if n in procs and procs[n][1] not in stall_said and \
                    (why := stalled(procs[n][0], procs[n][1], a.stall)):
                stall_said.add(procs[n][1])
                log(f'slot {n}: {running[n][0]} looks stalled ({why}); attach to pane s{n} to see')
                print('\a', end='', flush=True)
            if n not in procs and dt.datetime.now() >= free_at[n]:
                if not a.force and (w := weekly()) is not None and w >= WEEKLY_MAX:
                    if not cap_said:
                        log(f'weekly usage {w}% >= {WEEKLY_MAX}%: no new tasks until it resets '
                            f'(restart with --force to go on now)')
                        cap_said = True
                    continue
                cap_said = False
                if task := pick(running, tried, a.mode, branch):
                    if p := start(n, task, branch, a.model, a.idle, a.slots):
                        procs[n], running[n] = p, task
                    else:
                        tried.add(task[0])
        merging = [t for t in merging if t.is_alive()]
        # The merges wait on each other and on the master's reports, so RUN DONE goes out only once they
        # are all in — otherwise the master is told the run is over and then handed three more reports.
        if not done and not procs and not merging and not pick(running, tried, a.mode, branch) \
                and all(dt.datetime.now() >= t for t in free_at.values()):
            done = True
            done_at = time.time()
            if 'master' in panes() or queued():
                report('RUN DONE')
            log(f'run done; tried this run: {", ".join(sorted(tried)) or "none"}')
        flush(branch, a.slots)
        sweep_handed()
        # Stay up while the master still has something: a report typed into a session nobody is watching
        # for is a merge that silently never happens. Capped, so a master that died cannot pin the run.
        if done and not merging and not queued() and \
                (not master_busy() or time.time() - done_at > 90 * 60):
            sweep_handed()
            if left := [t for t, swept in HANDED.items() if not swept]:
                log(f'still with the master, or left unmerged on their branch: {", ".join(sorted(left))}')
            log('the master session stays up for the next run; its notes are in context/notes/master.md')
            return 0
        time.sleep(10)


def main(argv: list[str] | None = None) -> int:
    return cmd_run(list(sys.argv[1:] if argv is None else argv))


if __name__ == '__main__':
    sys.exit(main())
