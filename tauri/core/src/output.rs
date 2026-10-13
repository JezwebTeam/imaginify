//! Writing outputs. Nothing here ever replaces an existing file.
//!
//! Normally the data goes to a hidden temp file in the output folder and is
//! then renamed onto the first free name with an atomic no-replace rename, so
//! a crash or quit mid-write leaves only a `.imaginify-*.tmp` file, never a
//! truncated image. Volumes that support neither a no-replace rename nor hard
//! links (some network shares, FAT) fall back to an exclusive create of the
//! final name, which also never replaces anything.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

const MAX_NUMBER: u32 = 9999;

/// Strip characters that would turn the suffix into a path or an invalid name.
pub fn sanitize_suffix(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() && !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect()
}

fn candidate(out_dir: &Path, base: &str, ext: &str, n: u32) -> PathBuf {
    if n == 1 {
        out_dir.join(format!("{base}.{ext}"))
    } else {
        out_dir.join(format!("{base} ({n}).{ext}"))
    }
}

fn no_free_name(out_dir: &Path, base: &str, ext: &str) -> anyhow::Error {
    anyhow!("no free file name for {base}.{ext} in {}", out_dir.display())
}

/// Write `data` to `out_dir/<base>.<ext>`, or `<base> (2).<ext>`, `(3)`... if
/// that name is taken (by an earlier output, another file in this batch, the
/// source image itself, or anything else).
pub fn write_unique(out_dir: &Path, base: &str, ext: &str, data: &[u8]) -> Result<PathBuf> {
    let mut tmp = tempfile::Builder::new()
        .prefix(".imaginify-")
        .suffix(".tmp")
        .tempfile_in(out_dir)
        .with_context(|| format!("cannot write to {}", out_dir.display()))?;
    tmp.write_all(data)?;
    tmp.flush()?;
    let mut tmp = tmp.into_temp_path();

    for n in 1..=MAX_NUMBER {
        let path = candidate(out_dir, base, ext, n);
        match tmp.persist_noclobber(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.error.kind() == ErrorKind::AlreadyExists => tmp = e.path,
            Err(_) => {
                // Dropping the TempPath (inside the error) deletes the temp file.
                return write_exclusive(out_dir, base, ext, data, n);
            }
        }
    }
    Err(no_free_name(out_dir, base, ext))
}

/// Create the first free name with `create_new` (O_EXCL / CREATE_NEW) and
/// write into it. Never replaces a file; if the write fails, the file we just
/// created is removed.
pub(crate) fn write_exclusive(out_dir: &Path, base: &str, ext: &str, data: &[u8], start: u32) -> Result<PathBuf> {
    for n in start..=MAX_NUMBER {
        let path = candidate(out_dir, base, ext, n);
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(anyhow!("cannot save {}: {e}", path.display())),
        };
        if let Err(e) = file.write_all(data).and_then(|()| file.flush()) {
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(anyhow!("cannot save {}: {e}", path.display()));
        }
        return Ok(path);
    }
    Err(no_free_name(out_dir, base, ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_fallback_never_replaces() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.jpg"), b"original").unwrap();
        fs::write(dir.path().join("a (2).jpg"), b"earlier").unwrap();
        let out = write_exclusive(dir.path(), "a", "jpg", b"new", 1).unwrap();
        assert_eq!(out, dir.path().join("a (3).jpg"));
        assert_eq!(fs::read(dir.path().join("a.jpg")).unwrap(), b"original");
        assert_eq!(fs::read(dir.path().join("a (2).jpg")).unwrap(), b"earlier");
        assert_eq!(fs::read(out).unwrap(), b"new");
    }
}
