use semver::Version;
use serde::Deserialize;
use std::time::Duration;

const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");
const CHECK_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateResult {
    UpdateAvailable(String),
    UpToDate,
}

#[derive(Debug, Deserialize)]
struct Response {
    #[serde(rename = "crate")]
    crate_data: CrateData,
}

#[derive(Debug, Deserialize)]
struct CrateData {
    max_stable_version: String,
}

pub async fn check_for_update() -> Option<UpdateResult> {
    let installed = Version::parse(CRATE_VERSION).ok()?;
    let client = reqwest::Client::builder()
        .timeout(CHECK_TIMEOUT)
        .user_agent(format!("{CRATE_NAME}/{CRATE_VERSION}"))
        .build()
        .ok()?;

    let response = client
        .get(format!("https://crates.io/api/v1/crates/{CRATE_NAME}"))
        .send()
        .await
        .ok()?;

    let payload = response.json::<Response>().await.ok()?;
    let latest = Version::parse(payload.crate_data.max_stable_version.as_str()).ok()?;

    if latest > installed {
        Some(UpdateResult::UpdateAvailable(latest.to_string()))
    } else {
        Some(UpdateResult::UpToDate)
    }
}
