use std::collections::HashMap;

use blrs::{
    fetching::build_repository::BuildRepo,
    repos::read_repos,
    search::{query::VersionSearchQuery, searching::BInfoMatcher},
    BLRSConfig, LocalBuild,
};
use log::{error, info};

use crate::{
    errs::{error_writing, CommandError},
    resolving::get_choice_map,
};

pub fn remove_builds(
    cfg: &BLRSConfig,
    queries: Vec<VersionSearchQuery>,
    no_trash: bool,
) -> Result<(), CommandError> {
    std::fs::create_dir_all(&cfg.paths.library)
        .inspect_err(|e| error!("Failed to create library path: {:?}", e))
        .map_err(|e| error_writing(cfg.paths.library.clone(), e))?;

    let local_builds: Vec<_> = read_repos(cfg.repos.clone(), &cfg.paths, false)
        .map_err(|e| CommandError::IoError(crate::errs::IoErrorOrigin::ReadingRepos, e))?
        .into_iter()
        .filter_map(|r| match r {
            blrs::repos::RepoEntry::Registered(
                BuildRepo {
                    repo_id: _,
                    url: _,
                    nickname,
                    repo_type: _,
                },
                vec,
            )
            | blrs::repos::RepoEntry::Unknown(nickname, vec) => {
                let collect: Vec<blrs::LocalBuild> = vec
                    .into_iter()
                    .filter_map(|entry| match entry {
                        blrs::repos::BuildEntry::Installed(_, local_build) => Some(local_build),
                        _ => None,
                    })
                    .collect();
                (!collect.is_empty()).then_some((collect, nickname))
            }
            _ => None,
        })
        .flat_map(|v| v.0.into_iter().map(move |b| (b, v.1.clone())))
        .collect();

    let matcher = BInfoMatcher::new(&local_builds);

    let matched_builds: Vec<(LocalBuild, _)> = queries
        .into_iter()
        .flat_map(|query| matcher.find_all(&query))
        .cloned()
        .collect();

    let choice_map: HashMap<String, &LocalBuild> = get_choice_map(&matched_builds);

    println!["{:#?}", choice_map];

    let inquiry = inquire::MultiSelect::new(
        "Choose which builds you want to uninstall",
        choice_map.keys().cloned().collect(),
    );

    match inquiry.prompt() {
        Ok(v) => {
            let chosen_builds: Vec<_> = v
                .into_iter()
                .map(|choice| choice_map.get(&choice).unwrap())
                .collect();

            if !no_trash {
                chosen_builds
                    .into_iter()
                    .map(|build| {
                        info!["Trashing {}", build.folder.display()];
                        trash::delete(&build.folder)
                            .inspect(|_| info!["Success."])
                            .map_err(|e| {
                                error!["Failure. {}", e];
                                CommandError::TrashError(build.folder.clone(), e)
                            })
                    })
                    .collect::<Vec<_>>() // Generate all the results before checking if any failed
                    .into_iter()
                    .find(|r| r.is_err())
                    .unwrap_or(Ok(()))
            } else {
                chosen_builds
                    .into_iter()
                    .map(|build| {
                        info!["Deleting {}", build.folder.display()];
                        std::fs::remove_dir_all(&build.folder)
                            .inspect(|_| info!["Success."])
                            .map_err(|e| {
                                error!["Failure. {}", e];
                                CommandError::IoError(
                                    crate::errs::IoErrorOrigin::DeletingObject(
                                        build.folder.clone(),
                                    ),
                                    e,
                                )
                            })
                    })
                    .collect::<Vec<_>>() // Generate all the results before checking if any failed
                    .into_iter()
                    .find(|r| r.is_err())
                    .unwrap_or(Ok(()))
            }
        }
        Err(e) => {
            println!["{:?}", e];

            Ok(())
        }
    }
}
