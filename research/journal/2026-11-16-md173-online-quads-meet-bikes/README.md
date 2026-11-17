# MD173 — online quads meet bikes

2026-11-16. Task: `./kit plan MD173`.

Online, a seat's quad was in no contact list, so bikes rode through it. First pass (1) found the
quad had nothing to meet with even offline and blocked on MD179. After MD179, net/rides.rs puts the
quad's hull, pushed body and mirrors in the online pass as split screen does (2, FINAL), and a
hash-only field that broke state round trips is cleared.
