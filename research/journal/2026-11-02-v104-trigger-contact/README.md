# V104 — trigger contact

2026-11-02. Task: `./kit plan V104`.

The toy trigger's contact callback, its actions' counter/delay/UponExit rules and its update (touch bits,
exit test by the actor's box, contact list, timers) are ported in `creature.rs` and proved against the
original on the boot ELF. Path cars turned out to strike no trigger on the retail disc. IgnoreTrigger,
EnterTest, ExitTest and ActionTest are read and do nothing, as they do nothing in the original.
Details are in the numbered file.
