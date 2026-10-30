// A controller in the page (P236; crates/openmx/tests/web.rs, `a_pad_the_browser_shows_on_a_press_rides`): web/ served on
// loopback and opened in chromium as a player opens it (no `?args`), the disc picked, and then a pad that behaves as
// Chromium's Gamepad API does with a real one. The machine's own pads are shut out (K13: other slots' uinput pads reach
// every chromium here) and this one stands in for them, built the way the browser hands a pad over: `getGamepads()`
// is four slots of null until one of its buttons is pressed (the "gamepad user gesture" a page waits for), then a
// fresh snapshot each call, standard mapping, with a `gamepadconnected` event. Everything past that is the page's own:
// index.html's poll, the worker, web_host's PadNode and the pad layer's seats.
//
//   node gamepad.mjs <web dir> <disc.iso> <scratch profile dir> <out.png> [chromium flag...]
//
// The pad's first press is made on the splash, the press that shows it to the page: it must make the pad Player 1
// (the console's `pads: 1P: … the menus and the first seat are its`). Then Cross until a ride seats the pad
// (`pads: 1P: <pad>`), and the ride is watched on the pad's brake and then its gas. With OPENMX_GPU_PKG set the page's
// pkg/ is served from there. Prints one JSON line:
// { state, first, taps, seated, braked, gas, png } — `first` whether the first press made the pad Player 1, `taps` how
// many Cross presses it took to reach a ride, `braked` and `gas` the share of the speedometer that changed over two
// seconds on L2 and on R2. The last picture is <out.png>, the braked one beside it.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';

const [webdir, iso, profile, out, ...flags] = process.argv.slice(2);
const PKG = process.env.OPENMX_GPU_PKG;
const PAD = 'Xbox 360 Controller (XInput STANDARD GAMEPAD)';
const said = [];
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.svg': 'image/svg+xml',
  '.webmanifest': 'application/manifest+json', '.ttf': 'font/ttf' };
const server = http.createServer((req, res) => {
  let p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (p.endsWith('/')) p += 'index.html';
  const f = PKG && p.startsWith('/pkg/') ? path.join(PKG, path.normalize(p.slice(5))) : path.join(webdir, path.normalize(p));
  fs.readFile(f, (e, b) => {
    if (e) {
      res.writeHead(404);
      res.end();
      return;
    }
    // OPFS refused, as chromium.mjs's --no-opfs: the picked file is read where it is, and nothing is copied
    if (p === '/index.html') b = Buffer.from(String(b).replace('opfs = true;', 'opfs = false;'));
    res.writeHead(200, { 'content-type': types[path.extname(f)] ?? 'application/octet-stream' });
    res.end(b);
  });
}).listen(0, '127.0.0.1');
await new Promise(r => server.on('listening', r));
const url = `http://127.0.0.1:${server.address().port}/`;

