// The browser's online peer as a user gets it (crates/openmx/tests/netplay_browser.rs): web/ served on loopback, the
// page opened in headless chromium with `?join=<ticket>` — index.html, worker.js, the module and its glue, the page's
// own frame loop — and read over the DevTools protocol (node's own WebSocket, as chromium.mjs drives it).
//
//   node page.mjs <web dir> <pkg dir> <relay url> <ticket> <ticks>
//
// `/pkg/` is served from <pkg dir> (tools/web.sh --check's build), the rest from <web dir>. The chromium profile is
// a scratch directory, removed at the end. Prints what peer.mjs prints: `state <JSON>` a few times a second (the
// page's `window.openmxOnline`), and once the page has them, `tick <line>` for each of `window.openmxTicks`' lines;
// before the page has a state, `page <step>` as chromium starts and the page opens (each is progress to the test).
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
// No extensions (K7): this machine's chromium brings its own into every profile, the policy's force-installed one
// fetched from the web store a few seconds in, and one arriving mid-load aborted the worker's module fetch (the page
// then waits on a ride that never starts); nor its background calls to Google (GCM sign-in, component updates).
const chrome = spawn('chromium', ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', '--disable-extensions',
  '--disable-component-extensions-with-background-pages', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
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
  const late = setTimeout(() => fail(`chromium gave no DevTools address:\n${buf}`), 90000); // 20 s ran out starting chromium under a busy gate
  chrome.stderr.on('data', d => {
    buf += d;
    const m = buf.match(/DevTools listening on (ws:\S+)/);
    if (m) {
      clearTimeout(late);
      resolve(m[1]);
    }
  });
});
process.stdout.write('page chromium\n');
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
// A reply that never comes (K7 diagnostics): said on stderr, and the loop asks again.
const evaluate = async expression => {
  let late;
  const r = await Promise.race([send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }),
    new Promise(go => (late = setTimeout(() => go(null), 10000)))]);
  clearTimeout(late);
  if (!r) console.error('page: no answer to Runtime.evaluate in 10 s');
  return r?.result?.result?.value;
};

await send('Runtime.enable');
await send('Page.enable');
await send('Page.navigate', { url });
process.stdout.write('page opened\n');
let sent = false;
let quiet = 0;
for (;;) {
  await new Promise(r => setTimeout(r, 250));
  const [state, lines, page] = (await evaluate('[JSON.stringify(window.openmxOnline ?? null), window.openmxTicks ?? "", ' +
    'JSON.stringify([document.readyState, document.body?.dataset.state, document.querySelector("#status")?.textContent])]')) ?? [];
  // no state yet: what the page is at, every five seconds, on stderr (the test's log, not progress)
  if ((!state || state === 'null') && ++quiet % 20 === 0) console.error(`page: no state yet, at ${page}`);
  if (!sent && lines) {
    process.stdout.write(lines.split('\n').filter(Boolean).map(l => `tick ${l}\n`).join(''));
    sent = true;
  }
  if (state && state !== 'null') process.stdout.write(`state ${state}\n`);
}
