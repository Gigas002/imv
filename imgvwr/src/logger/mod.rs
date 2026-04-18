#[cfg(test)]
mod tests;

#[cfg(feature = "logging")]
pub(crate) fn init(level: &str) {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[cfg(not(feature = "logging"))]
pub(crate) fn init(_level: &str) {}