// No extensions, as page.mjs (K7, K12): this machine's chromium brings its own into every profile, the policy's
// force-installed one fetched from the web store a few seconds in, and one arriving mid-load can abort the worker's
// module fetch; under a busy gate the WebGPU ride failed before it chose a GPU ('{"disc":"no","state":"failed"}').
const chrome = spawn('chromium', ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', '--disable-extensions', '--disable-component-extensions-with-background-pages',
  '--disable-background-networking', '--window-size=1280,720', ...flags, 'about:blank'],
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
  else if (m.method === 'Runtime.consoleAPICalled') {
    const line = m.params.args.map(a => a.value ?? a.description).join(' ');
    said.push(line);
    console.error('console:', line);
  }
});
const send = (method, params = {}, sessionId) => new Promise(r => {
  const i = ++id;
  pending.set(i, r);
  ws.send(JSON.stringify({ id: i, method, params, ...(sessionId ? { sessionId } : {}) }));
});
const evaluate = async expression =>
  (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result?.result?.value;
// mid-navigation (the first load, a fallback's reload) there is no page to ask: nothing is known yet
const data = async () => (await evaluate('({ ...document.body.dataset })')) ?? {};
const until = async (ok, ms) => {
  const t = Date.now();
  for (;;) {
    const d = await data();
    if (ok(d) || Date.now() - t > ms) return d;
    await new Promise(r => setTimeout(r, 100));
  }
};

await send('Runtime.enable');
await send('Page.enable');

await send('Runtime.enable');
await send('Page.enable');
// The pad, as Chromium gives a real one to the page (see the top): hidden until pressed, a snapshot each call.
await send('Page.addScriptToEvaluateOnNewDocument', { source: `{
  const button = () => ({ pressed: false, touched: false, value: 0 });
  const state = { buttons: Array.from({ length: 17 }, button), axes: [0, 0, 0, 0], timestamp: 0 };
  let shown = false;
  const snapshot = () => ({ id: ${JSON.stringify(PAD)}, index: 0, connected: true, mapping: 'standard', timestamp: state.timestamp,
    buttons: state.buttons.map(b => ({ ...b })), axes: [...state.axes], vibrationActuator: null });
  navigator.getGamepads = () => [shown ? snapshot() : null, null, null, null];
  window.openmxTestPad = {
    set(i, v) {
      state.buttons[i] = { pressed: v > 0.5, touched: v > 0, value: v };
      state.timestamp = performance.now();
      if (v > 0.5 && !shown) {
        shown = true;
        const e = new Event('gamepadconnected');
        e.gamepad = snapshot();
        dispatchEvent(e);
      }
    },
  };
}` });
await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
await send('Page.navigate', { url });
let d = await until(d => ['asking', 'failed'].includes(d.state), 60000);
if (d.state !== 'asking') fail(`the page did not ask for the disc: ${JSON.stringify(d)}`);
const doc = await send('DOM.getDocument');
const pick = await send('DOM.querySelector', { nodeId: doc.result.root.nodeId, selector: '#pick' });
await send('DOM.setFileInputFiles', { files: [iso], nodeId: pick.result.nodeId });
d = await until(d => d.state === 'failed' || d.drawn || (d.state === 'started' && d.gpu === 'software'), 180000);
if (d.state === 'failed' || !(d.drawn || d.gpu === 'software'))
  fail(`the page did not start drawing: ${JSON.stringify(d)} (${await evaluate("document.querySelector('#status')?.textContent")})`);

const wait = ms => new Promise(r => setTimeout(r, ms));
// a line the console said since `from`, waited for up to `ms`
const heard = async (from, test, ms) => {
  const t = Date.now();
  for (;;) {
    if (said.slice(from).some(test)) return true;
    if (Date.now() - t > ms) return false;
    await wait(50);
  }
};
// A press as a hand makes one: down for a few of the page's frames, then up.
const tap = async b => {
  await evaluate(`openmxTestPad.set(${b}, 1)`);
  await wait(100);
  await evaluate(`openmxTestPad.set(${b}, 0)`);
};
// The splash: the front end is up once the menus' song plays; a moment more for its first frames.
if (!(await heard(0, l => l.startsWith('music: playing'), 60000))) fail('no song started: the front end never came up');
await wait(2000);
let from = said.length;
await tap(0);
const menus = l => l.startsWith(`pads: 1P: ${PAD}`) && l.includes('the menus');
const first = await heard(from, menus, 3000);
// On through the menus with Cross until a ride seats the pad.
const seated = l => l.startsWith(`pads: 1P: ${PAD}`) && !l.includes('the menus');
let taps = 1;
from = said.length;
while (!(await heard(from, seated, 1500)) && taps < 12) {
  await tap(0);
  taps++;
}
const ride = said.slice(from).some(seated);
// The ride, read off the speedometer (the picture's bottom-right corner, where the HUD puts the speed's digits): the
// start line is a slope a bike rolls down, so the pad's L2 (the brake) is held first and the digits must stand still
// over two seconds, then R2 (the gas) and they must keep changing as it gathers speed.
const shot = async () => (await send('Page.captureScreenshot', { format: 'png' })).result.data;
const change = (a, b) => evaluate(`(async () => {
  const read = async s => {
    const i = await createImageBitmap(await (await fetch('data:image/png;base64,' + s)).blob());
    const [x, y, w, h] = [Math.round(i.width * 0.84), Math.round(i.height * 0.70), Math.round(i.width * 0.12), Math.round(i.height * 0.18)];
    const c = new OffscreenCanvas(w, h);
    const g = c.getContext('2d');
    g.drawImage(i, x, y, w, h, 0, 0, w, h);
    return g.getImageData(0, 0, w, h).data;
  };
  const [p, q] = [await read('${a}'), await read('${b}')];
  let moved = 0;
  for (let k = 0; k < p.length; k += 4)
    if (Math.abs(p[k] - q[k]) + Math.abs(p[k + 1] - q[k + 1]) + Math.abs(p[k + 2] - q[k + 2]) > 96) moved++;
  return moved / (p.length / 4);
})()`);
let braked = null;
let gas = null;
if (ride) {
  await evaluate('openmxTestPad.set(6, 1)');
  await wait(5000);
  const a = await shot();
  await wait(2000);
  const b = await shot();
  braked = await change(a, b);
  await evaluate('openmxTestPad.set(6, 0)');
  await evaluate('openmxTestPad.set(7, 1)');
  await wait(1500);
  const c = await shot();
  await wait(2000);
  const e = await shot();
  gas = await change(c, e);
  fs.writeFileSync(out.replace(/\.png$/, '-braked.png'), Buffer.from(b, 'base64'));
  fs.writeFileSync(out, Buffer.from(e, 'base64'));
}
console.log(JSON.stringify({ state: (await data()).state, first, taps, seated: ride, braked, gas, png: out }));
chrome.kill();
await new Promise(r => chrome.on('exit', r));
server.close();
process.exit(0);
