// The in-game update notice (P193; crates/openmx/tests/web.rs, `a_new_build_shows_on_the_menu_not_the_ride`): web/
// served on loopback with a `version.txt` this script changes, opened in chromium with OPFS refused (the picked disc
// read in place, as gpu.mjs) on the software renderer, so it needs no GPU.
//
//   node update.mjs <web dir> <disc.iso> <scratch profile dir>
//
// Two visits. The ride: `?args=--ride`, and once it is on the canvas the site's version.txt changes and the page reads
// it again (`openmxRecheck`); the page's own banner must stay down and the game must offer nothing over the ride. The
// menu: the page loads again with a different build in local storage (`openmx.build`) as the last one started, the site
// changes again once the game is up, and Start (Enter) is pressed past the splash to the main menu, which must offer
// the newer build and say "Updated to …". What the game's main menu shows it also says on the console (menu.rs's
// `builds`), which is what is read here. With OPENMX_GPU_PKG set to a directory, pkg/ is served from there; with
// OPENMX_UPDATE_SHOT set to a file, the main menu's screenshot is written there, and with OPENMX_SPLASH_SHOT the
// splash's, which must show the newer build before Start is pressed (F14).
//
// Prints one JSON line: { ride: { state, banner, said: [...] }, menu: { state, banner, said: [...], stored } } —
// `banner` whether the page's own Reload banner is up, `said` the game's console lines about builds, `stored` what local
// storage holds as the last build started afterwards, `reloaded` the footer's build after Square reloaded the page.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';

const [webdir, iso, profile] = process.argv.slice(2);
const PKG = process.env.OPENMX_GPU_PKG;
const OLD = 'v0.0.1 1111111';
const NEW = 'v0.0.2 2222222';
let version = OLD;
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.svg': 'image/svg+xml',
  '.webmanifest': 'application/manifest+json', '.ttf': 'font/ttf' };
const server = http.createServer((req, res) => {
  let p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (p.endsWith('/')) p += 'index.html';
  if (p === '/version.txt') {
    res.writeHead(200, { 'content-type': 'text/plain', 'cache-control': 'no-store' });
    res.end(`${version}\n`);
    return;
  }
  // no service worker: its cache would answer the second visit
  if (p === '/sw.js') {
    res.writeHead(404);
    res.end();
    return;
  }
  const f = PKG && p.startsWith('/pkg/') ? path.join(PKG, path.normalize(p.slice(5))) : path.join(webdir, path.normalize(p));
  fs.readFile(f, (e, b) => {
    if (e) {
      res.writeHead(404);
      res.end();
      return;
    }
    if (p === '/index.html') b = Buffer.from(String(b).replace('opfs = true;', 'opfs = false;'));
    res.writeHead(200, { 'content-type': types[path.extname(f)] ?? 'application/octet-stream' });
    res.end(b);
  });
}).listen(0, '127.0.0.1');
await new Promise(r => server.on('listening', r));
const base = `http://127.0.0.1:${server.address().port}/`;

const chrome = spawn('chromium', ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', '--window-size=960,672', 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
const fail = why => {
  console.error(why);
  chrome.kill();
  process.exit(1);
};
const wsUrl = await new Promise(resolve => {
  let buf = '';
  const late = setTimeout(() => fail(`chromium gave no DevTools address:\n${buf}`), 20000);
  chrome.stderr.on('data', d => {
    buf += d;
    const m = buf.match(/DevTools listening on (ws:\S+)/);
    if (m) {
      clearTimeout(late);
      resolve(m[1]);
    }
  });
});
const targets = await (await fetch(wsUrl.replace('ws://', 'http://').replace(/\/devtools.*/, '/json'))).json();
const ws = new WebSocket(targets.find(t => t.type === 'page').webSocketDebuggerUrl);
await new Promise(r => ws.addEventListener('open', r));
let id = 0;
const pending = new Map();
// the game's console lines about builds, since the visit began
let said = [];
// F14: the splash's console lines about the newer build
let splash = [];
ws.addEventListener('message', ({ data }) => {
  const m = JSON.parse(data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  } else if (m.method === 'Runtime.exceptionThrown') console.error('page:', m.params.exceptionDetails.text);
  else if (m.method === 'Target.attachedToTarget') send('Runtime.enable', {}, m.params.sessionId);
  else if (m.method === 'Runtime.consoleAPICalled') {
    const line = m.params.args.map(a => a.value ?? a.description).join(' ');
    console.error('console:', line);
    if (/the main menu (offers|says)/.test(line)) said.push(line);
    if (/the splash offers/.test(line)) splash.push(line);
  }
});
const send = (method, params = {}, sessionId) => new Promise(r => {
  const i = ++id;
  pending.set(i, r);
  ws.send(JSON.stringify({ id: i, method, params, ...(sessionId ? { sessionId } : {}) }));
});
const evaluate = async expression =>
  (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result?.result?.value;
const data = async () => (await evaluate('({ ...document.body.dataset })')) ?? {};
const until = async (ok, ms) => {
  const t = Date.now();
  for (;;) {
    const d = await data();
    if (ok(d) || Date.now() - t > ms) return d;
    await new Promise(r => setTimeout(r, 100));
  }
};
const rest = ms => new Promise(r => setTimeout(r, ms));
const banner = () => evaluate("document.getElementById('fresh').classList.contains('up')");

await send('Runtime.enable');
await send('Page.enable');
// No pad of the machine's reaches the page: another slot's `./kit pad` (or a pad in a hand) pressing a button shows
// itself to every chromium here, and the page then hides the touch pad and reads its presses as the rider's (K13).
await send('Page.addScriptToEvaluateOnNewDocument', { source: 'navigator.getGamepads = () => [];' });
await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });

