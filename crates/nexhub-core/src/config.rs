//! Runtime configuration, filesystem paths, auth token, and environment variables.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocketInfo {
    pub port: u16,
    pub pid: u32,
}

/// Resolves the NexHub data directory.
/// Can be overridden with the `NEXHUB_HOME` environment variable.
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("NEXHUB_HOME") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }

    if let Some(base) = directories::BaseDirs::new() {
        #[cfg(target_os = "windows")]
        {
            base.data_dir().join("NexHub")
        }
        #[cfg(target_os = "macos")]
        {
            base.data_dir().join("NexHub")
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            base.data_dir().join("nexhub")
        }
    } else {
        PathBuf::from(".nexhub-data")
    }
}

pub fn token_path(data_dir: &Path) -> PathBuf {
    data_dir.join("token")
}

pub fn socket_path(data_dir: &Path) -> PathBuf {
    data_dir.join("socket.json")
}

/// Reads the existing auth token from disk. Fail closed: returns error if missing or unreadable.
pub fn read_token(data_dir: &Path) -> std::io::Result<String> {
    let path = token_path(data_dir);
    let token = fs::read_to_string(&path)?.trim().to_string();
    if token.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "token file is empty",
        ));
    }
    Ok(token)
}

/// Reads or generates a 32-byte hex token (64 hex characters) and saves it to disk.
pub fn read_or_create_token(data_dir: &Path) -> std::io::Result<String> {
    fs::create_dir_all(data_dir)?;
    let path = token_path(data_dir);
    if path.exists() {
        return read_token(data_dir);
    }

    let token = generate_token_hex();
    fs::write(&path, &token)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        let _ = fs::set_permissions(&path, perms);
    }

    Ok(token)
}

/// Generates a random 32-byte hex string (64 characters).
pub fn generate_token_hex() -> String {
    let mut hex = String::with_capacity(64);
    for _ in 0..4 {
        let u = ulid::Ulid::new();
        let bytes = u.to_bytes();
        for b in bytes.iter().take(8) {
            hex.push_str(&format!("{:02x}", b));
        }
    }
    hex
}

pub fn write_socket_info(data_dir: &Path, port: u16, pid: u32) -> std::io::Result<()> {
    fs::create_dir_all(data_dir)?;
    let path = socket_path(data_dir);
    let info = SocketInfo { port, pid };
    let json = serde_json::to_string_pretty(&info).map_err(std::io::Error::other)?;
    fs::write(path, json)
}

pub fn read_socket_info(data_dir: &Path) -> std::io::Result<SocketInfo> {
    let path = socket_path(data_dir);
    let data = fs::read_to_string(path)?;
    serde_json::from_str(&data).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub fn remove_socket_info(data_dir: &Path) -> std::io::Result<()> {
    let path = socket_path(data_dir);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_token_creation_and_reading() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        assert!(read_token(path).is_err());

        let token1 = read_or_create_token(path).unwrap();
        assert_eq!(token1.len(), 64);

        let token2 = read_token(path).unwrap();
        assert_eq!(token1, token2);
    }

    #[test]
    fn test_socket_info_lifecycle() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        assert!(read_socket_info(path).is_err());

        write_socket_info(path, 12345, 9999).unwrap();
        let info = read_socket_info(path).unwrap();
        assert_eq!(info.port, 12345);
        assert_eq!(info.pid, 9999);

        remove_socket_info(path).unwrap();
        assert!(read_socket_info(path).is_err());
    }
}
