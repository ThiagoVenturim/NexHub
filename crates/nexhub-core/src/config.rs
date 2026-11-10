//! Runtime configuration, filesystem paths, auth token, and environment variables.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocketInfo {
    pub port: u16,
    pub pid: u32,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("home directory is not available")]
    HomeDirUnavailable,

    #[error("io error at {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("random number generation failed: {0}")]
    Random(String),

    #[error("token file at {0:?} contains an invalid token")]
    InvalidToken(PathBuf),
}

/// Filesystem paths used by NexHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    /// Resolves the NexHub root directory, respecting `NEXHUB_HOME` if set.
    /// Creates the directory if it does not exist.
    pub fn resolve() -> Result<Self, ConfigError> {
        let root = if let Ok(custom) = std::env::var("NEXHUB_HOME") {
            let trimmed = custom.trim();
            if !trimmed.is_empty() {
                PathBuf::from(trimmed)
            } else {
                Self::default_root()?
            }
        } else {
            Self::default_root()?
        };

        std::fs::create_dir_all(&root).map_err(|e| ConfigError::Io {
            path: root.clone(),
            source: e,
        })?;

        Ok(Self { root })
    }

    /// Creates a `Paths` instance with an explicit root directory (creating it if needed).
    pub fn from_root(root: impl Into<PathBuf>) -> Result<Self, ConfigError> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(|e| ConfigError::Io {
            path: root.clone(),
            source: e,
        })?;
        Ok(Self { root })
    }

    fn default_root() -> Result<PathBuf, ConfigError> {
        let base = directories::BaseDirs::new().ok_or(ConfigError::HomeDirUnavailable)?;
        #[cfg(target_os = "linux")]
        let path = base.data_dir().join("nexhub");
        #[cfg(not(target_os = "linux"))]
        let path = base.data_dir().join("NexHub");
        Ok(path)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn db_path(&self) -> PathBuf {
        self.root.join("nexhub.db")
    }

    pub fn token_path(&self) -> PathBuf {
        self.root.join("token")
    }

    pub fn socket_path(&self) -> PathBuf {
        self.root.join("socket.json")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }
}

/// Authentication token for NexHub IPC communication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// Creates a Token wrapper from a raw hex string after validating length and characters.
    pub fn new(raw: impl Into<String>) -> Result<Self, ConfigError> {
        let s = raw.into();
        let trimmed = s.trim();
        if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            Ok(Self(trimmed.to_string()))
        } else {
            Err(ConfigError::InvalidToken(PathBuf::from("<raw>")))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Loads or creates the token file at the given path.
    /// If the file does not exist, generates 32 random bytes via `getrandom` and writes them as hex.
    /// On Unix, permissions are set to 0o600. On Windows, the file is created inside the user profile
    /// directory which is private by default ACLs.
    pub fn load_or_create(path: &Path) -> Result<Self, ConfigError> {
        if path.exists() {
            let content = std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;
            let trimmed = content.trim();
            if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
                return Ok(Self(trimmed.to_string()));
            }
            return Err(ConfigError::InvalidToken(path.to_path_buf()));
        }

        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| ConfigError::Random(e.to_string()))?;
        let token_hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConfigError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .map_err(|e| ConfigError::Io {
                    path: path.to_path_buf(),
                    source: e,
                })?;
            file.write_all(token_hex.as_bytes())
                .map_err(|e| ConfigError::Io {
                    path: path.to_path_buf(),
                    source: e,
                })?;
        }

        #[cfg(not(unix))]
        {
            std::fs::write(path, &token_hex).map_err(|e| ConfigError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;
        }

        Ok(Self(token_hex))
    }

    /// Loads or creates the token using the default `Paths::resolve()`.
    pub fn load_or_create_default() -> Result<Self, ConfigError> {
        let paths = Paths::resolve()?;
        Self::load_or_create(&paths.token_path())
    }
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
    fn test_paths_resolve_honors_nexhub_home() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().join("custom_nexhub_home");
        std::env::set_var("NEXHUB_HOME", target_dir.to_str().unwrap());

        let paths = Paths::resolve().unwrap();
        assert_eq!(paths.root(), target_dir.as_path());
        assert!(target_dir.exists());
        assert_eq!(paths.db_path(), target_dir.join("nexhub.db"));
        assert_eq!(paths.token_path(), target_dir.join("token"));
        assert_eq!(paths.socket_path(), target_dir.join("socket.json"));
        assert_eq!(paths.logs_dir(), target_dir.join("logs"));
    }

    #[test]
    fn test_token_load_or_create_lifecycle() {
        let temp_dir = tempdir().unwrap();
        let token_path = temp_dir.path().join("token");

        let token1 = Token::load_or_create(&token_path).unwrap();
        assert_eq!(token1.as_str().len(), 64);
        assert!(token1.as_str().chars().all(|c| c.is_ascii_hexdigit()));
        assert!(token_path.exists());

        let token2 = Token::load_or_create(&token_path).unwrap();
        assert_eq!(token1, token2);

        std::fs::write(&token_path, "invalid-token-content").unwrap();
        let err = Token::load_or_create(&token_path);
        assert!(matches!(err, Err(ConfigError::InvalidToken(_))));
    }

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
