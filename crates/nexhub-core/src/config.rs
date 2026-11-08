//! Runtime configuration, filesystem paths, auth token, and environment variables.

use std::path::{Path, PathBuf};
use thiserror::Error;

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
            // On Windows, the file is written within the user's local AppData profile directory,
            // which is restricted by Windows default filesystem ACLs exclusively to the current user
            // and local administrators.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths_resolve_honors_nexhub_home() {
        let temp_dir = tempfile::tempdir().unwrap();
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
        let temp_dir = tempfile::tempdir().unwrap();
        let token_path = temp_dir.path().join("token");

        // First creation: creates file and produces 64-char hex
        let token1 = Token::load_or_create(&token_path).unwrap();
        assert_eq!(token1.as_str().len(), 64);
        assert!(token1.as_str().chars().all(|c| c.is_ascii_hexdigit()));
        assert!(token_path.exists());

        // Second call: loads exact same token
        let token2 = Token::load_or_create(&token_path).unwrap();
        assert_eq!(token1, token2);

        // Corrupted token returns error
        std::fs::write(&token_path, "invalid-token-content").unwrap();
        let err = Token::load_or_create(&token_path);
        assert!(matches!(err, Err(ConfigError::InvalidToken(_))));
    }
}
