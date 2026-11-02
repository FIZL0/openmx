# F14 — splash update notice

2026-11-02. Task: `./kit plan F14`.

opentennis's B581 (the splash's New version card), 651af09b (the page checks version.txt 5 s after load and every
minute) and B660 (the build small in a corner of the splash and of the site's header), ported to openMX. The press-start
splash now shows a New version card in its top right with Square's word (Update / Download on the desktop, Refresh in
the browser) and the build (a release's tag, else the commit) in its top left; the site's hero has the same label in
its corner and its banner says "New version … Refresh". Checked on the desktop (`OPENMX_LATEST`) and in headless
chromium (a changed version.txt). Details in 1-splash-update-notice-FINAL.md.
