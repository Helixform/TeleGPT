#[macro_use]
extern crate log;

use std::fs;

use anyhow::Error;
use clap::Parser;
use telegpt_core::{app, config::SharedConfig};

fn init_config(config_path: &str) -> Result<SharedConfig, Error> {
    let config_buf = fs::read(config_path)?;
    let config_json_str = String::from_utf8(config_buf)?;
    let config = serde_json::from_str(&config_json_str)?;
    Ok(SharedConfig::new(config))
}

#[derive(Parser)]
struct Args {
    #[arg(short = 'c', long = "config", default_value = "telegpt.config.json")]
    pub config_path: String,
}

#[tokio::main]
async fn main() {
    if std::env::var(env_logger::DEFAULT_FILTER_ENV).is_ok() {
        pretty_env_logger::init();
    } else {
        // No `RUST_LOG` environment variable found, use `Info` level as default.
        pretty_env_logger::formatted_timed_builder()
            .filter_level(log::LevelFilter::Info)
            .init();
    }

    let args = Args::parse();
    let config = match init_config(&args.config_path) {
        Ok(config) => config,
        Err(err) => {
            error!("Failed to load config: {}", err);
            return;
        }
    };

    app::run(config).await;

    info!("Bye");
}
