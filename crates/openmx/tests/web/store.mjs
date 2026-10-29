// The mod store in the page (P198; crates/openmx/tests/web.rs, `the_browser_installs_from_the_store`): web/ served
// on loopback with a fake store beside it under /store (the openmx-mods dataset's API and files, its mod.zip behind a
// redirect to /cdn as Hugging Face's is), opened in headless chromium with `?software&args=--screen store --mod-store …`, the
// disc picked, then Space (Cross) on the store's one card until the mod is in OPFS `mods/`.
//
//   node store.mjs <web dir> <pkg dir> <disc.iso> <scratch profile dir>
//
// Prints one JSON line: { state, files: [OPFS mods/ paths], stamp, left: [OPFS store/ names], zips, ranges, presses }
// — `stamp` the installed store.json's sha256, `left` what is still in the downloads folder, `zips` the mod.zip
// requests the store and its CDN saw, `ranges` how many asked a Range.
// With OPENMX_STORE_LOG set, each store request is printed to stderr; with OPENMX_STORE_SHOT a path, the page's last
// picture is written there.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import crypto from 'node:crypto';
import { spawn } from 'node:child_process';

const [webdir, pkg, iso, profile] = process.argv.slice(2);
const REPO = 'FIZL0/openmx-mods';
const ID = 'web_store_track';

// The mod: a track's mod.json, a scene of 2 MB that does not deflate much (so the download comes in many pieces),
// and a preview still, zipped as the store's tooling zips (deflate, made on Unix).
const modJson = JSON.stringify({ standard: 1, kind: 'track', id: ID, name: 'Web Store Track', version: '1.0.0', author: 'someone',
  permission: 'test', source: { game: 'MX vs. ATV Unleashed' }, todo: [], songs: [] });
