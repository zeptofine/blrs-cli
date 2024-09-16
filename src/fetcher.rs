use async_std::io::WriteExt;
use blrs::{
    fetching::from_builder::{default_url, fetch_builds_from_builder, FetchError},
    BLRSConfig,
};
use chrono::Utc;
use log::{debug, error, info};
use reqwest::Url;

/// Fetches from the builder's repo. Returns whether BLRS should be saved.
pub async fn fetch(cfg: &mut BLRSConfig, url: Option<Url>) -> Result<bool, std::io::Error> {
    let client = cfg.client_builder().build().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "This client is not supported",
        )
    })?;
    let url = url.unwrap_or(default_url());

    debug!["Using client {:?}", client];
    info!["Sending request to \"{}\"...", url];

    let r = fetch_builds_from_builder(client, url.clone()).await;

    match r {
        Ok(builds) => {
            info!["Successfully downloaded builds"];

            debug!["Saving builds to database..."];
            let repos_folder = &cfg.paths.remote_repos;
            // Ensure the repos folder exists
            std::fs::create_dir_all(repos_folder)?;
            {
                let filepath = repos_folder.join(url.domain().unwrap().to_string() + ".json");
                let mut file = async_std::fs::File::create(&filepath).await?;

                let data = serde_json::to_string(&builds).unwrap();
                file.write_all(data.as_bytes()).await?;
                info!["Saved cache to {}", filepath.to_str().unwrap()];
            }

            // Update the last time checked in the config
            let now = Utc::now();
            cfg.last_time_checked = Some(now);

            Ok(true)
        }
        Err(e) => {
            error!["Failed fetching from builder: {:?}", e];

            match e {
                FetchError::IoError(error) => Err(error),
                e => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!["Error: {e:?}"],
                )),
            }
        }
    }
}
