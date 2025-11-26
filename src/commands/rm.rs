use std::collections::HashMap;

use blrs::{
    fetching::build_repository::BuildRepo,
    repos::read_repos,
    search::{BInfoMatcher, VersionSearchQuery},
    BLRSConfig, LocalBuild,
};
use log::{error, info};

use crate::{errs::CommandError as CE, resolving::get_choice_map};

pub fn remove_builds(
    cfg: &BLRSConfig,
    queries: Vec<VersionSearchQuery>,
    no_trash: bool,
) -> Result<(), CE> {
    std::fs::create_dir_all(&cfg.paths.library)
        .inspect_err(|e| error!("Failed to create library path: {:?}", e))
        .map_err(CE::writing(&cfg.paths.library))?;

    let repos = read_repos(&cfg.repos, &cfg.paths, false)
        .map_err(|e| CE::IoError(crate::errs::IoErrorOrigin::ReadingRepos, e))?;

    let local_builds: Vec<_> = repos
        .iter()
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
                let collect: Vec<&blrs::LocalBuild> = vec
                    .iter()
                    .filter_map(|entry| match entry {
                        blrs::repos::BuildEntry::Installed(_, local_build) => Some(local_build),
                        _ => None,
                    })
                    .collect();
                (!collect.is_empty()).then_some((collect, nickname))
            }
            blrs::repos::RepoEntry::Error(_, _) => None,
        })
        .flat_map(|v| v.0.into_iter().map(move |b| (b, v.1.as_str())))
        .collect();

    let matched_builds: Vec<(&LocalBuild, _)> = {
        let matcher = BInfoMatcher::new(&local_builds);
        queries
            .into_iter()
            .flat_map(|query| matcher.find_all(&query))
            .map(|x| (x.0, x.1.to_string()))
            .collect()
    };

    let choice_map: HashMap<String, _> = get_choice_map(&matched_builds);

    println!["{:#?}", choice_map];

    let mut choices: Vec<_> = choice_map.keys().collect();
    choices.sort_by_cached_key(|b| &choice_map[*b].info.basic);

    let inquiry = inquire::MultiSelect::new("Choose which builds you want to uninstall", choices);

    match inquiry.prompt() {
        Ok(v) => {
            let chosen_builds: Vec<_> = v
                .into_iter()
                .map(|choice| choice_map.get(choice).unwrap())
                .collect();

            if no_trash {
                chosen_builds
                    .into_iter()
                    .map(|build| {
                        info!["Deleting {}", build.folder.display()];
                        std::fs::remove_dir_all(&build.folder)
                            .inspect(|()| info!["Success."])
                            .map_err(|e| {
                                error!["Failure. {}", e];
                                CE::IoError(
                                    crate::errs::IoErrorOrigin::DeletingObject(
                                        build.folder.clone(),
                                    ),
                                    e,
                                )
                            })
                    })
                    .collect::<Vec<_>>() // Generate all the results before checking if any failed
                    .into_iter()
                    .find(Result::is_err)
                    .unwrap_or(Ok(()))
            } else {
                let tctx = trash::TrashContext::new();
                chosen_builds
                    .into_iter()
                    .map(|build| {
                        info!["Trashing {}", build.folder.display()];
                        tctx.delete(&build.folder)
                            .inspect(|_| info!["Success."])
                            .map_err(|e| {
                                error!["Failure. {}", e];
                                CE::TrashError(build.folder.clone(), e)
                            })
                    })
                    .collect::<Vec<_>>() // Generate all the results before checking if any failed
                    .into_iter()
                    .find(Result::is_err)
                    .unwrap_or(Ok(()))
            }
        }
        Err(e) => {
            println!["{:?}", e];
            Err(CE::NotEnoughInput)
        }
    }
}
