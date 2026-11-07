// private file helpers: owner-only perms on unix

use std::io::{self, Write};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

// write/create a file readable only by the current user (0600).
// mode applies on create; existing files get their perms corrected too.
// permission failures are returned to the caller (no silent success).
pub fn write_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut f = options.open(path)?;
    f.write_all(contents)?;
    f.sync_all()?;
    drop(f);
    #[cfg(unix)]
    {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

// create a directory tree with owner-only access (0700 on unix).
// permission failures are returned to the caller.
pub fn create_dir_private(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(unix)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn write_private_sets_0600() {
        let dir = std::env::temp_dir().join(format!("tox-priv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        create_dir_private(&dir).unwrap();
        let dir_mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, "data dir should be 0700");

        let path = dir.join("secret.bin");
        write_private(&path, b"hello").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "file should be 0600");

        // rewrite keeps 0600 even if something loosened it
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_private(&path, b"hello2").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "rewrite should restore 0600");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn permission_errors_are_propagated() {
        // non-directory parent: create_dir_all fails, must not be Ok
        let file = std::env::temp_dir().join(format!("tox-notdir-{}", std::process::id()));
        std::fs::write(&file, b"x").unwrap();
        let child = file.join("nested");
        assert!(create_dir_private(&child).is_err());
        let _ = std::fs::remove_file(&file);
    }
}
