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
    let settings = AppSettings::resolve(&cli, &config);
    logger::init(&settings.log_level);
    app::run(settings)
}
