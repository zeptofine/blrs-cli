use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    fs::{self, File},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ansi_term as at;
use blrs::{
    fetching::{build_repository::BuildRepo, build_schemas::builder_schema::BlenderBuildSchema},
    info::build_info::VerboseVersion,
    BLRSConfig, LocalBuild, RemoteBuild,
};
use chrono::{DateTime, TimeZone, Utc};
use clap::ValueEnum;
use log::error;
use serde::{Deserialize, Serialize};
use termtree as tt;
use uuid::Uuid;

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

#[derive(Debug, Clone, Copy, Default, ValueEnum, Serialize, Deserialize)]
pub enum SortFormat {
    #[default]
    Version,
    Datetime,
}
impl SortFormat {
    fn sort(&self, v: &mut [BuildEntry]) {
        match self {
            SortFormat::Version => v.sort_by_key(|e| match e {
                BuildEntry::NotInstalled(remote_build) => {
                    (remote_build.basic.ver.clone(), remote_build.basic.commit_dt)
                }
                BuildEntry::Installed(_, local_build) => (
                    local_build.info.basic.ver.clone(),
                    local_build.info.basic.commit_dt,
                ),
                BuildEntry::Errored(_error, _path_buf) => {
                    (VerboseVersion::default(), DateTime::default())
                }
            }),
            SortFormat::Datetime => {
                v.sort_by_key(|e| match e {
                    BuildEntry::NotInstalled(remote_build) => {
                        (remote_build.basic.commit_dt, remote_build.basic.ver.clone())
                    }
                    BuildEntry::Installed(_, local_build) => (
                        local_build.info.basic.commit_dt,
                        local_build.info.basic.ver.clone(),
                    ),
                    BuildEntry::Errored(_error, pb) => (
                        pb.clone()
                            .and_then(|pb| {
                                (fs::metadata(pb).map(|m| m.modified().ok()).ok().flatten())
                                    .map(system_time_to_date_time)
                            })
                            .unwrap_or_default(),
                        VerboseVersion::default(),
                    ),
                });
            }
        }
    }
}

