use std::{path::PathBuf, process};

use blrs::{
    fetching::build_repository::BuildRepo,
    info::{
        launching::{BlendLaunchTarget, GeneratedParams, LaunchArguments, OSLaunchTarget},
        read_blendfile_header,
    },
    repos::{read_repos, BuildEntry, RepoEntry},
    search::{BInfoMatcher, OrdPlacement, VersionSearchQuery, WildPlacement},
    BLRSConfig,
};

use log::{debug, warn, info};

use crate::{
    commands::RunCommand,
    errs::{CommandError, IoErrorOrigin},
    resolving::resolve_match,
};

pub fn run(
    cfg: &BLRSConfig,
    cmd: RunCommand,
    fail_on_unresolved_conflict: bool,
) -> Result<usize, CommandError> {
    let (file, query): (Option<PathBuf>, Option<VersionSearchQuery>) = match &cmd {
        RunCommand::File { path } => (Some(path.clone()), None),
        RunCommand::Build {
            build_or_file,
            open_last: _,
        } => match build_or_file {
            Some(bof) => match VersionSearchQuery::try_from(bof.as_str()) {
                Ok(q) => (None, Some(q)),
                Err(_) => {
                    debug![
                        "Failed to convert {} to a query; assuming it's a blendfile",
                        bof
                    ];
                    (Some(PathBuf::from(bof)), None)
                }
            },
            None => return Err(CommandError::NotEnoughInput),
        },
    };

    let query = query.unwrap_or_else(|| {
        let file = file.as_ref().unwrap();

        // try to assume a query from the file header
        read_blendfile_header(file)
            .map(|header| {
                debug!["Header: {:?}", header];
                let ver = header.version();

                VersionSearchQuery {
                    repository: WildPlacement::default(),
                    major: OrdPlacement::Exact(ver.major),
                    minor: OrdPlacement::Exact(ver.minor),
                    patch: OrdPlacement::default(),
                    branch: WildPlacement::default(),
                    build_hash: WildPlacement::default(),
                    commit_dt: OrdPlacement::default(),
                }
            })
            .inspect_err(|e| warn!["Failed to generate a query from {:?}: {:?}", file, e])
            .unwrap_or_default()
    });

    let chosen_build = {
        // Get repos with installed builds
        let builds = read_repos(cfg.repos.clone(), &cfg.paths, false)
            .map_err(|e| CommandError::IoError(IoErrorOrigin::ReadingRepos, e))?
            .into_iter()
            .filter_map(|r| match r {
                RepoEntry::Registered(
                    BuildRepo {
                        repo_id: _,
                        url: _,
                        nickname,
                        repo_type: _,
                    },
                    vec,
                )
                | RepoEntry::Unknown(nickname, vec) => {
                    let local_builds = vec
                        .into_iter()
                        .filter_map(|entry| match entry {
                            BuildEntry::Installed(_, build) => Some(build),
                            _ => None,
                        })
                        .collect::<Vec<_>>();

                    match local_builds.is_empty() {
                        false => Some((local_builds, nickname)),
                        true => None,
                    }
                }
                _ => None,
            })
            .flat_map(|(builds, nick)| builds.into_iter().map(move |b| (b, nick.clone())))
            .collect::<Vec<_>>();

        let matcher = BInfoMatcher::new(&builds);
        let initial_matches = matcher.find_all(&query);
        match (initial_matches.len(), fail_on_unresolved_conflict) {
            // No conflict found
            (1, _) => Some(initial_matches[0].0.clone()),
            // Conflict found and can't resolve
            (0 | 2.., true) => return Err(CommandError::InvalidInput),
            // Conflict found and initial matches is empty
            (0, false) => resolve_match(&query, &builds).cloned(),
            // Conflict found and there are initial matches
            (2.., false) => resolve_match(
                &query,
                &initial_matches.into_iter().cloned().collect::<Vec<_>>(),
            )
            .cloned(),
        }
    };

    let chosen_build = match chosen_build {
        Some(c) => c,
        None => return Err(CommandError::InvalidInput),
    };

    let launch_arguments = LaunchArguments {
        file_target: match file {
            Some(f) => BlendLaunchTarget::File(f),
            None => BlendLaunchTarget::None,
        },
        os_target: OSLaunchTarget::default(),
        env: None,
    };

    let params = launch_arguments.assemble(&chosen_build);

    if let Err(e) = params {
        return Err(CommandError::CouldNotGenerateParams(e));
    }

    let params: GeneratedParams = params.unwrap();

    let mut command = process::Command::new(params.exe);

    command
        .args(
            params
                .args
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<String>>(),
        )
        .envs(params.env.clone().unwrap_or_default());

    info!["Running command {:?}", command];

    command
        .status()
        .map(|exit_status| exit_status.code().map(|i| i as usize).unwrap_or_default())
        .map_err(|e| CommandError::IoError(IoErrorOrigin::CommandExecution, e))
}
