use std::path::{Path, PathBuf};

use hf_hub::api::tokio::ApiBuilder;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info, warn};

/// Verified manifest for a pulled model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelManifest {
    pub source: ModelSource,
    pub repo_id: String,
    pub filename: String,
    pub local_path: PathBuf,
    pub size_bytes: u64,
    pub sha256: Option<String>,
    pub format: ModelFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSource {
    HuggingFace,
    Ngc,
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFormat {
    Gguf,
    Safetensors,
    Onnx,
    Unknown,
}

/// Downloads and caches models from HuggingFace Hub or NVIDIA NGC.
#[derive(Debug, Clone)]
pub struct ModelPuller {
    cache_dir: PathBuf,
    hf_token: Option<String>,
    ngc_api_key: Option<String>,
}

impl ModelPuller {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            hf_token: None,
            ngc_api_key: None,
        }
    }

    pub fn with_hf_token(mut self, token: String) -> Self {
        self.hf_token = Some(token);
        self
    }

    pub fn with_ngc_key(mut self, key: String) -> Self {
        self.ngc_api_key = Some(key);
        self
    }

    /// Pull a model from HuggingFace Hub.
    ///
    /// Example: `pull_hf("TheBloke/Llama-2-7B-GGUF", "llama-2-7b.Q4_K_M.gguf")`
    pub async fn pull_hf(&self, repo_id: &str, filename: &str) -> anyhow::Result<ModelManifest> {
        info!(repo = %repo_id, file = %filename, "pulling from HuggingFace Hub");

        let api = match &self.hf_token {
            Some(token) => ApiBuilder::new().with_token(Some(token.clone())).build()?,
            None => ApiBuilder::new().build()?,
        };

        let repo = hf_hub::Repo::new(repo_id.to_string(), hf_hub::RepoType::Model);
        let path = api.repo(repo).download(filename).await?;

        let metadata = tokio::fs::metadata(&path).await?;
        let format = detect_format(filename);

        let manifest = ModelManifest {
            source: ModelSource::HuggingFace,
            repo_id: repo_id.to_string(),
            filename: filename.to_string(),
            local_path: path,
            size_bytes: metadata.len(),
            sha256: None, // Could verify from .sha256 file if available
            format,
        };

        info!(path = %manifest.local_path.display(), size = manifest.size_bytes, "HF pull complete");
        Ok(manifest)
    }

    /// Pull a model from NVIDIA NGC.
    ///
    /// NGC models are accessed via:
    /// `https://api.ngc.nvidia.com/v2/models/{org}/{team}/{model_name}/versions/{version}/files/{filename}`
    pub async fn pull_ngc(
        &self,
        org: &str,
        team: &str,
        model_name: &str,
        version: &str,
        filename: &str,
    ) -> anyhow::Result<ModelManifest> {
        let api_key = self
            .ngc_api_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NGC API key required. Set NGC_API_KEY or use --ngc-key"))?;

        let url = format!(
            "https://api.ngc.nvidia.com/v2/models/{}/{}/{}/versions/{}/files/{}",
            org, team, model_name, version, filename
        );

        info!(url = %url, "pulling from NGC");

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("NGC download failed: {}", response.status());
        }

        let total_size = response.content_length();
        let progress = ProgressBar::new(total_size.unwrap_or(0));
        progress.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
                .progress_chars("#>-")
        );

        let dest_dir = self.cache_dir.join("ngc").join(org).join(team).join(model_name).join(version);
        tokio::fs::create_dir_all(&dest_dir).await?;
        let dest_path = dest_dir.join(filename);

        let mut file = tokio::fs::File::create(&dest_path).await?;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            progress.inc(chunk.len() as u64);
        }

        progress.finish_with_message("download complete");

        let metadata = tokio::fs::metadata(&dest_path).await?;
        let format = detect_format(filename);

        let manifest = ModelManifest {
            source: ModelSource::Ngc,
            repo_id: format!("{}/{}/{}", org, team, model_name),
            filename: filename.to_string(),
            local_path: dest_path,
            size_bytes: metadata.len(),
            sha256: None,
            format,
        };

        info!(path = %manifest.local_path.display(), size = manifest.size_bytes, "NGC pull complete");
        Ok(manifest)
    }

    /// List all models currently in the cache.
    pub async fn list_cached(&self) -> anyhow::Result<Vec<ModelManifest>> {
        let mut manifests = Vec::new();

        if !self.cache_dir.exists() {
            return Ok(manifests);
        }

        // In a real implementation, scan for manifest.json files in cache subdirectories
        // and deserialize them.

        Ok(manifests)
    }

    /// Delete a cached model by local path.
    pub async fn delete_cached(&self, path: &Path) -> anyhow::Result<()> {
        info!(path = %path.display(), "deleting cached model");
        tokio::fs::remove_file(path).await?;
        Ok(())
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

fn detect_format(filename: &str) -> ModelFormat {
    let lower = filename.to_lowercase();
    if lower.ends_with(".gguf") {
        ModelFormat::Gguf
    } else if lower.ends_with(".safetensors") {
        ModelFormat::Safetensors
    } else if lower.ends_with(".onnx") {
        ModelFormat::Onnx
    } else {
        ModelFormat::Unknown
    }
}

use futures::StreamExt;
