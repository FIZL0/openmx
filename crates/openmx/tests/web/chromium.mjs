// The page in a real browser: serve web/, open it in headless chromium, pick the disc the way a user does, wait for
// the frame, reload, press Ride on the menu the stored disc opens on (P181), and wait for it again without being asked
// for the disc (crates/openmx/tests/web.rs,
// `the_page_starts_in_a_browser`, which is ignored: it copies the whole disc into a scratch browser profile).
//
//   node chromium.mjs <web dir> <disc.iso> <scratch profile dir> [--no-opfs]
//
// Driven over the DevTools protocol with node's own WebSocket, so it needs nothing but chromium. `--no-opfs` serves a
// copy of the page with OPFS refused, which is the private-window path (worker.js reads the picked file itself).
// Prints one JSON line: { visits: [{ state, seconds, shot }...], usage }, `shot` the screenshot's sha-256 and size.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawn } from 'node:child_process';

const [webdir, iso, profile, flag] = process.argv.slice(2);
const noOpfs = flag === '--no-opfs';
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.svg': 'image/svg+xml',
  '.webmanifest': 'application/manifest+json', '.ttf': 'font/ttf' };
const server = http.createServer((req, res) => {
  let p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (p.endsWith('/')) p += 'index.html';
  const f = path.join(webdir, path.normalize(p));
  fs.readFile(f, (e, b) => {
    if (e) {
      res.writeHead(404);
      res.end();
      return;
    }
    if (noOpfs && p === '/index.html') b = Buffer.from(String(b).replace('opfs = true;', 'opfs = false;'));
    res.writeHead(200, { 'content-type': types[path.extname(f)] ?? 'application/octet-stream' });
    res.end(b);
  });
}).listen(0, '127.0.0.1');
await new Promise(r => server.on('listening', r));
const url = `http://127.0.0.1:${server.address().port}/`;

const chrome = spawn('chromium', ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
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
ws.addEventListener('message', ({ data }) => {
  const m = JSON.parse(data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  } else if (m.method === 'Runtime.exceptionThrown') console.error('page:', m.params.exceptionDetails.text);
});
const send = (method, params = {}) => new Promise(r => {
  const i = ++id;
  pending.set(i, r);
  ws.send(JSON.stringify({ id: i, method, params }));
});
const evaluate = async expression =>
  (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result?.result?.value;
const state = () => evaluate('document.body.dataset.state ?? ""');
const until = async (want, ms) => {
  const t = Date.now();
  for (;;) {
    const s = await state();
    if (want.includes(s) || Date.now() - t > ms) return s;
    await new Promise(r => setTimeout(r, 100));
  }
};
const shot = async () => {
  const png = Buffer.from((await send('Page.captureScreenshot', { format: 'png' })).result.data, 'base64');
  return { sha256: crypto.createHash('sha256').update(png).digest('hex'), bytes: png.length };
};

await send('Runtime.enable');
await send('Page.enable');
const visits = [];

// The first visit: nothing stored, so the page asks; the disc is picked, copied and drawn.
let t0 = Date.now();
await send('Page.navigate', { url });
if ((await until(['asking', 'failed'], 20000)) !== 'asking') fail(`the first visit did not ask for the disc: ${await state()}`);
const doc = await send('DOM.getDocument');
const pick = await send('DOM.querySelector', { nodeId: doc.result.root.nodeId, selector: '#pick' });
await send('DOM.setFileInputFiles', { files: [iso], nodeId: pick.result.nodeId });
visits.push({ state: await until(['started', 'failed'], 600000), seconds: (Date.now() - t0) / 1000, shot: await shot() });

// The second: the page should start on what the first one stored, without asking.
t0 = Date.now();
// (marked first, so the old document's `started` is never read as the new one's)
await evaluate("document.body.dataset.state = 'reloading'");
await send('Page.reload');
// A stored disc opens on the menu (P181), and its Ride starts the game; no menu means it asked or failed.
if ((await until(['menu', 'started', 'failed', 'asking'], 60000)) === 'menu') await evaluate("document.getElementById('ride').click()");
visits.push({ state: await until(['started', 'failed', 'asking'], 60000), seconds: (Date.now() - t0) / 1000, shot: await shot() });

const usage = await evaluate('navigator.storage.estimate().then(e => e.usage)');
console.log(JSON.stringify({ visits, usage }));
chrome.kill();
await new Promise(r => chrome.on('exit', r));
server.close();
process.exit(0);
