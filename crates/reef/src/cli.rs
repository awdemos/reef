use std::path::PathBuf;

use clap::{Parser, Subcommand};
use reef_engine::{ModelPuller, ModelSource, CapabilityRegistry};
use reef_engine::backends::embedded_llama::LlamaBackend;
use reef_storage::MemoryBackend;
use tracing::{info, error};

/// Reef: The Rust control plane for CowabungaAI.
#[derive(Debug, Parser)]
#[command(name = "reef")]
#[command(about = "AI control plane with embedded inference", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Path to config file
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Enable debug logging
    #[arg(long, global = true)]
    pub debug: bool,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start the API server and run worker
    Serve {
        /// Bind address
        #[arg(short, long, default_value = "0.0.0.0:8080")]
        bind: String,

        /// Storage backend: memory, postgres, turso
        #[arg(short, long, default_value = "memory")]
        storage: String,

        /// Enable development routes
        #[arg(long)]
        dev: bool,
    },

    /// Start only the background run worker
    Worker {
        /// Storage backend
        #[arg(short, long, default_value = "memory")]
        storage: String,
    },

    /// Pull a model from HuggingFace or NGC
    Pull {
        /// Model identifier: hf:org/repo/file or ngc:org/team/model/version/file
        model: String,

        /// HuggingFace auth token
        #[arg(long, env = "HF_TOKEN")]
        hf_token: Option<String>,

        /// NGC API key
        #[arg(long, env = "NGC_API_KEY")]
        ngc_key: Option<String>,

        /// Cache directory
        #[arg(short, long, default_value = "~/.cache/reef/models")]
        cache: PathBuf,
    },

    /// List cached models
    List {
        /// Cache directory
        #[arg(short, long, default_value = "~/.cache/reef/models")]
        cache: PathBuf,
    },

    /// Run a model interactively (embedded inference)
    Run {
        /// Path to model file or cached model name
        model: String,

        /// System prompt
        #[arg(short, long)]
        system: Option<String>,
    },

    /// Run database migrations
    Migrate {
        /// Storage backend: postgres, turso
        #[arg(short, long, default_value = "postgres")]
        storage: String,
    },

    /// Health check
    Health {
        /// API server URL
        #[arg(short, long, default_value = "http://localhost:8080")]
        url: String,
    },
}

impl Cli {
    pub async fn execute(self) -> anyhow::Result<()> {
        match self.command {
            Commands::Serve { bind, storage, dev } => {
                info!(%bind, %storage, dev, "starting reef server");
                crate::server::serve(bind, storage, dev).await?;
            }

            Commands::Worker { storage } => {
                info!(%storage, "starting reef worker");
                crate::worker::start(storage).await?;
            }

            Commands::Pull { model, hf_token, ngc_key, cache } => {
                let cache = expand_tilde(cache)?;
                tokio::fs::create_dir_all(&cache).await?;

                let puller = ModelPuller::new(cache)
                    .with_hf_token(hf_token.unwrap_or_default())
                    .with_ngc_key(ngc_key.unwrap_or_default());

                if model.starts_with("hf:") {
                    let parts: Vec<&str> = model[3..].split('/').collect();
                    if parts.len() < 3 {
                        anyhow::bail!("hf: format is hf:org/repo/filename");
                    }
                    let repo_id = parts[..parts.len() - 1].join("/");
                    let filename = parts[parts.len() - 1];
                    let manifest = puller.pull_hf(&repo_id, filename).await?;
                    println!("Pulled: {}", serde_json::to_string_pretty(&manifest)?);
                } else if model.starts_with("ngc:") {
                    let parts: Vec<&str> = model[4..].split('/').collect();
                    if parts.len() != 5 {
                        anyhow::bail!("ngc: format is ngc:org/team/model/version/filename");
                    }
                    let manifest = puller
                        .pull_ngc(parts[0], parts[1], parts[2], parts[3], parts[4])
                        .await?;
                    println!("Pulled: {}", serde_json::to_string_pretty(&manifest)?);
                } else {
                    anyhow::bail!("model must start with hf: or ngc:");
                }
            }

            Commands::List { cache } => {
                let cache = expand_tilde(cache)?;
                let puller = ModelPuller::new(cache);
                let models = puller.list_cached().await?;
                if models.is_empty() {
                    println!("No cached models found.");
                } else {
                    for m in models {
                        println!("{:?}  {:?}  {}  {}", m.source, m.format, m.size_bytes, m.local_path.display());
                    }
                }
            }

            Commands::Run { model, system } => {
                let model_path = expand_tilde(PathBuf::from(model))?;
                if !model_path.exists() {
                    anyhow::bail!("model not found: {}", model_path.display());
                }

                info!(path = %model_path.display(), "loading embedded model");
                let mut backend = LlamaBackend::new(
                    model_path.file_stem().unwrap_or_default().to_string_lossy().to_string(),
                    model_path,
                );
                backend.load().await?;

                println!("Model loaded. Type messages and press Enter. Use /quit to exit.");
                if let Some(sys) = system {
                    println!("System: {}", sys);
                }

                // Interactive REPL stub
                // In a real implementation, read stdin, build ChatCompletionRequest, stream tokens
            }

            Commands::Migrate { storage } => {
                info!(%storage, "running migrations");
                // TODO: dispatch to PostgresBackend::migrate() or TursoBackend::migrate()
            }

            Commands::Health { url } => {
                let client = reqwest::Client::new();
                let resp = client.get(format!("{}/health", url)).send().await?;
                println!("status: {}", resp.status());
            }
        }

        Ok(())
    }
}

fn expand_tilde(path: PathBuf) -> anyhow::Result<PathBuf> {
    let s = path.to_string_lossy();
    if s.starts_with("~/") {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| anyhow::anyhow!("could not determine home directory"))?;
        Ok(PathBuf::from(home).join(&s[2..]))
    } else {
        Ok(path)
    }
}
