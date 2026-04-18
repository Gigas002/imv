#[cfg(test)]
mod tests;

use crate::config::Config;

#[cfg(feature = "logging")]
pub(crate) fn init(config: &Config) {
    use tracing_subscriber::EnvFilter;

    let config_level = config
        .logging
        .as_ref()
        .and_then(|l| l.level.as_deref())
        .unwrap_or("warn");

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(config_level));

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[cfg(not(feature = "logging"))]
pub(crate) fn init(_config: &Config) {}
