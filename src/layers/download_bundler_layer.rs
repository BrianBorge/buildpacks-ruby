use crate::{util, RubyBuildpackError};
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use serde::{Deserialize, Serialize};

use std::path::Path;
use std::process::Command;

use crate::gemfile_lock::BundlerVersion;
use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::Env;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DownloadBundlerLayerMetadata {
    version: String,
}

// Installs an executable version of Bundler for the customer based on the
// passed in version value. To the location set by BUNDLE_PATH
pub struct DownloadBundlerLayer {
    pub version: BundlerVersion,
    pub env: Env,
}

impl DownloadBundlerLayer {
    fn version_string(&self) -> String {
        match &self.version {
            BundlerVersion::Explicit(v) => v.clone(),
            BundlerVersion::Default => String::from("2.3.7"),
        }
    }
}

impl Layer for DownloadBundlerLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = DownloadBundlerLayerMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: true,
        }
    }

    fn update(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        let metadata = &layer_data.content_metadata.metadata;
        let old_value = metadata.version.clone();

        let gem_path = &self
            .env
            .get("GEM_PATH")
            .expect("Internal buildpack error: GEM_PATH must be set");

        println!(
            "---> New bundler version detected {}, uninstalling the old version {}",
            self.version_string(),
            old_value
        );

        util::run_simple_command(
            Command::new("gem")
                .args(&[
                    "uninstall",
                    "bundler",
                    "--force",
                    "-v",
                    &old_value.to_string(),
                    "--install-dir",
                    gem_path.to_str().unwrap(),
                ])
                .envs(&self.env),
            RubyBuildpackError::GemInstallBundlerCommandError,
            RubyBuildpackError::GemInstallBundlerUnexpectedExitStatus,
        )?;

        self.create(context, &layer_data.path)
    }

    fn create(
        &self,
        context: &BuildContext<Self::Buildpack>,
        _layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        println!("---> Installing bundler {}", self.version_string());

        let gem_path = &self
            .env
            .get("GEM_PATH")
            .expect("Internal buildpack error: GEM_PATH must be set");

        util::run_simple_command(
            Command::new("gem")
                .args(&[
                    "install",
                    "bundler",
                    "--force",
                    "--no-document",
                    "-v",
                    &self.version_string(),
                    "--install-dir",
                    gem_path.to_str().unwrap(),
                ])
                .envs(&self.env),
            RubyBuildpackError::GemInstallBundlerCommandError,
            RubyBuildpackError::GemInstallBundlerUnexpectedExitStatus,
        )?;

        LayerResultBuilder::new(DownloadBundlerLayerMetadata {
            version: self.version_string(),
        })
        .env(
            LayerEnv::new()
                .chainable_insert(
                    Scope::Build,
                    ModificationBehavior::Delimiter,
                    "BUNDLE_WITHOUT",
                    ":",
                )
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Prepend,
                    "BUNDLE_WITHOUT",
                    "development:test",
                )
                .chainable_insert(
                    Scope::Build,
                    ModificationBehavior::Override,
                    "BUNDLE_GEMFILE",
                    context.app_dir.join("Gemfile").clone(),
                )
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "BUNDLE_CLEAN",
                    "1",
                )
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "BUNDLE_DEPLOYMENT",
                    "1",
                )
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "BUNDLE_GLOBAL_PATH_APPENDS_RUBY_SCOPE",
                    "1",
                )
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "NOKOGIRI_USE_SYSTEM_LIBRARIES",
                    "1",
                ),
        )
        .build()
    }

    fn existing_layer_strategy(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        layer: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        if self.version_string() == layer.content_metadata.metadata.version {
            println!("---> Bundler {} already installed", self.version_string());
            Ok(ExistingLayerStrategy::Keep)
        } else {
            Ok(ExistingLayerStrategy::Update)
        }
    }
}