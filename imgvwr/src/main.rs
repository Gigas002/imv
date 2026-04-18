mod app;
mod cli;
mod config;
mod logger;
mod settings;

use clap::Parser;
use config::Config;
use settings::AppSettings;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse();
    let config = Config::load_merged(cli.config.as_deref());
    logger::init(&config);
    let settings = AppSettings::resolve(&cli, &config);
    app::run(settings)
}
