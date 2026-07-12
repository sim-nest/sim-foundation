//! Safe path helpers for config roots and per-library files.

use std::path::PathBuf;

use sim_kernel::Symbol;
use sim_table_core::is_legal_table_segment;

use crate::{ConfigError, ConfigResult};

/// Central and working config roots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigRoots {
    /// Central user config root, if one can be derived from the environment.
    pub home: Option<PathBuf>,
    /// Working config root.
    pub work: PathBuf,
}

impl ConfigRoots {
    /// Creates roots from explicit values without reading the filesystem.
    pub fn new(home: Option<PathBuf>, work: PathBuf) -> Self {
        Self { home, work }
    }

    /// Derives roots from process environment variables and a work root.
    pub fn from_env(work_root: PathBuf) -> Self {
        Self::from_env_values(
            work_root,
            std::env::var_os("SIM_CONFIG_HOME").map(PathBuf::from),
            std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            std::env::var_os("HOME").map(PathBuf::from),
        )
    }

    /// Derives roots from provided environment values.
    pub fn from_env_values(
        work_root: PathBuf,
        sim_config_home: Option<PathBuf>,
        xdg_config_home: Option<PathBuf>,
        home: Option<PathBuf>,
    ) -> Self {
        let home = sim_config_home
            .or_else(|| xdg_config_home.map(|root| root.join("sim")))
            .or_else(|| home.map(|root| root.join(".config").join("sim")));
        Self {
            home,
            work: work_root.join(".sim").join("config"),
        }
    }
}

/// Returns the safe relative path for one library's per-lib config file.
pub fn lib_config_path(lib: &Symbol) -> ConfigResult<PathBuf> {
    let mut path = PathBuf::from("libs");
    for segment in lib.as_qualified_str().split('/') {
        if !safe_segment(segment) {
            return Err(ConfigError::InvalidPathSegment {
                segment: segment.to_owned(),
            });
        }
        path.push(segment);
    }
    path.set_extension("toml");
    Ok(path)
}

fn safe_segment(segment: &str) -> bool {
    is_legal_table_segment(segment)
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lib_config_path_maps_lib_id_under_libs() {
        let path = lib_config_path(&Symbol::qualified("sim", "cookbook")).unwrap();

        assert_eq!(
            path,
            PathBuf::from("libs").join("sim").join("cookbook.toml")
        );
    }

    #[test]
    fn lib_config_path_rejects_unsafe_segments() {
        let error = lib_config_path(&Symbol::new("..")).unwrap_err();

        assert_eq!(
            error,
            ConfigError::InvalidPathSegment {
                segment: "..".to_owned()
            }
        );
    }

    #[test]
    fn roots_use_config_home_before_xdg_and_home() {
        let roots = ConfigRoots::from_env_values(
            PathBuf::from("/work"),
            Some(PathBuf::from("/sim-config")),
            Some(PathBuf::from("/xdg")),
            Some(PathBuf::from("/user-home/alice")),
        );

        assert_eq!(roots.home, Some(PathBuf::from("/sim-config")));
        assert_eq!(roots.work, PathBuf::from("/work/.sim/config"));
    }
}
