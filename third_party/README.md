# Vendored crates

Copies of upstream crates with changes of our own. Each is a path dependency (`ggrs`) or a
`[patch.crates-io]` entry in the root `Cargo.toml` (`winit`).

| crate | upstream | files changed | why |
|---|---|---|---|
| `ggrs` | 0.13.0 | see opentennis's third_party/README.md | N2: its thread-drivable clock (opentennis B494) |
| `winit` | 0.30.13 | `src/platform_impl/ios/window.rs`, `src/platform_impl/ios/event_loop.rs` | P200, from opentennis M91 (b93ab164): iOS 27's UIScene lifecycle; windows join the app's scene once it connects |
| | | `src/platform/ios.rs`, `src/platform_impl/ios/view_controller.rs` | P200, from opentennis B694 (1949d952): `ValidOrientations::LandscapeRight`, the landscape side held under rotation lock |
| | | `src/platform/android.rs`, `src/platform_impl/android/mod.rs` | opentennis M101/M108's Android pad queue and lifecycle hook, copied with the rest; unused here |

The winit copy is opentennis's `third_party/winit` at 1949d952, unchanged.
