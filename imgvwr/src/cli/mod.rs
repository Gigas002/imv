#[cfg(test)]
mod tests;

use std::path::PathBuf;

use clap::Parser;

use crate::config::FilterMethod;

#[derive(Parser, Debug)]
#[command(about = "Minimal Wayland image viewer")]
pub struct Cli {
    pub paths: Vec<PathBuf>,

    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[arg(short = 'd', long, num_args = 0..=1, default_missing_value = "true")]
    pub decorations: Option<bool>,

    #[arg(short = 'a', long, num_args = 0..=1, default_missing_value = "true")]
    pub antialiasing: Option<bool>,

    #[arg(long)]
    pub min_scale: Option<f32>,

    #[arg(long)]
    pub max_scale: Option<f32>,

    #[arg(long)]
    pub scale_step: Option<f32>,

    #[arg(long)]
    pub filter_method: Option<FilterMethod>,

    #[arg(long)]
    pub log_level: Option<String>,
}
