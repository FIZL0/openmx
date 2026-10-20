// The browser's online peer as a user gets it (crates/openmx/tests/netplay_browser.rs): web/ served on loopback, the
// page opened in headless chromium with `?join=<ticket>` — index.html, worker.js, the module and its glue, the page's
// own frame loop — and read over the DevTools protocol (node's own WebSocket, as chromium.mjs drives it).
//
//   node page.mjs <web dir> <pkg dir> <relay url> <ticket> <ticks>
//
// `/pkg/` is served from <pkg dir> (tools/web.sh --check's build), the rest from <web dir>. The chromium profile is
// a scratch directory, removed at the end. Prints what peer.mjs prints: `state <JSON>` a few times a second (the
// page's `window.openmxOnline`), and once the page has them, `tick <line>` for each of `window.openmxTicks`' lines.
// Runs until stdin closes.
import http from 'node:http';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';

const [webdir, pkg, relay, ticket, ticks] = process.argv.slice(2);
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.svg': 'image/svg+xml',
  '.webmanifest': 'application/manifest+json' };
const server = http.createServer((req, res) => {
  let p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (p.endsWith('/')) p += 'index.html';
  p = path.normalize(p);
  const f = p.startsWith('/pkg/') ? path.join(pkg, p.slice(5)) : path.join(webdir, p);
  fs.readFile(f, (e, b) => {
    if (e) {
      res.writeHead(404);
      res.end();
      return;
    }
    res.writeHead(200, { 'content-type': types[path.extname(f)] ?? 'application/octet-stream' });
    res.end(b);
  });
}).listen(0, '127.0.0.1');
await new Promise(r => server.on('listening', r));
const ask = new URLSearchParams({ join: ticket, relay, codes: '', ticks, name: 'Browser' });
const url = `http://127.0.0.1:${server.address().port}/?${ask}`;

const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'openmx-page-'));
const chrome = spawn('chromium', ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
const quit = code => {
  const done = () => {
    fs.rmSync(profile, { recursive: true, force: true });
    process.exit(code);
  };
  if (chrome.exitCode !== null || chrome.signalCode !== null) done();
  chrome.on('exit', done);
  chrome.kill();
};
const fail = why => {
  console.error(why);
  quit(1);
};
process.stdin.on('end', () => quit(0));
process.stdin.resume();
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

await send('Runtime.enable');
await send('Page.enable');
await send('Page.navigate', { url });
let sent = false;
for (;;) {
  await new Promise(r => setTimeout(r, 250));
  const [state, lines] = (await evaluate('[JSON.stringify(window.openmxOnline ?? null), window.openmxTicks ?? ""]')) ?? [];
  if (!sent && lines) {
    process.stdout.write(lines.split('\n').filter(Boolean).map(l => `tick ${l}\n`).join(''));
    sent = true;
  }
  if (state && state !== 'null') process.stdout.write(`state ${state}\n`);
}
