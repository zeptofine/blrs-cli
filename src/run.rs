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

use log::{debug, info, warn};

use crate::{
    commands::RunCommand,
    errs::{CommandError, IoErrorOrigin},
    resolving::resolve_match,
};

pub fn run(
    cfg: &BLRSConfig,
    cmd: RunCommand,
    fail_on_unresolved_conflict: bool,
) -> Result<i32, CommandError> {
    let (file, query, args): (
        Option<PathBuf>,
        Option<VersionSearchQuery>,
        Option<Vec<String>>,
    ) = match cmd {
        RunCommand::File { path } => (Some(path.clone()), None, None),
        RunCommand::Build { build, args } => match build {
            Some(bof) => match VersionSearchQuery::try_from(bof.as_str()) {
                Ok(q) => (None, Some(q), Some(args)),
                Err(e) => return Err(CommandError::CouldNotParseQuery(bof, e)),
            },
            None => return Err(CommandError::NotEnoughInput),
        },
    };

    let query = query.unwrap_or_else(|| get_query_from_file(file.as_ref()));

    let chosen_build = select_build(cfg, fail_on_unresolved_conflict, &query)?;

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

    let mut params: GeneratedParams = params.unwrap();
    if let Some(args) = args {
        params.extend_args(args);
    }

    let mut command = process::Command::from(params);

    info!["Running command {:?}", command];

    command
        .status()
        .map(|exit_status| exit_status.code().unwrap_or_default())
        .map_err(|e| CommandError::IoError(IoErrorOrigin::CommandExecution, e))
}

fn select_build(
    cfg: &BLRSConfig,
    fail_on_unresolved_conflict: bool,
    query: &VersionSearchQuery,
) -> Result<blrs::LocalBuild, CommandError> {
    // Get repos with installed builds
    let repos: Vec<_> = read_repos(&cfg.repos, &cfg.paths, false)
        .map_err(|e| CommandError::IoError(IoErrorOrigin::ReadingRepos, e))?
        .into_iter()
        .filter_map(|r| match r {
            RepoEntry::Registered(BuildRepo { ref nickname, .. }, vec)
            | RepoEntry::Unknown(ref nickname, vec) => {
                let local_builds = vec
                    .into_iter()
                    .filter_map(|entry| match entry {
                        BuildEntry::Installed(_, build) => Some(build),
                        _ => None,
                    })
                    .collect::<Vec<_>>();

                (!local_builds.is_empty()).then_some((local_builds, nickname.clone()))
            }
            RepoEntry::Error(_, _) => None,
        })
        .collect();

    let builds = repos
        .into_iter()
        .flat_map(|(builds, nick)| builds.into_iter().map(move |b| (b, nick.clone())))
        .collect::<Vec<_>>();

    let matcher = BInfoMatcher::new(&builds);
    let initial_matches: Vec<_> = matcher.find_all(query).into_iter().cloned().collect();

    match (initial_matches.len(), fail_on_unresolved_conflict) {
        // No conflict found
        (1, _) => Ok(&initial_matches[0].0),
        // Conflict found and can't resolve
        (0 | 2.., true) => Err(CommandError::InvalidInput),
        // Conflict found and initial matches is empty
        (0, false) => resolve_match(
            &builds,
            &format!["No matches detected for query {query}! select a build"],
        )
        .ok_or(CommandError::InvalidInput),
        // Conflict found and there are initial matches
        (2.., false) => resolve_match(
            &initial_matches,
            &format!["Multiple matches for query {query}! select a build"],
        )
        .ok_or(CommandError::InvalidInput),
    }
    .cloned()
}

fn get_query_from_file(file: Option<&PathBuf>) -> VersionSearchQuery {
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
}
