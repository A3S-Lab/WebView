use super::control::{AgentControlActionKind, AuthorizedControl, ControlTransport};
use std::path::{Path, PathBuf};

const RESPONSE_BYTES: u64 = 1_024;

pub(crate) struct SensitiveControlClient {
    path: PathBuf,
}

impl SensitiveControlClient {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = path.into();
        if !path.is_absolute() {
            return Err("sensitive control socket path must be absolute".to_string());
        }
        let parent = path
            .parent()
            .ok_or_else(|| "sensitive control socket has no parent".to_string())?;
        super::singleton::validate_private_directory(parent)?;
        Ok(Self { path })
    }

    pub(crate) fn submit(
        &self,
        control: &mut AuthorizedControl,
        now_ms: u64,
    ) -> Result<(), String> {
        if control.transport != ControlTransport::EphemeralSocket
            || control.action != AgentControlActionKind::SetLlmApiKey
        {
            return Err("control is not eligible for the sensitive socket".to_string());
        }
        submit_unix(&self.path, control, now_ms)
    }
}

#[cfg(unix)]
fn submit_unix(path: &Path, control: &mut AuthorizedControl, now_ms: u64) -> Result<(), String> {
    use serde::Deserialize;
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;
    use zeroize::Zeroizing;

    super::singleton::validate_private_directory(
        path.parent()
            .ok_or_else(|| "sensitive control socket has no parent".to_string())?,
    )?;
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("inspect sensitive control socket: {error}"))?;
    if !metadata.file_type().is_socket()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err("sensitive control socket ownership or permissions are unsafe".to_string());
    }

    let mut stream = UnixStream::connect(path)
        .map_err(|error| format!("connect sensitive control socket: {error}"))?;
    if peer_uid(stream.as_raw_fd())? != unsafe { libc::geteuid() } {
        return Err("sensitive control socket belongs to another user".to_string());
    }
    let timeout = Some(Duration::from_secs(2));
    stream
        .set_read_timeout(timeout)
        .and_then(|()| stream.set_write_timeout(timeout))
        .map_err(|error| format!("configure sensitive control timeout: {error}"))?;

    let (_, request) = super::control::encode_protocol_request(control, now_ms)?;
    let request = Zeroizing::new(request);
    stream
        .write_all(&request)
        .map_err(|error| format!("write sensitive control request: {error}"))?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| format!("finish sensitive control request: {error}"))?;
    let mut response = Vec::with_capacity(32);
    stream
        .take(RESPONSE_BYTES + 1)
        .read_to_end(&mut response)
        .map_err(|error| format!("read sensitive control response: {error}"))?;
    if response.len() as u64 > RESPONSE_BYTES {
        return Err("sensitive control response exceeds the size limit".to_string());
    }
    #[derive(Deserialize)]
    struct Response {
        accepted: bool,
    }
    let response: Response = serde_json::from_slice(&response)
        .map_err(|_| "sensitive control response is invalid".to_string())?;
    if !response.accepted {
        return Err("sensitive control was rejected".to_string());
    }
    if let Some(message) = &mut control.message {
        use zeroize::Zeroize;
        message.zeroize();
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn peer_uid(fd: std::os::fd::RawFd) -> Result<libc::uid_t, String> {
    let mut uid = 0;
    let mut gid = 0;
    let result = unsafe { libc::getpeereid(fd, &mut uid, &mut gid) };
    if result == 0 {
        Ok(uid)
    } else {
        Err(format!(
            "read sensitive control peer credentials: {}",
            std::io::Error::last_os_error()
        ))
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn peer_uid(fd: std::os::fd::RawFd) -> Result<libc::uid_t, String> {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            std::ptr::addr_of_mut!(credentials).cast(),
            &mut length,
        )
    };
    if result == 0 && length as usize == std::mem::size_of::<libc::ucred>() {
        Ok(credentials.uid)
    } else {
        Err(format!(
            "read sensitive control peer credentials: {}",
            std::io::Error::last_os_error()
        ))
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "linux", target_os = "android"))
))]
fn peer_uid(_fd: std::os::fd::RawFd) -> Result<libc::uid_t, String> {
    Err("sensitive control peer credentials are unsupported on this platform".to_string())
}

#[cfg(not(unix))]
fn submit_unix(_path: &Path, _control: &mut AuthorizedControl, _now_ms: u64) -> Result<(), String> {
    Err("sensitive controls require a Unix-domain socket".to_string())
}
