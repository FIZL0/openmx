//! The Mac app (P351, opentennis's abb59e30): a launch nobody watches (from the Finder or the Dock, where stdout and
//! stderr go to /dev/null) logs into `openmx-log.txt` in the data folder beside settings.txt, the launch before's kept
//! as `openmx-log-previous.txt` (reopening after a crash would otherwise overwrite the crash's log), as the phones do
//! (`ios.rs`, `android.rs`). A terminal, pipe or file on stderr keeps it.

use std::os::fd::AsRawFd;

/// stdout and stderr into `openmx-log.txt` in `dir` when nobody reads them. Call before the shell's threads start.
#[allow(unsafe_code)]
pub fn unwatched_to(dir: &std::path::Path) {
    // SAFETY: isatty/fstat on fd 2 and dup2 of a file this function opened, before any thread of ours writes
    unsafe {
        let mut st: libc::stat = std::mem::zeroed();
        let kind = if libc::fstat(2, &mut st) == 0 { st.st_mode & libc::S_IFMT } else { 0 };
        if libc::isatty(2) == 1 || kind == libc::S_IFIFO || kind == libc::S_IFREG {
            return;
        }
        let _ = std::fs::create_dir_all(dir);
        let path = dir.join("openmx-log.txt");
        let _ = std::fs::rename(&path, dir.join("openmx-log-previous.txt"));
        let Ok(f) = std::fs::File::create(&path) else { return };
        libc::dup2(f.as_raw_fd(), 1);
        libc::dup2(f.as_raw_fd(), 2);
    }
}
