use std::time::Duration;

/// The limit covers the whole request, from connecting to the last byte read.
pub(crate) fn agent(limit_secs: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(limit_secs)))
        .build()
        .into()
}

pub(crate) fn failed(e: ureq::Error) -> crate::base::error::Error {
    crate::base::error::Error::network(crate::said!("the download failed: {}", e))
}

pub(crate) fn steam_failed(e: impl std::fmt::Display) -> crate::base::error::Error {
    crate::base::error::Error::network(crate::said!("Steam could not be asked: {}", e))
}
