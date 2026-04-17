mod app;
mod cli;
mod config;
mod settings;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use config::Config;
use settings::AppSettings;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = cli::Cli::parse();
    let config = Config::load_merged(cli.config.as_deref());
    let settings = AppSettings::resolve(&cli, &config);
    app::run(settings)
}
