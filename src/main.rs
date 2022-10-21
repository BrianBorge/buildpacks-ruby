#![warn(unused_crate_dependencies)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use crate::layers::{
    BundleInstallConfigureEnvLayer, BundleInstallCreatePathLayer,
    BundleInstallDownloadBundlerLayer, BundleInstallExecuteLayer, EnvDefaultsSetSecretKeyBaseLayer,
    EnvDefaultsSetStaticVarsLayer, InAppDirCacheLayer, RubyVersionInstallLayer,
};
use crate::lib::gemfile_lock::{GemfileLock, GemfileLockError, RubyVersion};
// use heroku_ruby_buildpack as _;

// Move eventually
use crate::lib::gem_list::GemListError;
use crate::lib::rake_detect::RakeDetectError;

use crate::steps::rake_assets_precompile_execute::RakeApplicationTasksExecute;

use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::data::launch::{LaunchBuilder, ProcessBuilder};
use libcnb::data::{layer_name, process_type};
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::{GenericMetadata, GenericPlatform};
use libcnb::layer_env::Scope;
use libcnb::Platform;
use libcnb::{buildpack_main, Buildpack};

use crate::lib::env_command::EnvCommandError;
#[cfg(test)]
use libcnb_test as _;

use core::str::FromStr;

use crate::util::{DownloadError, UntarError, UrlError};
use std::process::ExitStatus;

mod layers;
mod lib;
mod steps;

#[cfg(test)]
mod test_helper;
mod util;

use libcnb::data::build_plan::BuildPlanBuilder;
use libcnb::Env;

pub struct RubyBuildpack;
impl Buildpack for RubyBuildpack {
    type Platform = GenericPlatform;
    type Metadata = GenericMetadata;
    type Error = RubyBuildpackError;

    fn detect(&self, context: DetectContext<Self>) -> libcnb::Result<DetectResult, Self::Error> {
        let mut plan_builder = BuildPlanBuilder::new().provides("ruby");

        if context.app_dir.join("Gemfile.lock").exists() {
            plan_builder = plan_builder.requires("ruby");

            if context.app_dir.join("package.json").exists() {
                plan_builder = plan_builder.requires("node");
            }
        }

        DetectResultBuilder::pass()
            .build_plan(plan_builder.build())
            .build()
    }

    fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
        println!("---> Ruby Buildpack");

        // Get system env vars
        let mut env = Env::from_current();

        // Apply User env vars
        // TODO reject harmful vars like GEM_PATH
        for (k, v) in context.platform.env() {
            env.insert(k, v);
        }

        // Gather static information about project
        let gemfile_lock = std::fs::read_to_string(context.app_dir.join("Gemfile.lock")).unwrap();
        let bundle_info = GemfileLock::from_str(&gemfile_lock)
            .map_err(RubyBuildpackError::GemfileLockParsingError)?;

        // Setup default environment variables

        let secret_key_base_layer = context //
            .handle_layer(
                layer_name!("secret_key_base"),
                EnvDefaultsSetSecretKeyBaseLayer,
            )?;
        env = secret_key_base_layer.env.apply(Scope::Build, &env);

        let env_defaults_layer = context //
            .handle_layer(
                layer_name!("env_defaults"),
                EnvDefaultsSetStaticVarsLayer,
            )?;
        env = env_defaults_layer.env.apply(Scope::Build, &env);

        // ## Install executable ruby version
        let ruby_layer = context //
            .handle_layer(
                layer_name!("ruby"),
                RubyVersionInstallLayer {
                    version: bundle_info.ruby_version,
                },
            )?;

        env = ruby_layer.env.apply(Scope::Build, &env);

        // ## Setup bundler
        let create_bundle_path_layer = context.handle_layer(
            layer_name!("gems"),
            BundleInstallCreatePathLayer {
                ruby_version: ruby_layer.content_metadata.metadata.version,
            },
        )?;
        env = create_bundle_path_layer.env.apply(Scope::Build, &env);

        let create_bundle_path_layer = context.handle_layer(
            layer_name!("bundle_configure_env"),
            BundleInstallConfigureEnvLayer,
        )?;
        env = create_bundle_path_layer.env.apply(Scope::Build, &env);

        // ## Download bundler
        let download_bundler_layer = context.handle_layer(
            layer_name!("bundler"),
            BundleInstallDownloadBundlerLayer {
                version: bundle_info.bundler_version,
                env: env.clone(),
            },
        )?;
        env = download_bundler_layer.env.apply(Scope::Build, &env);

        // ## bundle install
        let execute_bundle_install_layer = context.handle_layer(
            layer_name!("execute_bundle_install"),
            BundleInstallExecuteLayer { env: env.clone() },
        )?;
        env = execute_bundle_install_layer.env.apply(Scope::Build, &env);

        // Assets install
        RakeApplicationTasksExecute::call(&context, &env)?;

        BuildResultBuilder::new()
            .launch(
                LaunchBuilder::new()
                    .process(
                        ProcessBuilder::new(process_type!("web"), "bundle")
                            .args(["exec", "rackup", "--port", "$PORT", "--host", "0.0.0.0"])
                            .default(true)
                            .build(),
                    )
                    .build(),
            )
            .build()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RubyBuildpackError {
    #[error("Cannot download: {0}")]
    RubyDownloadError(DownloadError),
    #[error("Cannot untar: {0}")]
    RubyUntarError(UntarError),
    #[error("Cannot create temporary file: {0}")]
    CouldNotCreateTemporaryFile(std::io::Error),
    #[error("Cannot generate checksum: {0}")]
    CouldNotGenerateChecksum(std::io::Error),
    #[error("Bundler gem install exit: {0}")]
    GemInstallBundlerUnexpectedExitStatus(ExitStatus),
    #[error("Bundle install command errored: {0}")]
    BundleInstallCommandError(EnvCommandError),

    #[error("Could not install bundler: {0}")]
    GemInstallBundlerCommandError(EnvCommandError),

    #[error("Bundle install exit: {0}")]
    BundleInstallUnexpectedExitStatus(ExitStatus),
    #[error("Bundle config error: {0}")]
    BundleConfigCommandError(std::io::Error),
    #[error("Bundle config exit: {0}")]
    BundleConfigUnexpectedExitStatus(ExitStatus),

    #[error("Url error: {0}")]
    UrlParseError(UrlError),

    #[error("Error building list of gems for application: {0}")]
    GemListGetError(GemListError),

    #[error("Error detecting rake tasks: {0}")]
    RakeDetectError(RakeDetectError),

    #[error("Error evaluating Gemfile.lock: {0}")]
    GemfileLockParsingError(GemfileLockError),
}
impl From<RubyBuildpackError> for libcnb::Error<RubyBuildpackError> {
    fn from(error: RubyBuildpackError) -> Self {
        Self::BuildpackError(error)
    }
}

buildpack_main!(RubyBuildpack);
