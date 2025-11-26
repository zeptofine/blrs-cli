use std::{
    fmt::Display,
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use ansi_term as at;
use blrs::{
    fetching::build_repository::BuildRepo,
    repos::{BuildEntry, BuildVariant, RepoEntry},
    search::VersionSearchQuery,
};
use chrono::{DateTime, TimeZone, Utc};
use clap::ValueEnum;
use semver::Version;
use serde::{Deserialize, Serialize};
use termtree as tt;

fn system_time_to_date_time(t: SystemTime) -> DateTime<Utc> {
    let nsec = match t.duration_since(UNIX_EPOCH) {
        Ok(dur) => dur.as_nanos(),
        Err(e) => e.duration().as_nanos(),
    };
    Utc.timestamp_nanos(nsec as i64)
}

#[derive(Debug, Clone, Copy, Default, ValueEnum, Serialize, Deserialize)]
pub enum SortFormat {
    #[default]
    Version,
    Datetime,
}
impl SortFormat {
    pub fn sort(self, v: &mut [BuildEntry]) {
        match self {
            SortFormat::Version => v.sort_by_key(|e| match e {
                BuildEntry::NotInstalled(remote_build) => (
                    remote_build.basic.version().clone(),
                    remote_build.basic.commit_dt,
                ),
                BuildEntry::Installed(_, local_build) => (
                    local_build.info.basic.version().clone(),
                    local_build.info.basic.commit_dt,
                ),
                BuildEntry::Errored(_error, _path_buf) => {
                    (Version::new(0, 0, 0), DateTime::default())
                }
            }),
            SortFormat::Datetime => {
                v.sort_by_key(|e| match e {
                    BuildEntry::NotInstalled(remote_build) => (
                        remote_build.basic.commit_dt,
                        remote_build.basic.version().clone(),
                    ),
                    BuildEntry::Installed(_, local_build) => (
                        local_build.info.basic.commit_dt,
                        local_build.info.basic.version().clone(),
                    ),
                    BuildEntry::Errored(_error, pb) => (
                        pb.clone()
                            .and_then(|pb| {
                                (fs::metadata(pb).map(|m| m.modified().ok()).ok().flatten())
                                    .map(system_time_to_date_time)
                            })
                            .unwrap_or_default(),
                        Version::new(0, 0, 0),
                    ),
                });
            }
        }
    }
}

#[derive(Debug)]
pub struct BuildEntryTreeConstructor<'a>(pub &'a BuildEntry);
impl BuildEntryTreeConstructor<'_> {
    fn to_tree(&self, show_variants: bool) -> tt::Tree<String> {
        let t = tt::Tree::new(self.to_string());
        match (self.0, show_variants) {
            (BuildEntry::NotInstalled(variants), true) => {
                t.with_leaves(variants.v.iter().map(BuildVariant::to_string))
            }
            _ => t,
        }
    }
}
impl Display for BuildEntryTreeConstructor<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            BuildEntry::NotInstalled(remote_builds) => write![
                f,
                "{} {}",
                VersionSearchQuery::from(remote_builds.basic.clone()).with_commit_dt(None),
                at::Color::White.dimmed().paint(format![
                    "{} - {} variants",
                    remote_builds.basic.commit_dt,
                    remote_builds.v.len()
                ]),
            ],
            BuildEntry::Installed(_, local_build) => {
                write![
                    f,
                    "{} {} {}",
                    VersionSearchQuery::from(local_build.info.basic.clone()).with_commit_dt(None),
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

#[derive(Debug)]
pub struct RepoEntryTreeConstructor<'a>(pub &'a RepoEntry<'a>);
impl RepoEntryTreeConstructor<'_> {
    pub fn to_tree(&self, show_variants: bool) -> tt::Tree<String> {
        let s = self.to_string();
        let leaves = match self.0 {
            RepoEntry::Registered(_, vec) | RepoEntry::Unknown(_, vec) => vec,
            RepoEntry::Error(_, _) => todo!(),
        };

        tt::Tree::new(s).with_leaves(
            leaves
                .iter()
                .map(|e| BuildEntryTreeConstructor(e).to_tree(show_variants)),
        )
    }
}
impl Display for RepoEntryTreeConstructor<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
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
                "{} {} - {} builds",
                at::Color::Yellow.paint(name),
                ansi_term::Color::White.dimmed().paint("(Unknown)"),
                builds.len(),
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
