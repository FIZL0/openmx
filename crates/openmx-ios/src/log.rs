//! The launch before's log (P212): the log is written fresh each launch, so the one a player saw
//! something wrong in is gone by the time they open the app again to look. It is kept beside the
//! new one instead, one launch deep.

use std::path::{Path, PathBuf};

/// Where the launch before's log is kept: `openmx-log.txt` → `openmx-log-previous.txt`.
pub fn previous_log(path: &Path) -> PathBuf {
    let stem = path.file_stem().map_or_else(|| "log".into(), |s| s.to_string_lossy().into_owned());
    let name = match path.extension() {
        Some(ext) => format!("{stem}-previous.{}", ext.to_string_lossy()),
        None => format!("{stem}-previous"),
    };
    path.with_file_name(name)
}

/// Moves the log at `path`, if there is one, over [`previous_log`]'s, before a launch starts its own.
pub fn keep_previous_log(path: &Path) {
    if path.exists() {
        let _ = std::fs::rename(path, previous_log(path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_previous_log_sits_beside_the_log() {
        assert_eq!(previous_log(Path::new("/d/openmx-log.txt")), Path::new("/d/openmx-log-previous.txt"));
        assert_eq!(previous_log(Path::new("/d/log")), Path::new("/d/log-previous"));
    }

    #[test]
    fn a_launch_keeps_one_log_back() {
        let dir = std::env::temp_dir().join(format!("openmx-ios-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("openmx-log.txt");
        // The first launch: nothing to keep.
        keep_previous_log(&log);
        assert!(!previous_log(&log).exists());
        std::fs::write(&log, "first").unwrap();
        keep_previous_log(&log);
        std::fs::write(&log, "second").unwrap();
        keep_previous_log(&log);
        assert_eq!(std::fs::read_to_string(previous_log(&log)).unwrap(), "second");
        assert!(!log.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
