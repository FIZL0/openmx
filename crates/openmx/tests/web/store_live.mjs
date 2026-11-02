// The mod store in the page against huggingface.co itself (P218): store.mjs's run without its fake. web/ served on
// loopback, opened in headless chromium with `?software&args=--screen store` (no --mod-store: the store is
// FIZL0/openmx-mods), the disc picked; then Space (Cross) on the lit card until a mod is in OPFS `mods/` — the worker's
// fetches to the API, resolve and the CDN redirect, all cross-origin — then Triangle (Report), which opens the store's
// discussions/new in a new tab, then Square (Remove) until the mod is gone from OPFS. Needs the network, and a mod on
// the store; not in the gate (crates/openmx/tests/web.rs, `the_browser_uses_the_live_store`, is ignored).
//
//   node store_live.mjs <web dir> <pkg dir> <disc.iso> <scratch profile dir>
//
// Prints one JSON line: { state, files: [OPFS mods/ paths once installed], stamp, left: [OPFS store/ names], report:
// the URL the new tab opened, opened: the URLs the page asked window.open for, removed: [OPFS mods/ paths after Remove], hf: { asked: requests the page made to
// huggingface.co and its CDN, failed: those that failed (CORS among them) } }. With OPENMX_STORE_SHOT a path, the page's
// picture after Report is written there.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';

const [webdir, pkg, iso, profile] = process.argv.slice(2);

const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.svg': 'image/svg+xml',
  '.webmanifest': 'application/manifest+json', '.ttf': 'font/ttf' };
const server = http.createServer((req, res) => {
  const u = new URL(req.url, 'http://x');
  let p = decodeURIComponent(u.pathname);
  if (p.endsWith('/')) p += 'index.html';
  const f = p.startsWith('/pkg/') ? path.join(pkg, path.normalize(p.slice(5))) : path.join(webdir, path.normalize(p));
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
const origin = `http://127.0.0.1:${server.address().port}`;
const url = `${origin}/?software&args=${encodeURIComponent('--screen store')}`;

const chrome = spawn('chromium', ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', '--window-size=1280,720', 'about:blank'],
  { stdio: ['ignore', 'ignore', 'pipe'] });
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
const hf = /^https:\/\/([a-z0-9.-]+\.)?(huggingface\.co|hf\.co)\//;
const asked = new Map();
const failed = [];
let report = null;
ws.addEventListener('message', ({ data }) => {
  const m = JSON.parse(data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  } else if (m.method === 'Runtime.exceptionThrown') console.error('page:', m.params.exceptionDetails.text);
  else if (m.method === 'Target.attachedToTarget') {
    send('Runtime.enable', {}, m.params.sessionId);
    send('Network.enable', {}, m.params.sessionId);
  } else if (['Target.targetCreated', 'Target.targetInfoChanged'].includes(m.method) && m.params.targetInfo.type === 'page'
    && hf.test(m.params.targetInfo.url)) {
    // a new tab starts on about:blank, then names its page
    report = m.params.targetInfo.url;
  } else if (m.method === 'Runtime.consoleAPICalled') {
    console.error('console:', m.params.args.map(a => a.value ?? a.description).join(' '));
  } else if (m.method === 'Network.requestWillBeSent' && hf.test(m.params.request.url)) {
    asked.set(m.params.requestId, m.params.request.url);
  } else if (m.method === 'Network.loadingFailed' && asked.has(m.params.requestId)) {
    failed.push(`${asked.get(m.params.requestId)}: ${m.params.errorText} ${m.params.corsErrorStatus?.corsError ?? ''}`.trim());
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
const opfs = top => evaluate(`(async () => {
  const out = [];
  let stamp = null;
  const walk = async (dir, at) => {
    for await (const [name, h] of dir.entries()) {
      const p = at ? at + '/' + name : name;
      if (h.kind === 'directory') await walk(h, p);
      else {
        out.push(p);
        if (name === 'store.json') stamp = JSON.parse(await (await h.getFile()).text()).sha256 ?? null;
      }
    }
  };
  try {
    await walk(await (await navigator.storage.getDirectory()).getDirectoryHandle(${JSON.stringify(top)}), '');
  } catch {}
  return { files: out.sort(), stamp };
})()`);

await send('Runtime.enable');
await send('Page.enable');
await send('Network.enable');
await send('Page.addScriptToEvaluateOnNewDocument', { source: 'navigator.getGamepads = () => [];' });
// what the page asked window.open for, whether or not a popup blocker lets the tab open
await send('Page.addScriptToEvaluateOnNewDocument', { source: 'window.openmxOpened = []; { const o = window.open; window.open = (...a) => { window.openmxOpened.push(String(a[0])); return o.apply(window, a); }; }' });
await send('Target.setDiscoverTargets', { discover: true });
await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
await send('Page.navigate', { url });
let d = await until(d => ['asking', 'failed'].includes(d.state), 120000);
if (d.state !== 'asking') fail(`the page did not ask for the disc: ${JSON.stringify(d)}`);
const doc = await send('DOM.getDocument');
const pick = await send('DOM.querySelector', { nodeId: doc.result.root.nodeId, selector: '#pick' });
await send('DOM.setFileInputFiles', { files: [iso], nodeId: pick.result.nodeId });
d = await until(d => d.state === 'started' || d.state === 'failed', 120000);
if (d.state !== 'started') fail(`the page did not start: ${JSON.stringify(d)}`);

const press = async (key, code, vk) => {
  await send('Input.dispatchKeyEvent', { type: 'keyDown', key, code, windowsVirtualKeyCode: vk });
  await new Promise(r => setTimeout(r, 80));
  await send('Input.dispatchKeyEvent', { type: 'keyUp', key, code, windowsVirtualKeyCode: vk });
};
// Cross on the lit card (the listing's first) until a mod is in: a press before the listing is in lights nothing
let got = { files: [], stamp: null };
for (let n = 0, end = Date.now() + 300000; Date.now() < end && !got.stamp; n++) {
  if (n < 60) await press(' ', 'Space', 32);
  await new Promise(r => setTimeout(r, 3000));
  got = await opfs('mods');
}
let left = [];
for (const end = Date.now() + 10000; Date.now() < end; ) {
  left = (await opfs('store')).files;
  if (!left.length) break;
  await new Promise(r => setTimeout(r, 200));
}
// Triangle (G under the default keys, `keys = modern`): Report, a new tab on the store's discussions/new
let opened = [];
for (const end = Date.now() + 15000; Date.now() < end && !report; ) {
  if (opened.length) {
    await new Promise(r => setTimeout(r, 1000));
    continue;
  }
  await press('g', 'KeyG', 71);
  await new Promise(r => setTimeout(r, 2000));
  opened = (await evaluate('window.openmxOpened')) ?? [];
}
if (process.env.OPENMX_STORE_SHOT)
  fs.writeFileSync(process.env.OPENMX_STORE_SHOT, Buffer.from((await send('Page.captureScreenshot', { format: 'png' })).result.data, 'base64'));
// Square (V): Remove
let removed = got.files;
for (const end = Date.now() + 30000; Date.now() < end && removed.some(f => f.endsWith('store.json')); ) {
  await press('v', 'KeyV', 86);
  await new Promise(r => setTimeout(r, 2000));
  removed = (await opfs('mods')).files;
}
d = await data();
console.log(JSON.stringify({ state: d.state, files: got.files, stamp: got.stamp, left, report, opened, removed,
  hf: { asked: asked.size, failed } }));
chrome.kill();
await new Promise(r => chrome.on('exit', r));
server.close();
process.exit(0);