// One visit at `query` with `last` stored as the build last started: picks the disc, waits for the game, then moves the
// site on and has the page read it again.
const visit = async (query, last) => {
  version = OLD;
  await send('Page.navigate', { url: `${base}?${query}` });
  await until(d => d.state !== undefined, 20000);
  await evaluate(last ? `localStorage.setItem('openmx.build', '${last}')` : "localStorage.removeItem('openmx.build')");
  // the stored build is read as the page loads: load it again with it in place
  await evaluate("document.body.dataset.state = 'reloading'");
  await send('Page.reload');
  let d = await until(d => ['asking', 'failed', 'started'].includes(d.state), 60000);
  if (d.state === 'asking') {
    const doc = await send('DOM.getDocument');
    const pick = await send('DOM.querySelector', { nodeId: doc.result.root.nodeId, selector: '#pick' });
    await send('DOM.setFileInputFiles', { files: [iso], nodeId: pick.result.nodeId });
    d = await until(d => ['failed', 'started'].includes(d.state), 120000);
  }
  if (d.state !== 'started') fail(`the game did not start (${query}): ${JSON.stringify(d)}`);
  said = [];
  splash = [];
  version = NEW;
  await evaluate('openmxRecheck()');
  return d;
};
// A key held down until `done()` says the game took it, then let go (K31): the menus read presses as edges, so a held
// key is one press however slow the frames come, where a tap pressed again on a clock can land a screen too far (an
// Enter that the main menu takes as Free Ride, after which Square does nothing). False if `ms` passed first.
const hold = async (code, key, vk, done, ms) => {
  await send('Input.dispatchKeyEvent', { type: 'keyDown', code, key, windowsVirtualKeyCode: vk });
  let ok = false;
  for (const t = Date.now(); !(ok = await done()) && Date.now() - t < ms; ) await rest(100);
  await send('Input.dispatchKeyEvent', { type: 'keyUp', code, key, windowsVirtualKeyCode: vk });
  return ok;
};

// The ride: nothing over it, from the page or the game.
let d = await visit('args=--ride&software', '');
await rest(4000);
const ride = { state: (await data()).state, banner: await banner(), said: [...said] };

// The menu: past the splash to the main menu, where the newer build shows, and the first start of this build says so.
d = await visit('software', 'old0000');
// F14: the splash shows the newer build before anyone presses Start
for (let t = Date.now(); Date.now() - t < 20000 && !splash.length; ) await rest(200);
if (process.env.OPENMX_SPLASH_SHOT) {
  await rest(1000);
  fs.writeFileSync(process.env.OPENMX_SPLASH_SHOT, Buffer.from((await send('Page.captureScreenshot', { format: 'png' })).result.data, 'base64'));
}
const splashSaid = [...splash];
// Start (Enter) held past the splash until the main menu has said both its lines
await hold('Enter', 'Enter', 13, async () => said.length >= 2, 60000);
if (process.env.OPENMX_UPDATE_SHOT) {
  await rest(500);
  fs.writeFileSync(process.env.OPENMX_UPDATE_SHOT, Buffer.from((await send('Page.captureScreenshot', { format: 'png' })).result.data, 'base64'));
}
const menu = { state: (await data()).state, banner: await banner(), said: [...said], splash: splashSaid, stored: await evaluate("localStorage.getItem('openmx.build')") };
// Square (V on the default keyboard, the modern layout's delete) on the main menu reloads the page into the newer build: a new document, whose footer names it.
// Held until a document without the old one's mark has its footer: the mark and the footer read in one evaluate, whose
// answer is null in the old document and nothing at all when it fails (the context going with the reload, or an answer
// lost with it), so a failed read is never taken for the new document (K31).
await evaluate('window.openmxBefore = 1');
let reloaded = '';
const fresh = async () => {
  const v = await Promise.race([
    evaluate("window.openmxBefore === undefined ? (document.getElementById('build')?.textContent ?? '') : null"),
    rest(2000),
  ]);
  if (typeof v === 'string' && v) reloaded = v;
  return !!reloaded;
};
await hold('KeyV', 'v', 86, fresh, 60000);
menu.reloaded = reloaded;

console.log(JSON.stringify({ ride, menu }));
chrome.kill();
await new Promise(r => chrome.on('exit', r));
server.close();
process.exit(0);
