// A `cargo test` runner for wasm32-wasip1, using node's own WASI implementation, so that proving the
// hash on a 32-bit target needs no extra tool installed. Used by `platforms.sh`; see its header.
//
//   node --no-warnings wasi-run.js <test.wasm> [test harness args...]
//
// The test binary gets the whole filesystem preopened, because the source-scanning tests in
// tests/platforms.rs read the crate's own sources, and `/` is where cargo's absolute paths point.
const { WASI } = require('node:wasi');
const fs = require('node:fs');

const wasm = process.argv[2];
if (!wasm) {
  console.error('usage: wasi-run.js <test.wasm> [args...]');
  process.exit(2);
}
const wasi = new WASI({
  version: 'preview1',
  args: [wasm, ...process.argv.slice(3)],
  env: process.env,
  preopens: { '/': '/' },
});

(async () => {
  const module = await WebAssembly.compile(fs.readFileSync(wasm));
  const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
  // wasi.start returns the exit code; a test failure is a non-zero one and must reach cargo.
  process.exitCode = wasi.start(instance);
})();
