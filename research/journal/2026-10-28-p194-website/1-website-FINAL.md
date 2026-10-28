# 1 — website (FINAL)

Task P194. 2026-10-28.

## What I did

Took opentennis's M96/M98/M107 shape (its web/index.html head, robots.txt, sitemap.xml, the `shots/` copy in
tools/web.sh, its HUMAN.md search-engine item, its footer) and fitted it to openMX's existing P181 page, whose look
and ISO-picker/menu-stage flow stay as they were.

- **Screenshots** (`docs/screenshots/`, ten 1600x900 progressive JPEGs, 73–157 KB each). All from the release shell,
  `--headless --size 1920x1080 --mods ~/repos/openmx/mods` with `DISPLAY=:0`, then
  `magick -resize 1600x900 -strip -sampling-factor 4:2:0 -interlace JPEG -quality 80`:
  - `ride-free06.jpg` (the hero and the link-preview image): `--play context/recordings/recording2.p2m2 --overlay none`, frame 265.
    It crosses the railway on Dallas (free06). recording2 rides free06 in the shell, not free09.
  - `air-cancan.jpg`: the same tape with the HUD, frame 298. A Can Can with its score on the HUD.
  - `wreck.jpg`: the same tape, frame 390. The rider is thrown towards a bush.
  - `biplane.jpg`: `--track free06 --vehicle biplane --fly-at 2500,2500,90,120`, frame 60.
  - `rain-nat01.jpg`: `--track nat01 --trace-at 2564.8,3016.85,180`, frame 400 (memory nat01-rain-shot).
  - `road-free09.jpg`: `--track free09 --trace-at 1872,4500,180 --overlay none --shot-pose 1600,260,4800,150,-12,85`,
    frame 30. A raised view over Bridgeport. The bike-height view down the road (memory free09-road-shot-spots) was
    too soft to use.
  - `split-4p.jpg`: `--players 4 --play recording2.p2m2`, frame 300. The tape drives 1P and the other three stand.
  - `mxvatv-nat04.jpg`: `--track mxvatv_nat04` (the user's imported MX vs. ATV track mod), frame 60.
  - `menu-main.jpg`, `menu-online.jpg`: `--menu --screen main|online --frames 2`.
  I swept each tape over 20 frames, then over 6 frames near the best ones, and picked from contact sheets. The
  sweep script is in the session scratchpad, not the repo.
- **web/index.html**:
  - The head: title and meta description, canonical https://openmx.dev/, Open Graph and Twitter card on
    `shots/ride-free06.jpg`, JSON-LD `VideoGame` with `isBasedOn` MX Unleashed (Rainbow Studios/THQ, 2004,
    Wikipedia/MobyGames/PCSX2-wiki `sameAs`), and a head script that adds `robots noindex` on `beta.*`. The canonical
    is static, so the beta's points at openmx.dev too.
  - The hero now has text beside the picture (stacked on a phone), "Play in your browser" and "Desktop downloads"
    pills, and the kicker "a remaster of MX Unleashed".
  - New sections: a lazy-loaded Screenshots gallery (`loading="lazy"`, width/height set so nothing jumps), "What is
    in it" (browser/PC/Steam Deck, online, the original's ride, split screen, mods, MX vs. ATV Unleashed tracks),
    Questions (`<details>`: do I need the game — yes, your own disc; PC; same game; decompilation; MX vs. ATV
    remastered?; online; cost), and Credits & links (GitHub, releases, issues, Wikipedia for both originals,
    MobyGames, PCSX2 wiki).
  - The footer is opentennis's: a nav and the not-affiliated line. The build stamp moved into the footer line.
  - The band's hints are now Pick your disc / Screenshots / Questions. Files is still in the footer nav.
  - The originals are named only as what this remasters, and the name stays openMX.
- **web/robots.txt and web/sitemap.xml**: one URL, with the ten pictures as `image:image`.
- **tools/web.sh** copies `docs/screenshots/*.jpg` into `web/shots/`, which is git-ignored (web/.gitignore). pages.yml and
  release.sh tar web/ after web.sh, so both sites get them.
- **web/sw.js** no longer precaches `pkg/openmx_web.js` and `openmx_web_bg.wasm` at install. That precache fetched the
  game module on a visitor's first page load. Now only Ride loads it (worker.js `game()` already imported it lazily),
  and the fetch handler still keeps it for offline play once a ride has loaded it.
- **plan/HUMAN.md** Releases item 6: Search Console, Bing, a link-preview check, the repo homepage and topics.
- **Filed**:
  - P195: switch the repo homepage to openmx.dev after the first release. The site is not live: openmx.dev gives 404,
    the only release is the `beta` pre-release, and the homepage is https://beta.openmx.dev/ today. The task said
    "once a release has deployed it", so I did not run `gh repo edit`. P195 may be renumbered on merge, since main
    has a release task of its own around that number.
  - P196: a `--shot` of an `--online` ride is one flat fog colour.

## What the original does

Not applicable: front end and website only (AGENT.md: front-end is ours to design).

## How it was proved

- `./kit check -- -p openmx --test web`: 3 passed. `the_launcher_fits_a_phone_and_a_desktop` checks the landing
  page and the menu stage at 1440 and 390 px, with no sideways scroll, the font loaded and Ride lit.
- `the_page_starts_in_a_browser` (ignored, run by hand with `--ignored`) passed. It runs tools/web.sh (and with it
  the new shots copy: web/shots got the ten files), picks the real disc into OPFS in headless chromium, waits for
  the frame, reloads, presses Ride on the menu, and also takes the no-OPFS path. So the ISO picker and ride flow
  still work after the page and sw.js changes.
- I looked at full-page chromium captures of both states at both widths (scratchpad): hero picture beside the text
  on desktop and under it on a phone. Gallery pictures below the fold were not fetched in the capture, which shows
  lazy loading works.
- The JSON-LD parses as JSON (python `json.loads`), and the sitemap parses as XML.

## Not verified / not 1:1

- **No online shot.** Four headless `--online` shells over loopback rode fine (282 ticks, 0 desyncs), but the
  host's `--shot` came out one flat fog colour, at frame 100 and at frame 280. The gallery shows online play with
  the Online menu screen and 4-player split screen instead. Filed as P196.
- **The PCSX2 wiki and MobyGames links were not opened.** Both sites answer this machine's curl with 403. The
  MobyGames IDs come from Wikidata: MX Unleashed has two IDs (12609 and 24466), and I linked 12609 without knowing
  which platform's page it is. The PCSX2 page name `MX_Unleashed` follows the wiki's title pattern, as opentennis's
  `Hot_Shots_Tennis` does. Both Wikipedia pages returned 200.
- **Not checked by real search engines or link previewers.** The site isn't on openmx.dev yet, so Search
  Console, Bing and the OG preview are the human's item 6, after the first release.
- **The beta's noindex comes from a script.** It is added by script on `beta.*`, as opentennis does; a crawler that
  doesn't run scripts won't see it. The static canonical link to openmx.dev covers that case.
- **The Steam Deck claim is from the build, not from running it.** The page says the Linux build runs on the
  Steam Deck because tools/build.sh builds it in the Steam Runtime container. I didn't run it on a Deck.
- **Speed not measured.** I did not run Lighthouse for "fast first load". What I did: the hero image is preloaded,
  the gallery is lazy, and the wasm loads only on Ride.
- **The pictures show the disc's art.** They include the original's textures, models and the MX Unleashed logo in
  the menu shots, drawn by our renderer. The task asked for promo shots in docs/screenshots/, as opentennis's M98
  did, so they are committed despite AGENT.md's general "no screenshots" rule.
- **README not updated.** It does not link the new screenshots: the task named web/ and docs/screenshots/ only.

## Next

Nothing in this task. After the first release: plan/HUMAN.md Releases item 6, and P195.
