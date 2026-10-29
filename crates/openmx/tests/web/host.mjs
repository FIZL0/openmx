// The browser's half of the web shell, in node: what web/worker.js does with the OPFS copy of the disc,
// done with a file instead, so the desktop's tests can run the browser's build (crates/openmx/tests/web.rs).
//
//   node host.mjs <pkg dir> <disc.iso> <width> <height> <out.rgba>
//
// The host calls are web/worker.js's, call for call; only where the bytes come from and where the frame
// goes differ. Exit status 0 with the frame written, 1 with the shell's own message on stderr.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const [pkg, iso, width, height, out] = process.argv.slice(2);
const fd = fs.openSync(iso, 'r');
const size = fs.fstatSync(fd).size;
let frame;
globalThis.openmx = {
  disc_size: () => size,
  disc_read: (at, into) => {
    try {
      return fs.readSync(fd, into, 0, into.length, at);
    } catch {
      return -1;
    }
  },
  present: (rgba, w, h) => {
    frame = Buffer.from(rgba.slice(0, w * h * 4));
  },
  log: (line) => console.error(line),
  // the game's (openmx_run's), which the start line's still never asks for: an empty window, no
  // input, no sound, no saved files, and a frame clock that never comes round
  view: () => Uint32Array.of(Number(width), Number(height)),
  keys: () => new Uint32Array(),
  clicks: () => new Int32Array(),
  gamepads: () => new Float32Array(),
  gamepad_name: () => '',
  play: () => {},
  queued: () => 0,
  file_read: () => undefined,
  file_write: () => {},
  frame: () => new Promise(() => {}),
  // P193's: no newer build, a first visit, nowhere to reload
  latest: () => '',
  last_build: () => undefined,
  reload: () => {},
  gpu: () => {},
  drawn: () => {},
};
const glue = await import(pathToFileURL(path.join(pkg, 'openmx_web.js')));
await glue.default({ module_or_path: fs.readFileSync(path.join(pkg, 'openmx_web_bg.wasm')) });
const err = glue.openmx_start(Number(width), Number(height));
if (err) {
  console.error(err);
  process.exit(1);
}
glue.openmx_frame();
if (!frame) {
  console.error('openmx_frame presented nothing');
  process.exit(1);
}
fs.writeFileSync(out, frame);
