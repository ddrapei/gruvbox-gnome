//! Writing files safely: atomically, and never destroying a file this program
//! did not create.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};

const BACKUP_SUFFIX: &str = ".before-gruvbox";

pub fn tmp_path(path: &Path) -> PathBuf {
    with_suffix(path, ".tmp")
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Readers see either the old or the new file, never a partial one.
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let dir = path.parent().ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "no parent directory"))?;
    fs::create_dir_all(dir)?;
    let tmp = tmp_path(path);
    match fs::remove_file(&tmp) {
        Err(e) if e.kind() != ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    // create_new = O_CREAT|O_EXCL: never follows a symlink planted at `tmp`.
    let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)?;
    // Make the rename itself durable.
    File::open(dir)?.sync_all()
}

/// What `install` did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Unchanged,
    Created,
    Updated,
    /// A file this program did not make was moved aside to `backup`.
    Replaced { backup: PathBuf },
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Outcome::Unchanged => write!(f, "up to date"),
            Outcome::Created => write!(f, "created"),
            Outcome::Updated => write!(f, "updated"),
            Outcome::Replaced { backup } => write!(f, "replaced (your previous file is kept as {})", backup.display()),
        }
    }
}

/// Make `path` contain `contents`. An existing file that `is_ours` does not
/// recognise (made by hand or by another program) is first moved aside to
/// `<name>.before-gruvbox` (or `.2`, `.3`, ...), so it is never lost. A
/// symlink is moved aside itself; its target is left alone.
pub fn install(path: &Path, contents: &[u8], is_ours: impl Fn(&[u8]) -> bool) -> io::Result<Outcome> {
    let present = fs::symlink_metadata(path).is_ok();
    let existing = match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == ErrorKind::NotFound => None, // missing, or a dangling symlink
        Err(e) => return Err(e),
    };
    let outcome = match existing {
        Some(bytes) if bytes == contents => return Ok(Outcome::Unchanged),
        Some(bytes) if is_ours(&bytes) => Outcome::Updated,
        _ if present => {
            let backup = free_backup_path(path)?;
            fs::rename(path, &backup)?;
            Outcome::Replaced { backup }
        }
        _ => Outcome::Created,
    };
    write_atomic(path, contents)?;
    Ok(outcome)
}

fn free_backup_path(path: &Path) -> io::Result<PathBuf> {
    (1..1000)
        .map(|n| with_suffix(path, &if n == 1 { BACKUP_SUFFIX.to_string() } else { format!("{BACKUP_SUFFIX}.{n}") }))
        .find(|candidate| fs::symlink_metadata(candidate).is_err())
        .ok_or_else(|| io::Error::new(ErrorKind::AlreadyExists, "too many backups"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir;

    const OURS: &[u8] = b"# ours\nnew";
    fn is_ours(bytes: &[u8]) -> bool {
        bytes.starts_with(b"# ours")
    }

    #[test]
    fn create_update_unchanged() {
        let dir = test_dir("fsutil-install");
        let path = dir.join("sub/file");
        assert_eq!(install(&path, b"# ours\nold", is_ours).unwrap(), Outcome::Created);
        assert_eq!(install(&path, OURS, is_ours).unwrap(), Outcome::Updated);
        assert_eq!(install(&path, OURS, is_ours).unwrap(), Outcome::Unchanged);
        assert_eq!(fs::read(&path).unwrap(), OURS);
    }

    #[test]
    fn foreign_files_are_backed_up_not_lost() {
        let dir = test_dir("fsutil-backup");
        let path = dir.join("gtk.css");
        for (i, foreign) in ["hand-made 1", "hand-made 2"].iter().enumerate() {
            fs::write(&path, foreign).unwrap();
            let Outcome::Replaced { backup } = install(&path, OURS, is_ours).unwrap() else { panic!("not replaced") };
            assert_eq!(fs::read_to_string(&backup).unwrap(), *foreign);
            assert!(backup.to_string_lossy().ends_with(if i == 0 { ".before-gruvbox" } else { ".before-gruvbox.2" }));
        }
    }

    #[test]
    fn a_symlink_is_moved_aside_and_its_target_kept() {
        let dir = test_dir("fsutil-symlink");
        let target = dir.join("theme-file");
        fs::write(&target, "theme").unwrap();
        let path = dir.join("gtk.css");
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(matches!(install(&path, OURS, is_ours).unwrap(), Outcome::Replaced { .. }));
        assert_eq!(fs::read_to_string(&target).unwrap(), "theme");
        assert!(!fs::symlink_metadata(&path).unwrap().file_type().is_symlink());
    }

    #[test]
    fn write_atomic_replaces_a_stale_tmp_symlink_instead_of_following_it() {
        let dir = test_dir("fsutil-tmp-symlink");
        let victim = dir.join("victim");
        fs::write(&victim, "keep").unwrap();
        let target = dir.join("out.css");
        std::os::unix::fs::symlink(&victim, tmp_path(&target)).unwrap();
        write_atomic(&target, b"new").unwrap();
        assert_eq!(fs::read_to_string(&victim).unwrap(), "keep");
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    }
}
