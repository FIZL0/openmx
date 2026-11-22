# K86 — web lobby ride options

2026-11-21. Task: `./kit plan K86`.

The browser build broke on lobby_ride.rs calling Net::next, which wasm lacks. B49 had already fixed it with a
native-only `next` and a wasm stand-in. The person picked A (keep that fix and close); `tools/web.sh --check` is
green on main (324941bbd), so no code changed.
