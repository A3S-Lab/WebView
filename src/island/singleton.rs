use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

use fs2::FileExt;

pub(crate) struct IslandLock {
    file: File,
}

impl IslandLock {
    /// Acquires the per-user island lock. `Ok(None)` means another healthy
    /// helper already owns it, which is a successful no-op for callers.
    pub(crate) fn acquire(path: &Path) -> Result<Option<Self>, String> {
        let parent = path
            .parent()
            .ok_or_else(|| "island lock has no parent directory".to_string())?;
        validate_private_directory(parent)?;
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err("island lock path is not a regular file".to_string());
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("inspect island lock: {error}")),
        }

        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let file = options
            .open(path)
            .map_err(|error| format!("open island lock: {error}"))?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("read island lock metadata: {error}"))?;
        if !metadata.is_file() {
            return Err("island lock is not a regular file".to_string());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};

            // SAFETY: geteuid has no preconditions and only reads process state.
            if metadata.uid() != unsafe { libc::geteuid() } {
                return Err("island lock must be owned by the current user".to_string());
            }
            file.set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|error| format!("secure island lock permissions: {error}"))?;
        }

        match FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Some(Self { file })),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(format!("lock agent island singleton: {error}")),
        }
    }
}

pub(crate) fn validate_private_directory(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("open private island directory: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("island state parent is not a real directory".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        // SAFETY: geteuid has no preconditions and only reads process state.
        if metadata.uid() != unsafe { libc::geteuid() } {
            return Err("island state directory must be owned by the current user".to_string());
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(
                "island state directory permissions must not grant group or other access"
                    .to_string(),
            );
        }
    }
    Ok(())
}

impl Drop for IslandLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> std::path::PathBuf {
        let nonce = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "a3s-webview-island-lock-{}-{epoch}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        path
    }

    #[test]
    fn only_one_lock_owner_is_admitted() {
        let directory = temp_dir();
        let path = directory.join("island.lock");
        let first = IslandLock::acquire(&path).unwrap().unwrap();
        assert!(IslandLock::acquire(&path).unwrap().is_none());
        drop(first);
        assert!(IslandLock::acquire(&path).unwrap().is_some());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn lock_excludes_a_second_helper_process() {
        let directory = temp_dir();
        let path = directory.join("island.lock");
        let first = IslandLock::acquire(&path).unwrap().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "island::singleton::tests::child_observes_held_lock",
            ])
            .env("A3S_ISLAND_TEST_LOCK_PATH", &path)
            .status()
            .unwrap();
        assert!(status.success());
        drop(first);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn child_observes_held_lock() {
        let Some(path) = std::env::var_os("A3S_ISLAND_TEST_LOCK_PATH") else {
            return;
        };
        assert!(IslandLock::acquire(Path::new(&path)).unwrap().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn lock_file_is_private() {
        use std::os::unix::fs::PermissionsExt;

        let directory = temp_dir();
        let path = directory.join("island.lock");
        let _lock = IslandLock::acquire(&path).unwrap().unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        drop(_lock);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_private_state_directory() {
        use std::os::unix::fs::PermissionsExt;

        let directory = temp_dir();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        let error = IslandLock::acquire(&directory.join("island.lock"))
            .err()
            .expect("exposed directory should be rejected");
        assert!(error.contains("permissions"), "{error}");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_lock_path() {
        use std::os::unix::fs::symlink;

        let directory = temp_dir();
        let target = directory.join("target.lock");
        std::fs::write(&target, b"").unwrap();
        let path = directory.join("island.lock");
        symlink(&target, &path).unwrap();
        let error = IslandLock::acquire(&path)
            .err()
            .expect("symlink lock should be rejected");
        assert!(error.contains("regular file"), "{error}");
        std::fs::remove_dir_all(directory).unwrap();
    }
}
