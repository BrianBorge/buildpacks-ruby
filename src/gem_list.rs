use core::str::FromStr;
use regex::Regex;
use std::collections::HashMap;

use crate::env_command::{EnvCommand, EnvCommandError};
use crate::gem_version::GemVersion;
use libcnb::Env;

// ## Gets list of an application's dependencies
//
// Requires `ruby` and `bundle` to be installed and on the PATH
//
#[derive(Debug)]
pub struct GemList {
    pub gems: HashMap<String, GemVersion>,
}

#[derive(thiserror::Error, Debug)]
pub enum GemListError {
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),

    #[error("Error determining dependencies: {0}")]
    BundleListShellCommandError(EnvCommandError),
}

impl GemList {
    // Calls `bundle list` and returns a `GemList` struct
    pub fn from_bundle_list(env: &Env) -> Result<Self, GemListError> {
        let output = EnvCommand::new("bundle", &["list"], env)
            .call()
            .map_err(GemListError::BundleListShellCommandError)?;

        GemList::from_str(&output.stdout)
    }

    #[allow(dead_code)]
    pub fn has(&self, str: &str) -> bool {
        self.gems.get(&str.trim().to_lowercase()).is_some()
    }

    #[allow(dead_code)]
    pub fn version_for(&self, str: &str) -> Option<&GemVersion> {
        self.gems.get(&str.trim().to_lowercase())
    }
}

impl FromStr for GemList {
    type Err = GemListError;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        // https://regex101.com/r/EIJe5G/1
        let gem_entry_re =
            Regex::new("  \\* (\\S+) \\(([a-zA-Z0-9\\.]+)\\)").map_err(GemListError::RegexError)?;

        let gems = gem_entry_re
            .captures_iter(string)
            .map(
                |capture| {
                    let name = match capture.get(1) {
                        Some(m) => m.as_str(),
                        None => "",
                    };

                    let version = match capture.get(2) {
                        Some(m) => m.as_str(),
                        None => "0.0.0",
                    };
                    (
                        name.to_string().to_lowercase(),
                        GemVersion::from_str(version).unwrap_or_default(),
                    )
                }, //
            )
            .collect::<HashMap<String, GemVersion>>();

        Ok(GemList { gems })
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn test_parsing_gem_list() {
        let gem_list = GemList::from_str(
            r#"
Gems included by the bundle:
  * actioncable (6.1.4.1)
  * actionmailbox (6.1.4.1)
  * actionmailer (6.1.4.1)
  * actionpack (6.1.4.1)
  * actiontext (6.1.4.1)
  * actionview (6.1.4.1)
  * activejob (6.1.4.1)
  * activemodel (6.1.4.1)
  * activerecord (6.1.4.1)
  * activestorage (6.1.4.1)
  * activesupport (6.1.4.1)
  * addressable (2.8.0)
  * ast (2.4.2)
  * railties (6.1.4.1)
Use `bundle info` to print more detailed information about a gem
            "#,
        )
        .unwrap();

        assert!(gem_list.has("railties"));
        assert!(!gem_list.has("foo"));

        assert_eq!(
            gem_list.version_for("railties").unwrap(),
            &GemVersion::from_str("6.1.4.1").unwrap()
        );
        assert_eq!(gem_list.version_for("foo"), None);

        assert_eq!(gem_list.gems.len(), 14);
    }
}
