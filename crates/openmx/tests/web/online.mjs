// Online from the game's own menu, in the browser (P204; crates/openmx/tests/netplay_browser.rs,
// `two_pages_host_and_join_from_the_online_menu`): web/ served on loopback, opened twice in one headless chromium,
// each page in a browser context of its own (its own OPFS, as two people's browsers are), the disc picked in both, and
// the shell's own front end on each canvas from `--screen online`. Nothing but keys: the host page's Down and Space on
// Host, the guest's on Join by Code and the on-screen keyboard's keys of the host's room code, Return, then Ready in
// the guest's lobby and Start in the host's. Both then ride with gas held, through the test's relay.
//
//   node online.mjs <web dir> <pkg dir> <disc.iso> <relay url> <scratch dir> [chromium flag...]
//
// Each page's `online.txt` (OPFS `files/`, written before the disc is picked) names the test's relay and this server's
// `/pkarr` as its code server, which keeps codes in memory as a pkarr relay does (no signature check). Chromium
// resolves no name but 127.0.0.1, so relay.pkarr.org, the shell's other code server, is never reached: nothing leaves
// the machine. The pages' consoles (the shell's `lobby: …` and `online: settled …` lines, P204) are what the driver
// reads, and are echoed to stderr as `host: …` and `guest: …`. Ends once both pages have said the hash before tick
// [TICKS], and prints one JSON line: { code, host: [lines], guest: [lines] } — each page's `online: settled` lines.
// Exits 1 (with why on stderr) if that is not so in ten minutes, or on two minutes with nothing new.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';

const [webdir, pkg, iso, relay, scratch, ...flags] = process.argv.slice(2);
const TICKS = 600;

const codes = new Map();
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.svg': 'image/svg+xml',
  '.webmanifest': 'application/manifest+json', '.ttf': 'font/ttf' };
const server = http.createServer((req, res) => {
  const u = new URL(req.url, 'http://x');
  if (u.pathname.startsWith('/pkarr/')) {
    const key = u.pathname.slice('/pkarr/'.length);
    const body = [];
    req.on('data', d => body.push(d));
    req.on('end', () => {
      if (req.method === 'PUT') {
        codes.set(key, Buffer.concat(body));
        res.writeHead(200);
        res.end();
      } else {
        const b = codes.get(key);
        res.writeHead(b ? 200 : 404, { 'content-type': 'application/octet-stream' });
        res.end(b ?? '');
      }
    });
    return;
  }
  let p = decodeURIComponent(u.pathname);
  if (p.endsWith('/')) p += 'index.html';
  const f = p.startsWith('/pkg/') ? path.join(pkg, path.normalize(p.slice(5))) : path.join(webdir, path.normalize(p));
  fs.readFile(f, (e, b) => {
    if (e) {
      res.writeHead(404);
      res.end();
      return;
    }
    // the picked disc read where it is, as gpu.mjs's: not a 2 GB copy into each context's OPFS
    if (p === '/index.html') b = Buffer.from(String(b).replace('opfs = true;', 'opfs = false;'));
    res.writeHead(200, { 'content-type': types[path.extname(f)] ?? 'application/octet-stream' });
    res.end(b);
  });
}).listen(0, '127.0.0.1');
await new Promise(r => server.on('listening', r));
const origin = `http://127.0.0.1:${server.address().port}`;
const url = `${origin}/?args=${encodeURIComponent('--screen online')}`;

