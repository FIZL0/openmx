# PLAN — how a task goes from the list to a commit

## Continue

"Continue" is one task, start to finish, then stop and report what you did and did not verify:

```sh
./kit plan next                 # the first open task
./kit plan brief T7             # its line, own file, journal, the files it names: use it instead of gathering
./kit check                     # the tests you touched
./kit journal new T7 walker     # the entry, ending in what you could not verify
./kit done T7 "port the walker" # tests, tick, commit, push your branch
```

One task per iteration: don't merge related tasks or skip ahead. Too big for one sitting → split it first
(`plan/<ID>.md` files, `./kit plan add`), then take the first part.

## The task list

Open tasks are in `plan/TODO.md`, finished in `plan/DONE.md`, human-only in `plan/HUMAN.md`, background in
`plan/REFERENCE.md`. Don't read TODO.md or edit checkboxes by hand: use `./kit plan` (`next`, `<ID>`, `brief`,
`tick`, `block`, `unblock`, `add`, `move`, `note`, `newid`, `archive`; table in `docs/agent/TASKS.md`).
A new task names its files and what it waits on as fields, never prose:
`./kit plan add T "text" --files a.rs,dir/** [--after X] [--test name]` (`plan add` refuses a line with neither).

## Journal

`research/journal/YYYY-MM-DD-<ID>-<slug>/`, one per task (`./kit journal new|state|finish`). The newest
iteration file says whose move it is: `-FINAL` done, `-PART` an agent continues, `-READY` needs the human.
Captures, screenshots and dumps go in `context/`, never the journal (it is committed). The last section is
always `## Not verified / not 1:1` (`./kit done` checks).

## Wrap-up

`./kit done T7 "msg"` (or `--blocked "<why + what unblocks it>"`): tests → tick or block → commit everything →
push the branch. Run it, and any `./kit check --all`, as a background command and wait for it: under the
other slots' load the gate can run past an hour, past your shell's own time limit, and a gate cut off there
commits nothing. Never start a second one while the first still runs. Push details, unattended runs (`./kit run`) and merge conflicts: `docs/agent/TASKS.md`.

## When blocked

A tool hangs, the emulator won't cooperate, the same fix fails twice, something needs the human, or ~20 minutes
pass with nothing to show: never loop. Stash experiments, write what happened and what you tried in the journal,
`./kit done <ID> "<what is solid>" --blocked "<why + what unblocks it>"`, take the next open task, preferably in
another section. "The emulator is not running" is not a blocker: start it. A usage limit isn't either: commit
what is solid and wait. Never end a run while open tasks remain; at the end, list the blockers. Human-only
items go in `plan/HUMAN.md`, never into code as a guess.

## Code map

`docs/agent/CODEMAP.md`, one `## <area>` section per crate (`./kit plan game-sim`): read the one you need. Add
each new module there with one line.