pub fn list_builds(
    cfg: &BLRSConfig,
    ls_format: LsFormat,
    sort_format: SortFormat,
    installed_only: bool,
) -> Result<(), std::io::Error> {
    let repo_map: HashMap<String, &BuildRepo> =
        cfg.repos.iter().map(|r| (r.repo_id.clone(), r)).collect();
    let repos: HashSet<String> = repo_map.keys().cloned().collect();

    std::fs::create_dir_all(&cfg.paths.library)
        .inspect_err(|e| error!("Failed to create library path: {:?}", e))?;

    let folders: HashSet<String> = cfg
        .paths
        .library
        .read_dir()
        .inspect_err(|e| error!("Failed to read {:?}: {}", cfg.paths.library, e))?
        .filter_map(|item| {
            let item = item.ok()?;
            item.file_type()
                .ok()?
                .is_dir()
                .then(|| item.file_name().to_str().unwrap().to_string())
            // .and_then(|n| {
            //     let count = item.path().read_dir().ok()?.count();
            //     (count > 0).then(|| n)
            // })
        })
        .collect();

    // Every folder that corresponds to a known repo
    let known_existing_repos: HashSet<&String> = repos.intersection(&folders).collect();
    // Every repo that does not exist
    let missing_repos: HashSet<&String> = repos.difference(&folders).collect();

    // Every folder that does not correspond to a known repo
    let unknown_repos: HashSet<&String> = folders.difference(&repos).collect();

    let mut all_repos: Vec<RepoEntry> = []
        .into_iter()
        // Add all repos that are missing
        .chain(missing_repos.into_iter().map(|repo| {
            let r = *repo_map.get(repo).unwrap();
            let cache_path = cfg.paths.remote_repos.join(r.repo_id.clone() + ".json");
            if installed_only {
                RepoEntry::Registered(r.clone(), vec![])
            } else {
                let builds: Vec<BuildEntry> = read_repo_cache(&cache_path)
                    .into_iter()
                    .map(BuildEntry::from)
                    .collect();

                RepoEntry::Registered(r.clone(), builds)
            }
        }))
        // Add all repos that are unknown
        .chain(unknown_repos.into_iter().map(|repo| {
            let library_path = cfg.paths.library.join(repo);
            let entries = read_local_entries(&library_path);

            match entries {
                Ok(entries) => RepoEntry::Unknown(repo.clone(), entries),
                Err(e) => RepoEntry::Error(repo.clone(), e),
            }
        }))
        // Add all repos that do exist and are recognized
        .chain(known_existing_repos.into_iter().map(|repo| {
            let r = *repo_map.get(repo).unwrap();
            let cache_path = cfg.paths.remote_repos.join(r.repo_id.clone() + ".json");
            let library_path = cfg.paths.library.join(repo);

            let installed_entries = match read_local_entries(&library_path) {
                Ok(e) => e,
                Err(err) => return RepoEntry::Error(repo.clone(), err),
            };

            if installed_only {
                let repo_entry = RepoEntry::Registered(r.clone(), installed_entries);

                repo_entry
            } else {
                let repo_cache: HashMap<String, BuildEntry> = read_repo_cache(&cache_path)
                    .into_iter()
                    .map(|b| (b.basic.ver.to_string(), BuildEntry::NotInstalled(b)))
                    .collect();

                let mut all_builds = repo_cache;
                all_builds.extend(installed_entries.into_iter().map(|entry| match &entry {
                    BuildEntry::Installed(_dir, local_build) => {
                        (local_build.info.basic.ver.to_string(), entry)
                    }
                    BuildEntry::Errored(_, _) => (Uuid::new_v4().to_string(), entry),
                    BuildEntry::NotInstalled(_) => unreachable!(),
                }));
                let all_builds: Vec<BuildEntry> = all_builds.into_values().collect();

                let repo_entry = RepoEntry::Registered(r.clone(), all_builds);

                repo_entry
            }
        }))
        .collect();

    // println!["{:?}", all_repos];

    if installed_only {
        all_repos = all_repos
            .into_iter()
            .filter(|r| r.has_installed_builds())
            .collect();
    } else {
        all_repos.sort_by_key(|r| r.has_installed_builds());
    }

    match ls_format {
        LsFormat::Tree => all_repos.into_iter().for_each(|mut repo_entry| {
            let mut err_v = vec![];
            let entries: &mut Vec<_> = match &mut repo_entry {
                RepoEntry::Registered(_build_repo, vec) => vec,
                RepoEntry::Unknown(_, vec) => vec,
                RepoEntry::Error(_, _) => &mut err_v,
            };

            sort_format.sort(entries);

            let entries: Vec<_> = entries
                .iter_mut()
                .map(|entry| tt::Tree::new(format!["{}", entry]))
                .collect();

            let tree = tt::Tree::new(format!["{}", repo_entry]).with_leaves(entries);

            println!["{}", tree];
        }),
        LsFormat::JustPaths => todo!(),
        LsFormat::Json => todo!(),
        LsFormat::PrettyJson => todo!(),
        LsFormat::Toml => todo!(),
    }

    Ok(())
}

fn read_repo_cache(repo_cache_path: &Path) -> Vec<RemoteBuild> {
    match repo_cache_path.exists() {
        true => match File::open(repo_cache_path) {
            Ok(file) => {
                serde_json::from_reader::<_, Vec<BlenderBuildSchema>>(file).unwrap_or_default()
            }
            Err(_) => vec![],
        },
        false => vec![],
    }
    .into_iter()
    .map(RemoteBuild::from)
    .collect()
}

fn read_local_entries(repo_library_path: &Path) -> Result<Vec<BuildEntry>, std::io::Error> {
    Ok(repo_library_path
        .read_dir()
        .inspect_err(|e| error!("Failed to read dir {:?}: {}", repo_library_path, e))?
        .filter_map(|item| match item {
            Ok(f) => match f.file_type() {
                Ok(t) => match t.is_dir() | (t.is_symlink()) {
                    true => Some(
                        match LocalBuild::read(&f.path().read_link().unwrap_or(f.path())) {
                            Ok(build) => BuildEntry::Installed(
                                f.file_name().to_str().unwrap().to_string(),
                                build,
                            ),
                            Err(e) => BuildEntry::Errored(e, Some(f.path())),
                        },
                    ),
                    false => None,
                },
                Err(e) => Some(BuildEntry::Errored(e, Some(f.path()))),
            },

            Err(e) => Some(BuildEntry::Errored(e, None)),
        })
        .collect())
}

