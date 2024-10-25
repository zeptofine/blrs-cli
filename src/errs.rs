use std::path::PathBuf;

use blrs::{info::ArgGenerationError, search::query::FromError};
use reqwest::StatusCode;
use thiserror::Error;

#[derive(Debug)]
#[allow(dead_code)] // They are used in error viewing
pub enum IoErrorOrigin {
    Fetching,
    ReadingRepos,
    CommandExecution,
    RenamingObject(PathBuf, PathBuf),
    ReadingObject(PathBuf),
    WritingObject(PathBuf),
    DeletingObject(PathBuf),
}

#[derive(Error, Debug)]
pub enum CommandError {
    #[error(
        "Could not parse query {0:?}: {1:?}
    Query syntax: [repo/]<major>.<minor>.<patch>[-<branch>][[+ or #]<build_hash>][@<commit time>]
    The major, minor, and patch numbers can be integers, or one of these:
    - `^`    | Match the largest/newest item
    - `*`    | Match any item
    - `-`    | Match the smallest/oldest item
    The commit time HAS to be one of these. By default it is \"*\"
    "
    )]
    CouldNotParseQuery(String, FromError),
    #[error("Could not generate params: {0:?}")]
    CouldNotGenerateParams(ArgGenerationError),
    #[error("Not enough command input, see --help for details")]
    NotEnoughInput,
    #[error("Invalid command input, see --help for details")]
    InvalidInput,
    #[error("No matches for Query(s) {0:?}")]
    QueryResultEmpty(String),
    #[error("No query has been given but is required")]
    MissingQuery,
    #[error("Insufficient time has passed since the last fetch. It is unlikely that new builds will be available, and to conserve requests these will be skipped.\nWait for {remaining}s")]
    FetchingTooFast { remaining: i64 },
    #[error("Error making a request: {0:?}")]
    ReqwestError(reqwest::Error),
    #[error("request returned code {0:?}: {:?}", .0.canonical_reason())]
    ReturnCode(StatusCode),
    #[error("Unsupported file format: {0:?}")]
    UnsupportedFileFormat(String),
    #[error("Cancelled pre-emptively")]
    Cancelled,
    #[error("Trash error from {0:?}:  {1:?}")]
    TrashError(PathBuf, trash::Error),

    #[error("IO Error from {0:?}:  {1:?}")]
    IoError(IoErrorOrigin, std::io::Error),
}

impl CommandError {
    pub fn exit_code(&self) -> i32 {
        match self {
            CommandError::CouldNotParseQuery(_, _)
            | CommandError::MissingQuery
            | CommandError::NotEnoughInput
            | CommandError::InvalidInput
            | CommandError::QueryResultEmpty(_)
            | CommandError::FetchingTooFast { remaining: _ } => 2,
            CommandError::ReturnCode(_)
            | CommandError::UnsupportedFileFormat(_)
            | CommandError::CouldNotGenerateParams(_)
            | CommandError::ReqwestError(_) => 1,
            CommandError::IoError(_, error) => error.raw_os_error().unwrap_or(1),
            CommandError::TrashError(_, error) => match error {
                trash::Error::Os {
                    code,
                    description: _,
                } => *code,
                _ => 1,
            },
            CommandError::Cancelled => 130,
        }
    }
}

pub fn error_reading(p: PathBuf, e: std::io::Error) -> CommandError {
    CommandError::IoError(IoErrorOrigin::ReadingObject(p), e)
}
pub fn error_writing(p: PathBuf, e: std::io::Error) -> CommandError {
    CommandError::IoError(IoErrorOrigin::WritingObject(p), e)
}
pub fn error_renaming(p: PathBuf, p2: PathBuf, e: std::io::Error) -> CommandError {
    CommandError::IoError(IoErrorOrigin::RenamingObject(p, p2), e)
}
