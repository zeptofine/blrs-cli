use blrs::{
    build_targets::{filter_repos_by_target, get_target_setup},
    fetching::build_repository::BuildRepo,
    repos::{read_repos, BuildEntry, RepoEntry},
    BLRSConfig,
};
use clap::ValueEnum;
use log::{debug, error};
use serde::{Deserialize, Serialize};

use crate::{
    errs::{CommandError as CE, IoErrorOrigin},
    repo_formatting::{RepoEntryTreeConstructor, SortFormat},
};

#[derive(Debug, Clone, Copy, Default, ValueEnum, Serialize, Deserialize)]
pub enum LsFormat {
    /// A visual tree. Good for human interpretation, but not easily parsed.
    #[default]
    Tree,
    /// Shows filepaths of builds. Only shows installed.
    Paths,
    /// single-line JSON format.
    Json,
    /// Json but indented by 2 spaces to make it more human readable.
    PrettyJson,
}

fn gather_and_filter_repos(
    cfg: &BLRSConfig,
    installed_only: bool,
    all_builds: bool,
    sort_format: Option<SortFormat>,
) -> Result<Vec<RepoEntry>, std::io::Error> {
    let mut repos = read_repos(&cfg.repos, &cfg.paths, installed_only)?;
    debug!("Finished reading repos");
    repos = if all_builds {
        repos
    } else {
        let target = get_target_setup().unwrap();
        debug!["filtering list of builds by the target: {:?}", target];
        filter_repos_by_target(repos, Some(target))
    };

    if installed_only {
        repos.retain(RepoEntry::has_installed_builds);
    } else {
        repos.sort_by_cached_key(RepoEntry::has_installed_builds);
    }

    if let Some(sort_format) = sort_format {
        repos.iter_mut().for_each(|repo| match repo {
            RepoEntry::Registered(_, vec) | RepoEntry::Unknown(_, vec) => sort_format.sort(vec),
            RepoEntry::Error(_, _) => {}
        });
    }

    debug!["Successfully gathered repos."];

    Ok(repos)
}

pub fn list_builds(
    cfg: &BLRSConfig,
    ls_format: LsFormat,
    sort_format: SortFormat,
    installed_only: bool,
    show_variants: bool,
    all_builds: bool,
) -> Result<(), CE> {
    std::fs::create_dir_all(&cfg.paths.library)
        .inspect_err(|e| error!("Failed to create library path: {:?}", e))
        .map_err(CE::writing(cfg.paths.library.clone()))?;

    let mut all_repos = gather_and_filter_repos(cfg, installed_only, all_builds, Some(sort_format))
        .map_err(|e| CE::IoError(IoErrorOrigin::ReadingRepos, e))?;

    all_repos.sort_by_cached_key(|r| match r {
        RepoEntry::Registered(BuildRepo { nickname, .. }, _)
        | RepoEntry::Error(nickname, _)
        | RepoEntry::Unknown(nickname, _) => nickname.clone(),
    });

    match ls_format {
        LsFormat::Tree => all_repos.into_iter().for_each(|repo_entry| {
            let tree = RepoEntryTreeConstructor(&repo_entry).to_tree(show_variants);

            println!["{}", tree];
        }),
        LsFormat::Paths => {
            all_repos.into_iter().for_each(|repo| match repo {
                RepoEntry::Registered(_, vec) | RepoEntry::Unknown(_, vec) => {
                    for build in vec {
                        if let BuildEntry::Installed(_, local_build) = build {
                            println!["{}", local_build.folder.display()];
                        }
                    }
                }
                RepoEntry::Error(_, _) => {}
            });
        }
        LsFormat::Json => {
            println!["{}", serde_json::to_string(&all_repos).unwrap()];
        }
        LsFormat::PrettyJson => {
            println!["{}", serde_json::to_string_pretty(&all_repos).unwrap()];
        }
    }

    Ok(())
}