fn system_time_to_date_time(t: SystemTime) -> DateTime<Utc> {
    let nsec = match t.duration_since(UNIX_EPOCH) {
        Ok(dur) => dur.as_nanos(),
        Err(e) => {
            // unlikely but should be handled
            let dur = e.duration();
            dur.as_nanos()
        }
    };
    Utc.timestamp_nanos(nsec as i64)
}

fn format_build_repo(r: &BuildRepo) -> String {
    match r.nickname.as_str() {
        "" => format![
            "{} ({:?})",
            ansi_term::Color::Green.paint(r.repo_id.clone()),
            r.repo_type,
        ],
        nick => format![
            "{} {}",
            ansi_term::Color::Green.paint(nick),
            ansi_term::Color::White.dimmed().paint(format![
                "{} ({:?})",
                r.repo_id.clone(),
                r.repo_type.clone()
            ]),
        ],
    }
}

#[derive(Debug)]
enum RepoEntry {
    Registered(BuildRepo, Vec<BuildEntry>),
    Unknown(String, Vec<BuildEntry>),
    Error(String, std::io::Error),
}
impl RepoEntry {
    fn has_installed_builds(&self) -> bool {
        match self {
            RepoEntry::Registered(_, vec) | RepoEntry::Unknown(_, vec) => {
                vec.iter().any(|entry| match entry {
                    BuildEntry::Installed(_, _) => true,
                    _ => false,
                })
            }
            RepoEntry::Error(_, _) => false,
        }
    }
}
impl Display for RepoEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoEntry::Registered(build_repo, builds) => {
                write![
                    f,
                    "{} - {} builds",
                    format_build_repo(build_repo),
                    builds.len()
                ]
            }
            RepoEntry::Unknown(name, builds) => write![
                f,
                "{} - {} builds {}",
                name,
                builds.len(),
                ansi_term::Color::White.dimmed().paint("(Unknown)")
            ],
            RepoEntry::Error(name, error) => write![
                f,
                "{} {}",
                at::Color::Red.bold().paint(format!["Error at {:?}:", name]),
                at::Color::White.dimmed().paint(format!["{:?}", error])
            ],
        }
    }
}

#[derive(Debug)]
enum BuildEntry {
    NotInstalled(RemoteBuild),
    Installed(String, LocalBuild),
    Errored(std::io::Error, Option<PathBuf>),
}
impl From<RemoteBuild> for BuildEntry {
    fn from(value: RemoteBuild) -> Self {
        Self::NotInstalled(value)
    }
}
impl From<(String, LocalBuild)> for BuildEntry {
    fn from(value: (String, LocalBuild)) -> Self {
        Self::Installed(value.0, value.1)
    }
}

impl Display for BuildEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildEntry::NotInstalled(remote_build) => write![
                f,
                "{} {}",
                remote_build.basic.ver.v,
                at::Color::White
                    .dimmed()
                    .paint(format!["{}", remote_build.basic.commit_dt]),
            ],
            BuildEntry::Installed(_, local_build) => {
                write![
                    f,
                    "{} {} {}",
                    local_build.info.basic.ver.v,
                    at::Color::White
                        .dimmed()
                        .paint(format!["{}", local_build.info.basic.commit_dt]),
                    at::Color::Cyan.paint("(Installed)")
                ]
            }
            BuildEntry::Errored(error, path_buf) => write![
                f,
                "{} {}",
                at::Color::Red
                    .bold()
                    .paint(format!["Error at {:?}:", path_buf]),
                at::Color::White.dimmed().paint(format!["{:?}", error])
            ],
        }
    }
}
