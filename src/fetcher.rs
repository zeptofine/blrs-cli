use std::path::PathBuf;

use async_std::io::WriteExt;
use blrs::{
    fetching::{
        build_repository::{fetch_repo, FetchError},
        build_schemas::builder_schema::BlenderBuildSchema,
    },
    BLRSConfig,
};
use chrono::Utc;
use log::{debug, error, info};

/// Fetches from the builder's repo. If Ok(()) is returned, make sure to update the last time checked in the config.
pub async fn fetch(cfg: &mut BLRSConfig) -> Vec<Result<bool, std::io::Error>> {
    let repos_folder = &cfg.paths.remote_repos.clone();
    // Ensure the repos folder exists
    std::fs::create_dir_all(repos_folder);

    let mut results = Vec::with_capacity(cfg.repos.len());
    for repo in &cfg.repos.clone() {
        let url = repo.url();
        let client = cfg
            .client_builder(url.domain().is_some_and(|h| h.contains("api.github.com")))
            .build()
            .unwrap();

        let r = fetch_repo(client, repo.clone()).await;

        let filename = repos_folder.join(repo.repo_id.clone() + ".json");

        info!["Fetching from {}", url];

        results.push(_process_result(filename, r).await);
    }

    if results.iter().any(|r| r.as_ref().is_ok_and(|b| *b)) {
        // Update the last time checked in the config
        let now = Utc::now();
        cfg.last_time_checked = Some(now);
    }

    results
}

async fn _process_result(
    filename: PathBuf,
    r: Result<Vec<BlenderBuildSchema>, FetchError>,
) -> Result<bool, std::io::Error> {
    match r {
        Ok(builds) => {
            info!["Successfully downloaded builds"];

            debug!["Saving builds to database..."];

            {
                let mut file = async_std::fs::File::create(&filename).await?;

                let data = serde_json::to_string(&builds).unwrap();
                file.write_all(data.as_bytes()).await?;
                info!["Saved cache to {}", filename.to_str().unwrap()];
            }

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
