// The browser's online peer, run in node (crates/openmx/tests/netplay_browser.rs): the module and glue tools/web.sh
// builds, the page's `Online` (crates/openmx/src/web.rs) and the same frame loop web/worker.js runs — join by ticket
// through the relay, ready once seated, a frame every tick of the ride's 30 Hz clock (N6).
//
//   node peer.mjs <pkg dir> <relay url> <ticket> <ticks>
//
// Prints `state <JSON of Online.state()>` a few times a second, and once that many ticks are settled here, each
// of Online.ticks(0, <ticks>)'s lines as `tick <line>`. Keeps riding after that, so the desktop gets the inputs it
// still needs, until stdin closes.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const [pkg, relay, ticket, want] = process.argv.slice(2);
const T = Number(want);
const state = s => process.stdout.write(`state ${JSON.stringify(s)}\n`);
globalThis.openmx = {
  disc_size: () => 0,
  disc_read: () => -1,
  present: () => {},
  log: line => process.stderr.write(`peer: ${line}\n`),
};
const glue = await import(pathToFileURL(path.join(pkg, 'openmx_web.js')).href);
await glue.default({ module_or_path: fs.readFileSync(path.join(pkg, 'openmx_web_bg.wasm')) });

const net = new glue.Online('Browser', relay, '');
net.join(ticket, 'race125p');
let last = performance.now();
let told = 0;
let readied = false;
let sent = false;
setInterval(() => {
  const now = performance.now();
  net.frame(Math.min((now - last) / 1000, 0.25));
  last = now;
  const s = net.state();
  if (!readied && s.taken > 1) {
    net.ready(true);
    readied = true;
  }
  // settled, not confirmed: a tick confirmed between runs can still hold its prediction (K7)
  const lines = !sent && s.settled >= T ? net.ticks(0, T).split('\n').filter(Boolean) : [];
  if (lines.length === T) {
    process.stdout.write(lines.map(l => `tick ${l}\n`).join(''));
    state(s);
    sent = true;
  } else if (now - told > 250) {
    state(s);
    told = now;
  }
}, 1001 / 30);
process.stdin.on('end', () => process.exit(0));
process.stdin.resume();
