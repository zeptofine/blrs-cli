use std::path::PathBuf;

use async_std::io::WriteExt;
use blrs::{
    fetching::{
        build_repository::{fetch_repo, FetchError},
        build_schemas::BlenderBuildSchema,
    },
    BLRSConfig,
};
use futures::future::{join_all, try_join_all};
use log::{debug, error, info};

use crate::tasks::ConfigTask;

/// Fetches from the builder's repo
pub async fn fetch(
    cfg: &BLRSConfig,
    parallel: bool,
    ignore_errors: bool,
) -> Result<ConfigTask, std::io::Error> {
    let repos_folder = &cfg.paths.remote_repos.clone();
    // Ensure the repos folder exists
    let _ = std::fs::create_dir_all(repos_folder);

    let actions = cfg
        .repos
        .iter()
        .map(|repo| async {
            let url = repo.url();
            let client = cfg.client_builder().build().unwrap();

            info!["Fetching from {}", url];
            let r = fetch_repo(client, repo.clone()).await;

            let filename = repos_folder.join(repo.repo_id.clone() + ".json");

            process_result_(filename, r).await
        })
        .collect::<Vec<_>>();

    let mut result = Ok(ConfigTask::UpdateLastTimeChecked);
    if parallel {
        if ignore_errors {
            join_all(actions.into_iter())
                .await
                .into_iter()
                .map(|r| match r {
                    Ok(_) => Ok(ConfigTask::UpdateLastTimeChecked),
                    Err(e) => Err(e),
                })
                .find(Result::is_err)
                .unwrap_or(result)
        } else {
            try_join_all(actions.into_iter())
                .await
                .map(|_| ConfigTask::UpdateLastTimeChecked)
        }
    } else {
        for action in actions {
            let r = action.await.map(|_| ConfigTask::UpdateLastTimeChecked);

            if r.is_err() {
                result = r;

                if ignore_errors {
                    break;
                }
            }
        }

        result
    }
}

async fn process_result_(
    filename: PathBuf,
    r: Result<Vec<BlenderBuildSchema>, FetchError>,
) -> Result<(), std::io::Error> {
    match r {
        Ok(builds) => {
            info!["Successfully downloaded build lists"];

            debug!["Saving builds to database..."];

            {
                let mut file = async_std::fs::File::create(&filename).await?;

                let data = serde_json::to_string(&builds).unwrap();
                file.write_all(data.as_bytes()).await?;
                info!["Saved cache to {}", filename.to_str().unwrap()];
            }

            Ok(())
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
