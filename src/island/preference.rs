use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;

const AGENT_ISLAND_DISABLED_FILE: &str = "island.disabled";

pub(crate) fn is_disabled_for_snapshot(snapshot: &Path) -> bool {
    let Some(directory) = snapshot.parent() else {
        return true;
    };
    match std::fs::symlink_metadata(directory.join(AGENT_ISLAND_DISABLED_FILE)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Ok(_) | Err(_) => true,
    }
}

pub(crate) fn disable_for_snapshot(snapshot: &Path) -> Result<(), String> {
    let directory = snapshot
        .parent()
        .ok_or_else(|| "snapshot has no private parent directory".to_string())?;
    let path = directory.join(AGENT_ISLAND_DISABLED_FILE);
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(mut file) => {
            file.write_all(b"disabled\n")
                .map_err(|error| format!("write {}: {error}", path.display()))?;
            file.sync_all()
                .map_err(|error| format!("sync {}: {error}", path.display()))
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(format!("create {}: {error}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disable_marker_is_created_once_beside_the_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "a3s-agent-island-preference-{}-{}",
            std::process::id(),
            crate::island::epoch_ms()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let snapshot = root.join("system-snapshot.json");

        assert!(!is_disabled_for_snapshot(&snapshot));
        disable_for_snapshot(&snapshot).unwrap();
        disable_for_snapshot(&snapshot).unwrap();
        assert!(is_disabled_for_snapshot(&snapshot));
        assert_eq!(
            std::fs::read_to_string(root.join(AGENT_ISLAND_DISABLED_FILE)).unwrap(),
            "disabled\n"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                std::fs::metadata(root.join(AGENT_ISLAND_DISABLED_FILE))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        let _ = std::fs::remove_dir_all(root);
    }
}
