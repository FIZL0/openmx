# Task mechanics in detail

The core rules are in `AGENT.md` and `PLAN.md`; this is the detail behind them. Read a section when you need it
(`./kit plan "<section>"`), not the file whole.

## Plan commands

`plan/TODO.md` is plain markdown so a human can skim it, but every change goes through the command, which keeps
the format consistent and the archive (`plan/DONE.md`) in step. Why a tool and not a file: a task list you read
is a task list you read *all of*, every turn, for the rest of the session. The list is a few hundred lines;
`./kit plan next` is three.

| Command | Does |
|---|---|
| `./kit plan` | the index: sections and how many are open in each |
| `./kit plan next` | the first open task |
| `./kit plan T7` | that task: its own file if it has one, else its line |
| `./kit plan brief T7` | everything you need to start it, in one call |
| `./kit plan tick T7` | done → moves it to `plan/DONE.md` |
| `./kit plan block T7 "<why + what unblocks it>"` | blocked, with the reason recorded |
| `./kit plan unblock T7 [note]` | un-blocked |
| `./kit plan add T "text" --files a.rs,dir/** [--after X] [--test name] [@ where]` | a new task, next free ID in that prefix; fields, never prose, say its files and what it waits on |
| `./kit plan move T7 top\|before T3\|after T3\|in <section>` | reprioritise |
| `./kit plan note T7 "<text>"` | add a line to its file |
| `./kit plan newid T` | the next free ID in a prefix |
| `./kit plan archive` | sweep finished lines into `plan/DONE.md` (after merges or hand edits) |

`brief` assembles the task's line, its plan file, its journal directory (the newest entry pasted in, capped),
and each file the task names with its length and last few commits. One call instead of eight, and you start
with the same context every time.

## Journal tools

`./kit journal new <ID> <slug>` creates `research/journal/YYYY-MM-DD-<ID>-<slug>/` and stubs with the required
sections; `./kit journal state <ID>` answers "whose move is it"; `./kit journal finish` closes an entry. The
journal is the project's memory: what you found, what the original does, which addresses you used, what you
measured, and what you could not check. Write it for someone picking this up months later with no memory of
yesterday.

## Wrap-up and pushing

`./kit done` runs the test command, runs the float lint if the port has one, refuses to commit if the tests
fail, ticks or blocks the task, commits everything including the journal entry, and pushes the branch it
committed on. One call, so a wrap-up cannot be half-done: code committed with the task still open confuses the
next session most, and code committed only on this disk loses work outright.

It pushes the branch you are on when that branch is a task's own (an agent's `task/<ID>` in its worktree, a
topic branch here), so the branch of a task whose merge went wrong is on the remote. The **shared** branch is
pushed only by `./kit run`, after a merge has passed the test gate (`[git] push_merge`, on by default); a
wrap-up in the main checkout prints the one-line `git push` instead. A merged `task/<ID>` is dropped from the
remote once the remote holds the merge. A push that cannot be made is one logged line, never a failed task:
the commit is safe locally and the next wrap-up pushes it. `[git] push = false` in `kit.toml` turns it all off;
`[git] push_merge = false` makes a run's merges wait for a person to push them.

## Unattended runs

```sh
./kit run once                  # one task, then stop
./kit run 3                     # three agents at a time, each in its own worktree
./kit run 3 --mode p            # ... strictly in TODO.md order (mode d, the default, keeps them off each other's files)
./kit run --dry-run 3           # what would start, and why the rest wait
./kit stop                      # running tasks finish and merge, nothing new starts
```

The loop is [agent-runner](https://github.com/FIZL0/agent-runner) (install: SETUP.md step 1). Each agent gets a
git worktree on a `task/<ID>` branch and its own emulator copy (`KIT_PCSX2=N`), and each branch is merged in a
separate merge worktree when its session ends, tested, and pushed.

**A conflict the runner cannot join goes to an agent session, not to you.** Files where a line is a whole
record (the task list, the journal, the oracle's address list) are joined mechanically; a name given two
addresses in the address list is refused (`./kit oracle join`). Anything else gets one session per merge in the
merge worktree, which resolves and commits; the runner then tests it as usual. A branch that session gives up
on is parked as `wip/<ID>` and the task opens again. `[run] fix_merges = false` parks them all. See
`docs/WORKFLOW.md` for what to expect from a long run and how to pick up the pieces afterwards.
