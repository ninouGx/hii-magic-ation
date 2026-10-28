use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use serde::{Deserialize, Serialize};

pub const APP_NAME: &str = "hii-magic-ation";
const ENV_PREFIX: &str = "HMA_";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub data_dir: PathBuf,
    pub bind: SocketAddr,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(APP_NAME),
            bind: SocketAddr::from(([127, 0, 0, 1], 7878)),
        }
    }
}

impl Config {
    /// Precedence, lowest first: defaults, TOML file, `HMA_*` environment variables.
    pub fn load(file: Option<&Path>) -> Result<Self, Box<figment::Error>> {
        let file = file.map_or_else(default_config_file, Path::to_path_buf);
        Figment::from(Serialized::defaults(Self::default()))
            .merge(Toml::file(file))
            .merge(Env::prefixed(ENV_PREFIX))
            .extract()
            .map_err(Box::new)
    }
}

fn default_config_file() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME)
        .join("config.toml")
}

#[cfg(test)]
#[allow(
    clippy::result_large_err,
    reason = "figment::Jail closures must return figment::Error"
)]
mod tests {
    use super::*;

    #[test]
    fn file_overrides_defaults_and_env_overrides_file() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "config.toml",
                "bind = \"127.0.0.1:9000\"\ndata_dir = \"/from/file\"",
            )?;
            jail.set_env("HMA_DATA_DIR", "/from/env");

            let config = Config::load(Some(Path::new("config.toml"))).unwrap();

            assert_eq!(config.bind.port(), 9000);
            assert_eq!(config.data_dir, PathBuf::from("/from/env"));
            Ok(())
        });
    }

    #[test]
    fn missing_file_falls_back_to_defaults() {
        figment::Jail::expect_with(|_| {
            let config = Config::load(Some(Path::new("absent.toml"))).unwrap();

            assert_eq!(config.bind.port(), 7878);
            assert!(config.data_dir.ends_with(APP_NAME));
            Ok(())
        });
    }
}
