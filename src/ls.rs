use blrs::{
    downloading::extensions::{filter_repos_by_target, get_target_setup},
    repos::{read_repos, RepoEntry},
    BLRSConfig,
};
use clap::ValueEnum;
use log::{debug, error};
use serde::{Deserialize, Serialize};

use crate::repo_formatting::{RepoEntryTreeConstructor, SortFormat};

#[derive(Debug, Clone, Copy, Default, ValueEnum, Serialize, Deserialize)]
pub enum LsFormat {
    #[default]
    Tree,
    JustPaths,
    Json,
    /// Json but indented by 4 spaces to make it more human readable.
    PrettyJson,
    Toml,
}

fn gather_and_filter_repos(
    cfg: &BLRSConfig,
    installed_only: bool,
    all_builds: bool,
    sort_format: Option<SortFormat>,
) -> Result<Vec<RepoEntry>, std::io::Error> {
    let mut repos = read_repos(cfg.repos.clone(), &cfg.paths, installed_only)?;
    debug!("Finished reading repos");
    repos = if !all_builds {
        let target = get_target_setup().unwrap();
        debug!["filtering list of builds by the target: {:?}", target];
        filter_repos_by_target(repos, Some(target))
    } else {
        repos
    };

    if installed_only {
        repos.retain(|r| r.has_installed_builds())
    } else {
        repos.sort_by_key(|r| r.has_installed_builds());
    }

    if let Some(sort_format) = sort_format {
        repos.iter_mut().for_each(|repo| match repo {
            RepoEntry::Registered(_, vec) | RepoEntry::Unknown(_, vec) => sort_format.sort(vec),
            RepoEntry::Error(_, _) => {}
        });
    }

    Ok(repos)
}

pub fn list_builds(
    cfg: &BLRSConfig,
    ls_format: LsFormat,
    sort_format: SortFormat,
    installed_only: bool,
    show_variants: bool,
    all_builds: bool,
) -> std::io::Result<()> {
    std::fs::create_dir_all(&cfg.paths.library)
        .inspect_err(|e| error!("Failed to create library path: {:?}", e))?;

    let all_repos = gather_and_filter_repos(cfg, installed_only, all_builds, Some(sort_format))?;

    match ls_format {
        LsFormat::Tree => all_repos.into_iter().for_each(|repo_entry| {
            let tree = RepoEntryTreeConstructor(&repo_entry).to_tree(show_variants);

            println!["{}", tree];
        }),
        LsFormat::JustPaths => todo!(),
        LsFormat::Json => todo!(),
        LsFormat::PrettyJson => todo!(),
        LsFormat::Toml => todo!(),
    }

    Ok(())
}
