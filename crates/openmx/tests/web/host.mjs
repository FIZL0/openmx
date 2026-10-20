// The browser's half of the web shell, in node: what web/worker.js does with the OPFS copy of the disc,
// done with a file instead, so the desktop's tests can run the browser's build (crates/openmx/tests/web.rs).
//
//   node host.mjs <openmx_web.wasm> <disc.iso> <width> <height> <out.rgba>
//
// The imports are web/worker.js's, call for call; only where the bytes come from and where the frame
// goes differ. Exit status 0 with the frame written, 1 with the shell's own message on stderr.
import fs from 'node:fs';

const [wasmPath, iso, width, height, out] = process.argv.slice(2);
const fd = fs.openSync(iso, 'r');
const size = fs.fstatSync(fd).size;
let memory;
let frame;
const bytes = (ptr, len) => new Uint8Array(memory.buffer, ptr, len);
const imports = {
  openmx: {
    disc_size: () => size,
    disc_read: (at, ptr, len) => {
      try {
        return fs.readSync(fd, bytes(ptr, len), 0, len, at);
      } catch {
        return -1;
      }
    },
    present: (ptr, w, h) => {
      frame = Buffer.from(bytes(ptr, w * h * 4));
    },
    log: (ptr, len) => console.error(new TextDecoder().decode(bytes(ptr, len))),
  },
};
const { instance } = await WebAssembly.instantiate(fs.readFileSync(wasmPath), imports);
memory = instance.exports.memory;
const err = instance.exports.openmx_start(Number(width), Number(height));
if (err) {
  console.error(new TextDecoder().decode(bytes(instance.exports.openmx_message(), err)));
  process.exit(1);
}
instance.exports.openmx_frame();
if (!frame) {
  console.error('openmx_frame presented nothing');
  process.exit(1);
}
fs.writeFileSync(out, frame);