let seed = 0x198;
const scene = Buffer.alloc(2 << 20);
for (let i = 0; i < scene.length; i++) scene[i] = (seed = (Math.imul(seed, 1103515245) + 12345) >>> 0) >>> 24;
const files = [['mod.json', Buffer.from(modJson)], ['MC01.scn', scene], ['previews/0.png', Buffer.alloc(300, 7)]];
function zip(entries) {
  const out = [];
  const dir = [];
  let at = 0;
  for (const [name, data] of entries) {
    const packed = zlib.deflateRawSync(data);
    const crc = zlib.crc32(data);
    const n = Buffer.from(name);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(8, 8);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(packed.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(n.length, 26);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE((3 << 8) | 20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(8, 10);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(packed.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(n.length, 28);
    central.writeUInt32LE((0o100644 << 16) >>> 0, 38);
    central.writeUInt32LE(at, 42);
    out.push(local, n, packed);
    dir.push(central, n);
    at += local.length + n.length + packed.length;
  }
  const d = Buffer.concat(dir);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(d.length, 12);
  end.writeUInt32LE(at, 16);
  return Buffer.concat([...out, d, end]);
}
const modZip = zip(files);
const sha = crypto.createHash('sha256').update(modZip).digest('hex');
const meta = { id: ID, kind: 'track', name: 'Web Store Track', author: 'someone', source: 'MX vs. ATV Unleashed', version: '1.0.0',
  size: modZip.length, sha256: sha, standard: 1 };
const store = new Map([
  [`/api/datasets/${REPO}/tree/main/mods`, JSON.stringify([{ type: 'directory', path: `mods/${ID}` }])],
  [`/api/datasets/${REPO}/discussions?type=pull_request&status=open&p=0`, JSON.stringify({ discussions: [], count: 0 })],
  [`/api/datasets/${REPO}/refs?include_prs=1`, JSON.stringify({ pullRequests: [] })],
  [`/datasets/${REPO}/resolve/main/mods/${ID}/meta.json`, JSON.stringify(meta)],
  [`/cdn/datasets/${REPO}/resolve/main/mods/${ID}/mod.zip`, modZip],
]);
let zips = 0;
let ranges = 0;

const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.svg': 'image/svg+xml',
  '.webmanifest': 'application/manifest+json', '.ttf': 'font/ttf' };
const server = http.createServer((req, res) => {
  const u = new URL(req.url, 'http://x');
  if (u.pathname.startsWith('/store/')) {
    const p = req.url.slice('/store'.length);
    if (process.env.OPENMX_STORE_LOG) console.error('store:', p);
    if (p.endsWith('/mod.zip')) {
      zips++;
      if (req.headers.range) ranges++;
    }
    if (p.endsWith('/mod.zip') && !p.startsWith('/cdn/')) {
      res.writeHead(302, { location: `/store/cdn${p}` });
      res.end();
      return;
    }
    const b = store.get(p);
    res.writeHead(b ? 200 : 404, { 'content-type': 'application/octet-stream' });
    res.end(b ?? '{"error":"not found"}');
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
    // the picked disc read where it is, as gpu.mjs's: OPFS is still the store's, but the disc is not copied into it
    if (p === '/index.html') b = Buffer.from(String(b).replace('opfs = true;', 'opfs = false;'));
    res.writeHead(200, { 'content-type': types[path.extname(f)] ?? 'application/octet-stream' });
    res.end(b);
  });
}).listen(0, '127.0.0.1');
await new Promise(r => server.on('listening', r));
const origin = `http://127.0.0.1:${server.address().port}`;
// the software picture: the menu is all this needs, and no GPU fallback reloads the page to ask for the disc again
const url = `${origin}/?software&args=${encodeURIComponent(`--screen store --mod-store ${origin}/store`)}`;

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
ws.addEventListener('message', ({ data }) => {
  const m = JSON.parse(data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  } else if (m.method === 'Runtime.exceptionThrown') console.error('page:', m.params.exceptionDetails.text);
  else if (m.method === 'Target.attachedToTarget') send('Runtime.enable', {}, m.params.sessionId);
  else if (m.method === 'Runtime.consoleAPICalled') console.error('console:', m.params.args.map(a => a.value ?? a.description).join(' '));
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
// OPFS read from the page, the worker's origin: every file under `top`, by path, and the store.json's sha256
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
await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
await send('Page.navigate', { url });
let d = await until(d => ['asking', 'failed'].includes(d.state), 120000);
if (d.state !== 'asking') fail(`the page did not ask for the disc: ${JSON.stringify(d)}`);
const doc = await send('DOM.getDocument');
const pick = await send('DOM.querySelector', { nodeId: doc.result.root.nodeId, selector: '#pick' });
await send('DOM.setFileInputFiles', { files: [iso], nodeId: pick.result.nodeId });
d = await until(d => d.state === 'started' || d.state === 'failed', 120000);
if (d.state !== 'started') fail(`the page did not start: ${JSON.stringify(d)}`);

// Space (the keyboard's Cross; Return is Start, Refresh) on the lit card, the store's only one, until the mod is in: a press before the listing is in lights
// nothing, and one while the install runs (or after) does nothing more, so pressing again is safe
const key = type => send('Input.dispatchKeyEvent', { type, key: ' ', code: 'Space', windowsVirtualKeyCode: 32 });
let presses = 0;
let got = { files: [], stamp: null };
for (const end = Date.now() + 120000; Date.now() < end && !got.stamp; ) {
  if (presses < 40) {
    await key('keyDown');
    await new Promise(r => setTimeout(r, 80));
    await key('keyUp');
    presses++;
  }
  await new Promise(r => setTimeout(r, 2000));
  got = await opfs('mods');
}
// the download's file closes and goes once the install is done
let left = [];
for (const end = Date.now() + 10000; Date.now() < end; ) {
  left = (await opfs('store')).files;
  if (!left.length) break;
  await new Promise(r => setTimeout(r, 200));
}
if (process.env.OPENMX_STORE_SHOT)
  fs.writeFileSync(process.env.OPENMX_STORE_SHOT, Buffer.from((await send('Page.captureScreenshot', { format: 'png' })).result.data, 'base64'));
d = await data();
console.log(JSON.stringify({ state: d.state, files: got.files, stamp: got.stamp, sha, left, zips, ranges, presses }));
chrome.kill();
await new Promise(r => chrome.on('exit', r));
server.close();
process.exit(0);
