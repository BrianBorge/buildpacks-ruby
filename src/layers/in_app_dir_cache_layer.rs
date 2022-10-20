use crate::RubyBuildpackError;
use libcnb::data::layer_content_metadata::LayerTypes;
use std::path::Path;

use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};

use serde::{Deserialize, Serialize};

use std::path::PathBuf;

/*

# Caches a folder in the application directory

Layers are used for caching, however layers cannot be inside of the app directory.
This layer can be used to hold a directory's contents so they are preserved
between deploys.

The primary usecase of this is for caching assets. After `rake assets:precompile` runs
file in `<app-dir>/public/assets` need to be preserved between deploys. This allows
for faster deploys, and also allows for prior generated asssets to remain on the system
 until "cleaned." Historically, sprockets will keep 3 versions of old files on disk. This
 allows for emails that might have a long time to live to reference a specific SHA of an
 asset without.

*/

pub struct InAppDirCacheLayer {
    pub app_dir_path: PathBuf,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct InAppDirCacheLayerMetadata {
    app_dir_path: PathBuf,
}

impl Layer for InAppDirCacheLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = InAppDirCacheLayerMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: true,
        }
    }

    fn create(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        _layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        println!("---> Creating cache for {}", self.app_dir_path.display());

        LayerResultBuilder::new(InAppDirCacheLayerMetadata {
            app_dir_path: self.app_dir_path.clone(),
        })
        .build()
    }

    fn existing_layer_strategy(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        if self.app_dir_path == layer_data.content_metadata.metadata.app_dir_path {
            println!("---> Loading cache for {}", self.app_dir_path.display());

            Ok(ExistingLayerStrategy::Keep)
        } else {
            // prinln in inside of create()

            Ok(ExistingLayerStrategy::Recreate)
        }
    }
}