// No extensions and no calls home, as page.mjs (K7, K12); no name resolves but loopback's; and two windows, neither in
// front, both drawing and keeping time.
const profile = fs.mkdtempSync(path.join(scratch, 'profile-'));
const chrome = spawn('chromium', ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', '--disable-extensions', '--disable-component-extensions-with-background-pages',
  '--disable-background-networking', '--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1',
  '--disable-background-timer-throttling', '--disable-renderer-backgrounding', '--disable-backgrounding-occluded-windows',
  ...flags, 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
const quit = code => {
  const done = () => {
    server.close();
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
process.stdin.on('end', () => fail('stdin closed'));
process.stdin.resume();
const wsUrl = await new Promise(resolve => {
  let buf = '';
  const late = setTimeout(() => fail(`chromium gave no DevTools address:\n${buf}`), 90000);
  chrome.stderr.on('data', d => {
    buf += d;
    const m = buf.match(/DevTools listening on (ws:\S+)/);
    if (m) {
      clearTimeout(late);
      resolve(m[1]);
    }
  });
});
const ws = new WebSocket(wsUrl);
await new Promise(r => ws.addEventListener('open', r));

// What each page's console said, by page; and when anything last moved on.
const pages = { host: { lines: [] }, guest: { lines: [] } };
const owner = new Map();
let progress = Date.now();
const started = Date.now();
let id = 0;
const pending = new Map();
ws.addEventListener('message', ({ data }) => {
  const m = JSON.parse(data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
    return;
  }
  const who = owner.get(m.sessionId);
  if (m.method === 'Target.attachedToTarget' && who) {
    // the page's worker, where the shell runs: its console too
    owner.set(m.params.sessionId, who);
    send('Runtime.enable', {}, m.params.sessionId);
  } else if (m.method === 'Runtime.consoleAPICalled' && who) {
    const line = m.params.args.map(a => a.value ?? a.description).join(' ');
    pages[who].lines.push(line);
    if (/^(lobby|online|renderer|openmx)/.test(line)) {
      progress = Date.now();
      console.error(`${who} at ${((Date.now() - started) / 1000).toFixed(1)} s: ${line}`);
    }
  } else if (m.method === 'Runtime.exceptionThrown' && who) console.error(`${who}: ${m.params.exceptionDetails.text}`);
});
const send = (method, params = {}, sessionId) => new Promise(r => {
  const i = ++id;
  pending.set(i, r);
  ws.send(JSON.stringify({ id: i, method, params, ...(sessionId ? { sessionId } : {}) }));
});
const wait = ms => new Promise(r => setTimeout(r, ms));

// A page in a context of its own, attached flat, its worker's console in reach.
async function open(who) {
  const ctx = (await send('Target.createBrowserContext', { disposeOnDetach: true })).result.browserContextId;
  const target = (await send('Target.createTarget', { url: 'about:blank', browserContextId: ctx, width: 960, height: 540 })).result.targetId;
  const session = (await send('Target.attachToTarget', { targetId: target, flatten: true })).result.sessionId;
  owner.set(session, who);
  const s = (method, params) => send(method, params, session);
  await s('Runtime.enable');
  await s('Page.enable');
  // keys reach it with the other window in front
  await s('Emulation.setFocusEmulationEnabled', { enabled: true });
  // no pad of the machine's (K13)
  await s('Page.addScriptToEvaluateOnNewDocument', { source: 'navigator.getGamepads = () => [];' });
  await s('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
  await s('Page.navigate', { url });
  const evaluate = async expression => (await s('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result?.result?.value;
  const page = { who, s, evaluate };
  pages[who] = Object.assign(pages[who], page);
  return pages[who];
}

async function pick(page, name) {
  for (const end = Date.now() + 120000; ; ) {
    const st = await page.evaluate('document.body?.dataset.state');
    if (st === 'asking') break;
    if (Date.now() > end) fail(`${page.who}: the page did not ask for the disc (${st})`);
    await wait(200);
  }
  // online.txt before the worker reads its files (it does once the disc is picked)
  const text = `name = ${name}\nrelay = own\nrelay_url = ${relay}\nprivate = no\ncode_server = ${origin}/pkarr\n`;
  const wrote = await page.evaluate(`(async () => {
    const dir = await (await navigator.storage.getDirectory()).getDirectoryHandle('files', { create: true });
    const w = await (await dir.getFileHandle('online.txt', { create: true })).createWritable();
    await w.write(${JSON.stringify(text)});
    await w.close();
    return true;
  })()`);
  if (!wrote) fail(`${page.who}: could not write online.txt`);
  const doc = await page.s('DOM.getDocument');
  const input = await page.s('DOM.querySelector', { nodeId: doc.result.root.nodeId, selector: '#pick' });
  await page.s('DOM.setFileInputFiles', { files: [iso], nodeId: input.result.nodeId });
}

const KEYS = {
  Down: { key: 'ArrowDown', code: 'ArrowDown', windowsVirtualKeyCode: 40 },
  Up: { key: 'ArrowUp', code: 'ArrowUp', windowsVirtualKeyCode: 38 },
  Left: { key: 'ArrowLeft', code: 'ArrowLeft', windowsVirtualKeyCode: 37 },
  Right: { key: 'ArrowRight', code: 'ArrowRight', windowsVirtualKeyCode: 39 },
  // Cross (keyboard-cross-is-space) and Start
  Space: { key: ' ', code: 'Space', windowsVirtualKeyCode: 32 },
  Return: { key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 },
  W: { key: 'w', code: 'KeyW', windowsVirtualKeyCode: 87 },
};
// A tap, held for a few frames and rested after (menu-taps-need-a-rest-since-p109).
async function tap(page, name, n = 1) {
  for (let i = 0; i < n; i++) {
    await page.s('Input.dispatchKeyEvent', { type: 'rawKeyDown', ...KEYS[name] });
    await wait(120);
    await page.s('Input.dispatchKeyEvent', { type: 'keyUp', ...KEYS[name] });
    await wait(300);
  }
}

// The first console line of `page`'s after `from` that `test` takes, or a failure.
async function heard(page, test, from = 0, why = '') {
  for (;;) {
    const i = page.lines.findIndex((l, k) => k >= from && test(l));
    if (i >= 0) return [page.lines[i], i];
    const failed = page.lines.find((l, k) => k >= from && /^lobby: failed/.test(l));
    if (failed) fail(`${page.who}: ${failed}`);
    if (Date.now() - progress > 120000) fail(`${page.who}: nothing new for two minutes, waiting for ${why}`);
    if (Date.now() - started > 600000) fail(`${page.who}: not done in ten minutes, waiting for ${why}`);
    await wait(100);
  }
}

// The menu up: the shell's front end has started and drawn.
async function menu(page) {
  for (;;) {
    const d = (await page.evaluate('({ ...document.body.dataset })')) ?? {};
    if (d.state === 'failed') fail(`${page.who}: the page failed: ${JSON.stringify(d)}`);
    if (d.state === 'started' && d.drawn) return;
    if (Date.now() - started > 600000) fail(`${page.who}: no menu in ten minutes: ${JSON.stringify(d)}`);
    await wait(200);
  }
}

const host = await open('host');
const guest = await open('guest');
await pick(host, 'Hosty');
await pick(guest, 'Guesty');
await menu(host);
await menu(guest);
// the menu's first frames go by before it reads keys as presses
await wait(2000);

// Host: Online's second row.
await tap(host, 'Down');
await tap(host, 'Space');
await heard(host, l => /^lobby: room /.test(l), 0, 'its room');
// its code, once it is up on the code server (a code taken re-rolls, and a lookup before the put finds nothing)
for (const end = Date.now() + 120000; !codes.size; ) {
  if (Date.now() > end) fail('host: no code on the code server in two minutes');
  await wait(100);
}
const code = host.lines.filter(l => l.startsWith('lobby: code ')).map(l => l.slice('lobby: code '.length).trim()).at(-1);
if (!code || !/^[2-9A-HJ-NP-Z]{4}$/.test(code)) fail(`host: no room code (${code})`);
console.error(`the host's room code: ${code}`);

// Guest: Join by Code (the fourth row), the code on the on-screen keyboard, Return to finish it.
await tap(guest, 'Down', 3);
await tap(guest, 'Space');
const ROWS = ['1234567890', 'qwertyuiop', 'asdfghjkl\'', 'zxcvbnm-.&'];
let at = [0, 0];
for (const c of code.toLowerCase()) {
  const row = ROWS.findIndex(r => r.includes(c));
  const col = ROWS[row].indexOf(c);
  await tap(guest, row > at[0] ? 'Down' : 'Up', Math.abs(row - at[0]));
  await tap(guest, col > at[1] ? 'Right' : 'Left', Math.abs(col - at[1]));
  await tap(guest, 'Space');
  at = [row, col];
}
await tap(guest, 'Return');
const [, seated] = await heard(guest, l => /^lobby: room .*\b2 seated/.test(l), 0, 'a seat in the host\'s room');
// Ready, the guest's lobby's third row
await tap(guest, 'Down', 2);
await tap(guest, 'Space');
await heard(host, l => /^lobby: room .*\b2 seated, ready true/.test(l), 0, 'the guest ready');
// Start, the host's lobby's seventh row
await tap(host, 'Down', 6);
await tap(host, 'Space');
await heard(host, l => l.startsWith('lobby: start '), 0, 'the start');
await heard(guest, l => l.startsWith('lobby: start '), seated, 'the start');

// Both ride on the gas (W), held from here.
for (const page of [host, guest]) await page.s('Input.dispatchKeyEvent', { type: 'rawKeyDown', ...KEYS.W });
const want = new RegExp(`state before tick ${TICKS} `);
await heard(host, l => want.test(l), 0, `tick ${TICKS} settled`);
await heard(guest, l => want.test(l), 0, `tick ${TICKS} settled`);
const settled = page => page.lines.filter(l => l.startsWith('online: settled '));
console.log(JSON.stringify({ code, host: settled(host), guest: settled(guest) }));
quit(0);
