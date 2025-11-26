use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};

use blrs::build_targets::get_target_setup;
use blrs::info::build_info::LocalBuildInfo;
use blrs::search::{BInfoMatcher, VersionSearchQuery};
use blrs::LocalBuild;
use blrs::{
    fetching::{build_repository::BuildRepo, fetcher::FetchStreamerState},
    repos::{read_repos, BuildEntry, RepoEntry, Variants},
    BLRSConfig, BasicBuildInfo, RemoteBuild,
};

use futures::AsyncWriteExt;
use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use log::{error, info, warn};
use reqwest::{Client, Url};
use tar::Archive;
use uuid::Uuid;
use xz::read::XzDecoder;
use zip::ZipArchive;

use crate::errs::{CommandError as CE, IoErrorOrigin};
use crate::resolving::{resolve_match, resolve_variant};

pub static CANCELLED: LazyLock<Arc<AtomicBool>> =
    LazyLock::new(|| Arc::new(AtomicBool::new(false)));

pub async fn pull_builds(
    cfg: &BLRSConfig,
    queries: Vec<VersionSearchQuery>,
    all_platforms: bool,
) -> Result<(), CE> {
    std::fs::create_dir_all(&cfg.paths.library)
        .inspect_err(|e| error!("Failed to create library path: {:?}", e))
        .map_err(CE::writing(&cfg.paths.library))?;

    let repos: Vec<_> = read_repos(&cfg.repos, &cfg.paths, false)
        .map_err(|e| CE::IoError(IoErrorOrigin::ReadingRepos, e))?
        .into_iter()
        .filter_map(|r| match r {
            RepoEntry::Registered(repo, vec) => {
                let collect = vec
                    .into_iter()
                    .filter_map(|entry| match entry {
                        BuildEntry::NotInstalled(variants) => Some(variants),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                match collect.is_empty() {
                    false => Some((repo, collect)),
                    true => None,
                }
            }
            _ => None,
        })
        .collect();

    let map = build_map(&repos, all_platforms);

    let versions: Vec<(&BasicBuildInfo, &str)> = map
        .iter()
        .map(|(b, (r, _))| (b, r.nickname.as_str()))
        .collect();
    let matcher = BInfoMatcher::new(&versions);
    let version_matches: Vec<(&VersionSearchQuery, Vec<(&BasicBuildInfo, &str)>)> = queries
        .iter()
        .map(|query| {
            (
                query,
                matcher.find_all(query).into_iter().cloned().collect(),
            )
        })
        .collect();

    // Check if any of the queries have no matches
    {
        let empty_matches: Vec<_> = version_matches
            .iter()
            .filter_map(|(q, v)| v.is_empty().then_some(format!["{q}"]))
            .collect();
        if !empty_matches.is_empty() {
            return Err(CE::QueryResultEmpty(empty_matches.join(", ")));
        }
    }

    // Get builds selected to download
    let mut dl_map = build_map(&repos, all_platforms);

    let choices: Vec<_> = version_matches
        .into_iter()
        // Check if any of the queries had multiple matches. If so, perform conflict resolution
        .filter_map(|(query, matches)| {
            resolve_match(
                &matches,
                &format!["Multiple matches for query {query}! select a build to download"],
            )
            .cloned()
        })
        // Get variants of the chosen builds
        .map(|info| {
            let build = dl_map.remove(info).unwrap();
            info![
                "Selected build {}/{} for installation",
                build.0.nickname, info.ver
            ];
            build
        })
        // Check if the variants were larger than 1. If so, perform conflict resolution
        .filter_map(|(repo, variants): (_, _)| {
            resolve_variant(variants, all_platforms).map(|build| (repo, build))
        })
        .collect();

    // // ? Progress bar styling
    let pb = MultiProgress::new();
    let template = "{spinner:.green} [{elapsed_precise} (ETA {eta})] [{bar:40.cyan/red}] {bytes}/{total_bytes} {msg:.green}";
    let pbstyle = ProgressStyle::with_template(template)
        .unwrap()
        .with_key(
            "eta",
            |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap();
            },
        )
        .progress_chars("#|-");

    // // Setup Ctrl+C handler, if possible
    let _ = ctrlc::set_handler(|| {
        CANCELLED.store(true, Ordering::Release);
    });

    let setups: Vec<_> = choices
        .into_iter()
        .map(|(repo, remote_build)| {
            let url = remote_build.url();
            let extension = remote_build.file_extension.unwrap_or_default();
            let filename = PathBuf::from(url.path()).file_name().map_or_else(
                || {
                    // Fallback to a generated name
                    PathBuf::from(Uuid::new_v4().to_string())
                        .with_extension(&extension)
                        .as_os_str()
                        .to_os_string()
                },
                |name| name.to_os_string(),
            );

            let repo_path = cfg.paths.path_to_repo(repo);

            let completed_filepath = repo_path.join(filename);
            let temporary_filepath = completed_filepath.with_extension(extension + ".part");
            let destination = repo_path.join(remote_build.basic.version().to_string());

            let ppb = pb.add(ProgressBar::new(0));
            ppb.set_style(pbstyle.clone());
            (
                process_build(
                    ppb,
                    cfg,
                    url,
                    remote_build.basic,
                    temporary_filepath.clone(),
                    completed_filepath.clone(),
                    destination,
                ),
                temporary_filepath,
                completed_filepath,
            )
        })
        .collect();

    let targets: Vec<(PathBuf, PathBuf)> = setups
        .iter()
        .map(|(_, temp, finished)| (temp.clone(), finished.clone()))
        .collect();
    let result: Vec<Result<(), CE>> =
        futures::future::join_all(setups.into_iter().map(|(fut, _, _)| fut))
            .await
            .into_iter()
            .collect();

    prompt_deletions(result, targets);

    Ok(())
}

fn build_map<'a>(
    repos: &[(&'a BuildRepo, Vec<Variants<RemoteBuild>>)],
    all_platforms: bool,
) -> HashMap<BasicBuildInfo, (&'a BuildRepo, Variants<RemoteBuild>)> {
    let mut m = HashMap::with_capacity(repos.len());
    repos
        .iter()
        .flat_map(|(r, vec)| {
            vec.iter()
                .map(move |variants| (variants.basic.clone(), (variants, *r)))
        })
        .for_each(|(info, (variants, r))| match m.remove(&info) {
            None => {
                m.insert(info, (r, variants.clone()));
            }
            Some((ref mut repos, mut var)) => {
                var.v.extend(variants.v.clone());
                *repos = r;
            }
        });

    // Filter out build variants that do not coencide with our system
    if !all_platforms {
        let target = get_target_setup().unwrap();

        let h: HashMap<_, _> = m
            .into_iter()
            .filter_map(|(key, (repo, variants))| {
                let filtered = variants.filter_target(target);
                match filtered.v.len() {
                    0 => None,
                    _ => Some((key, (repo, filtered))),
                }
            })
            .collect();
        m = h;
    }
    m
}

async fn process_build(
    ppb: ProgressBar,
    cfg: &BLRSConfig,
    url: Url,
    basic: BasicBuildInfo,
    temporary_filepath: PathBuf,
    completed_filepath: PathBuf,
    destination: PathBuf,
) -> Result<(), CE> {
    if !completed_filepath.exists() {
        let client = cfg.client_builder().build().unwrap();

        ppb.set_message(format!["Downloading file {}", url]);

        download_file(&ppb, client, url, &temporary_filepath, &completed_filepath).await?;
    }

    // Extract file
    ppb.set_message(format!["Extracting file {}", completed_filepath.display()]);
    let success = extract_file(&ppb, &completed_filepath, &destination).await?;
    if !success {
        return Err(CE::UnsupportedFileFormat(
            completed_filepath
                .extension()
                .unwrap()
                .to_str()
                .unwrap()
                .into(),
        ));
    }

    ppb.set_message("Generating build info");
    ppb.set_position(0);
    ppb.set_length(1);

    let lb = LocalBuild {
        folder: destination.clone(),
        info: LocalBuildInfo {
            basic,
            is_favorited: false,
            custom_name: None,
            custom_exe: None,
            custom_env: None,
        },
    };

    lb.write().map_err(CE::writing(&destination))?;

    // Delete archive file

    ppb.set_message("Deleting temp file");
    if trash::delete(&completed_filepath).is_err() {
        std::fs::remove_file(completed_filepath).map_err(CE::writing(&destination))?;
    }

    ppb.set_message("Done");

    ppb.finish();

    Ok(())
}

async fn download_file(
    ppb: &ProgressBar,
    client: Client,
    url: Url,
    temporary_filepath: &Path,
    completed_filepath: &Path,
) -> Result<(), CE> {
    // Make sure the temporary filepath exists
    std::fs::create_dir_all(temporary_filepath.parent().unwrap())
        .map_err(CE::writing(temporary_filepath.parent().unwrap().into()))?;

    let mut file = async_std::fs::File::create(&temporary_filepath)
        .await
        .map_err(CE::writing(temporary_filepath.into()))?;

    let mut state = FetchStreamerState::new(client, url);

    let mut length = None;

    loop {
        state = state.advance().await;

        match &state {
            FetchStreamerState::Ready(_, _) => unreachable!(),
            FetchStreamerState::Downloading {
                response,
                last_chunk,
            } => {
                if length.is_none() {
                    if let Some(received_length) = response.content_length() {
                        length = Some(received_length);
                        ppb.set_length(received_length);
                    }
                }
                {}

                ppb.inc(last_chunk.len() as u64);

                file.write_all(last_chunk)
                    .await
                    .map_err(CE::writing(temporary_filepath.into()))?;
            }
            FetchStreamerState::Finished { response } => {
                if !response.status().is_success() {
                    return Err(CE::ReturnCode(response.status()));
                }

                file.flush()
                    .await
                    .map_err(CE::writing(temporary_filepath.into()))?;
                file.close()
                    .await
                    .map_err(CE::writing(temporary_filepath.into()))?;

                async_std::fs::rename(&temporary_filepath, &completed_filepath)
                    .await
                    .map_err(CE::renaming(
                        temporary_filepath.into(),
                        completed_filepath.into(),
                    ))?;

                break;
            }

            FetchStreamerState::Err(_) => {
                break;
            }
        }

        if CANCELLED.load(Ordering::Acquire) {
            drop(state);
            drop(file);

            return Err(CE::Cancelled);
        }
    }

    // Moved out of the loop to gain ownership of the error
    match state {
        FetchStreamerState::Err(error) => Err(CE::ReqwestError(error)),
        _ => Ok(()),
    }
}

async fn extract_file<P>(ppb: &ProgressBar, filepath: P, destination: P) -> Result<bool, CE>
where
    P: AsRef<Path>,
{
    let filepath = filepath.as_ref();
    let destination = destination.as_ref();
    match filepath.extension().unwrap().to_str().unwrap() {
        "xz" => {
            let total_size = filepath.metadata().unwrap().len();
            ppb.set_length(total_size);
            ppb.set_position(0);

            let file = XzDecoder::new(File::open(filepath).map_err(CE::reading(filepath.into()))?);
            let mut archive = Archive::new(file);

            for entry in archive.entries().map_err(CE::reading(filepath.into()))? {
                match entry {
                    Ok(mut entry) => {
                        let unpacked_size = entry.size();

                        // Skip the root folder
                        let pth: PathBuf = destination.join(
                            entry
                                .path()
                                .unwrap()
                                .components()
                                .skip(1)
                                .collect::<PathBuf>(),
                        );

                        let parent_path = pth.parent().unwrap();
                        async_std::fs::create_dir_all(parent_path)
                            .await
                            .map_err(CE::writing(parent_path.into()))?;
                        entry.unpack(&pth).map_err(CE::writing(&pth))?;

                        ppb.inc(unpacked_size);
                    }
                    Err(e) => {
                        return Err(CE::IoError(
                            IoErrorOrigin::WritingObject(filepath.into()),
                            e,
                        ));
                    }
                }

                if CANCELLED.load(Ordering::Acquire) {
                    return Err(CE::Cancelled);
                }
            }

            Ok(true)
        }
        "zip" => {
            let mut archive =
                ZipArchive::new(File::open(filepath).map_err(CE::reading(filepath.into()))?)
                    .map_err(|e| match e {
                        zip::result::ZipError::Io(error) => CE::reading(filepath)(error),
                        zip::result::ZipError::InvalidArchive(e) => {
                            CE::BrokenArchive(filepath.to_path_buf(), e.to_string())
                        }
                        zip::result::ZipError::UnsupportedArchive(e) => {
                            CE::BrokenArchive(filepath.to_path_buf(), e.to_string())
                        }
                        zip::result::ZipError::FileNotFound => todo!(),
                        zip::result::ZipError::InvalidPassword => todo!(),
                        _ => todo!(),
                    })?;

            let total_size = archive
                .decompressed_size()
                .map_or_else(|| filepath.metadata().unwrap().len(), |n| n as u64);
            ppb.set_length(total_size);
            ppb.set_position(0);

            for name in archive.file_names().map(str::to_string).collect::<Vec<_>>() {
                let mut file = archive.by_name(&name).unwrap();

                let file_path = file.enclosed_name().unwrap_or(file.mangled_name());

                // Skip the root folder
                let pth: PathBuf =
                    destination.join(file_path.components().skip(1).collect::<PathBuf>());

                let parent_path = pth.parent().unwrap();
                let _ = async_std::fs::create_dir_all(parent_path).await;
                if file.is_dir() {
                    async_std::fs::create_dir_all(&pth)
                        .await
                        .map_err(CE::writing(&pth))?;
                } else {
                    {
                        let mut extracted_file =
                            std::fs::File::create(&pth).map_err(CE::writing(&pth))?;

                        let mut v = Vec::with_capacity(file.size() as usize);
                        file.read_to_end(&mut v).map_err(CE::writing(&pth))?;
                        extracted_file.write_all(&v).map_err(CE::writing(&pth))?;
                    }
                }

                ppb.inc(file.size());

                if CANCELLED.load(Ordering::Acquire) {
                    return Err(CE::Cancelled);
                }
            }

            Ok(true)
        }
        "dmg" => {
            println!["DETECTED DMG FILE {:?}", filepath];
            todo!();
        }
        ext => Err(CE::UnsupportedFileFormat(ext.to_string())),
    }
}

/// Prompt the user to delete files after cancellation of pulling
fn prompt_deletions(result: Vec<Result<(), CE>>, targets: Vec<(PathBuf, PathBuf)>) {
    result
        .into_iter()
        .zip(targets)
        .for_each(|(result, (temp, finished))| {
            if let Err(CE::Cancelled) = result {
                if temp.exists() {
                    let s = format![
                        "Cancelled during downloading of {}. Do you wish to delete it?",
                        temp.display()
                    ];
                    let inquiry = inquire::Confirm::new(&s).with_default(false);
                    match inquiry.prompt_skippable() {
                        Ok(Some(true)) => {
                            info!["Deleting {:?}...", temp];

                            match std::fs::remove_file(&temp) {
                                Ok(_) => info!["Success."],
                                Err(e) => warn!["Failed to delete {:?}! {:?}", temp, e],
                            }
                        }
                        Ok(_) | Err(_) => todo!(),
                    }
                }

                if finished.exists() {
                    let s = format![
                        "Cancelled during extraction of {}. Do you wish to delete it?",
                        temp.display()
                    ];
                    let inquiry = inquire::Confirm::new(&s).with_default(false);
                    match inquiry.prompt_skippable() {
                        Ok(Some(true)) => {
                            info!["Deleting {:?}...", finished];

                            match std::fs::remove_file(&finished) {
                                Ok(()) => info!["Success."],
                                Err(e) => warn!["Failed to delete {:?}! {:?}", finished, e],
                            }
                        }
                        Ok(_) | Err(_) => todo!(),
                    }
                }
            }
        });
}
