#[cfg(test)]
mod tests;

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(about = "Minimal Wayland image viewer")]
pub struct Cli {
    pub paths: Vec<PathBuf>,

    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,
}
