use std::fs::File;
use std::path::{Path, PathBuf};
use std::{collections::HashMap, fmt::Write, time::Duration};

use blrs::info::build_info::LocalBuildInfo;
use blrs::LocalBuild;
use blrs::{
    downloading::extensions::get_target_setup,
    fetching::{build_repository::BuildRepo, fetcher::FetchStreamerState},
    repos::{read_repos, BuildEntry, BuildVariant, RepoEntry, Variants},
    search::{query::VersionSearchQuery, searching::BInfoMatcher},
    BLRSConfig, BasicBuildInfo, RemoteBuild,
};
use futures::AsyncWriteExt;
use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use inquire::InquireError;
use log::{debug, error, info, warn};
use reqwest::{Client, StatusCode, Url};
use tar::Archive;
use tokio::time::{interval, Interval};
use uuid::Uuid;
use xz::read::XzDecoder;

#[derive(Debug)]
pub enum PullFailureLocation {
    Downloading,
    Extraction,
    InfoGeneration,
}
#[derive(Debug)]
pub enum PullFailure {
    FailedToCreate(PathBuf),
    FailedToWrite,
    UnsupportedFormat,
    FailedToClose,
    FailedToRename,
    ReturnCode(StatusCode, Option<&'static str>),
    ReqwestError(reqwest::Error),
    IoError(std::io::Error),
}

pub async fn pull_builds(
    cfg: &BLRSConfig,
    queries: Vec<VersionSearchQuery>,
    all_platforms: bool,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(&cfg.paths.library)
        .inspect_err(|e| error!("Failed to create library path: {:?}", e))?;

    let repos: Vec<_> = read_repos(cfg.repos.clone(), &cfg.paths, false)?
        .into_iter()
        .filter_map(|r| match r {
            RepoEntry::Registered(repo, vec) => Some((
                repo,
                vec.into_iter()
                    .filter_map(|entry| match entry {
                        BuildEntry::NotInstalled(variants) => Some(variants),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            )),
            _ => None,
        })
        .filter(|(_, v)| !v.is_empty())
        .collect();

    let all_remote_builds: Vec<(BasicBuildInfo, (&Variants<RemoteBuild>, &BuildRepo))> = repos
        .iter()
        .flat_map(|(r, vec)| {
            vec.iter()
                .map(move |variants| (variants.basic.clone(), (variants, r)))
        })
        .collect();

    let full_size = all_remote_builds.len();

    let variant_map: HashMap<BasicBuildInfo, (Variants<RemoteBuild>, Vec<&BuildRepo>)> = {
        let mut m = HashMap::with_capacity(full_size);
        for (info, (variants, r)) in all_remote_builds.into_iter() {
            match m.remove(&info) {
                None => {
                    m.insert(info, (variants.clone(), vec![r]));
                }
                Some((mut var, mut repos)) => {
                    var.v.extend(variants.v.clone());
                    repos.push(r)
                }
            }
        }

        // Filter out build variants that do not coencide with self
        if !all_platforms {
            let target = get_target_setup().unwrap();

            let h: HashMap<_, _> = m
                .into_iter()
                .filter_map(|(key, (variants, repos))| {
                    let filtered = variants.filter_target(target);
                    match filtered.v.len() {
                        0 => None,
                        _ => Some((key, (filtered, repos))),
                    }
                })
                .collect();
            m = h;
        }

        m.shrink_to_fit();

        m
    };

    let builds: Vec<BasicBuildInfo> = variant_map.keys().cloned().collect();
    let matcher = BInfoMatcher::new(&builds);

    let matches: Vec<_> = queries.iter().map(|q| (q, matcher.find_all(q))).collect();

    // Check if any of the queries have no matches
    let empty_matches: Vec<_> = matches
        .iter()
        .filter_map(|(q, v)| v.is_empty().then_some(format!["{q}"]))
        .collect();
    if !empty_matches.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!["No matches for the queries: {}", empty_matches.join(", ")],
        ));
    }

    // Check if any of the queries had multiple matches. If so, perform conflict resolution
    let queue = resolve_queue(matches, variant_map);

    // Check the queue variants
    let remote_builds = resolve_variants(queue, all_platforms);

    let pb = MultiProgress::new();
    let template =
        "{spinner:.green} [{elapsed_precise} (ETA {eta})] [{bar:40.cyan/red}] {bytes}/{total_bytes} {msg:.green}";
    let pbstyle = ProgressStyle::with_template(template)
        .unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| {
            write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
        })
        .progress_chars("#!_");

    let futures: Vec<_> = remote_builds
        .into_iter()
        .map(|(remote_build, repo)| {
            let ppb = pb.add(ProgressBar::new(1000));
            ppb.set_style(pbstyle.clone());

            let mut intv = interval(Duration::from_millis(1));
            // Download file if does not exist
            let url = remote_build.url();

            let filename = PathBuf::from(url.path())
                .file_name()
                .map(|name| name.to_os_string())
                .unwrap_or_else(|| {
                    // Fallback to a generated name
                    PathBuf::from(Uuid::new_v4().to_string())
                        .with_extension(remote_build.file_extension.clone().unwrap_or_default())
                        .as_os_str()
                        .to_os_string()
                });

            let repo_path = cfg.paths.path_to_repo(repo);

            let completed_filepath = repo_path.join(&filename);
            let temporary_filepath = completed_filepath
                .with_extension(remote_build.file_extension.unwrap_or_default() + ".part");
            let destination = repo_path.join(remote_build.basic.ver.v.to_string());

            async move {
                if !completed_filepath.exists() {
                    let client = cfg
                        .client_builder(url.domain().is_some_and(|h| h.contains("api.github.com")))
                        .build()
                        .unwrap();

                    ppb.set_message(format!["Downloading file {}", remote_build.link]);

                    download_file(
                        &ppb,
                        &mut intv,
                        client,
                        url,
                        &temporary_filepath,
                        &completed_filepath,
                    )
                    .await
                    .map_err(|e| (PullFailureLocation::Downloading, e))?;
                }

                // Extract file
                ppb.set_message("Extracting file");
                let success = extract_file(&ppb, &mut intv, &completed_filepath, &destination)
                    .await
                    .map_err(|e| (PullFailureLocation::Extraction, PullFailure::IoError(e)))?;
                if !success {
                    return Err((
                        PullFailureLocation::Extraction,
                        PullFailure::UnsupportedFormat,
                    ));
                }

                ppb.set_message("Generating the build info");
                ppb.set_length(1);

                let lb = LocalBuild {
                    folder: destination,
                    info: LocalBuildInfo {
                        basic: remote_build.basic,
                        is_favorited: false,
                        custom_name: None,
                        custom_exe: None,
                        custom_env: None,
                    },
                };

                lb.write()
                    .map_err(|e| (PullFailureLocation::InfoGeneration, PullFailure::IoError(e)))?;

                ppb.set_position(0);

                ppb.finish();

                Ok(())
            }
        })
        .collect();

    let result: Vec<Result<(), (PullFailureLocation, PullFailure)>> =
        futures::future::join_all(futures).await;

    println!["{:?}", result];

    Ok(PathBuf::default())
}

fn resolve_queue<'a>(
    matches: Vec<(&VersionSearchQuery, Vec<&BasicBuildInfo>)>,
    mut variant_map: HashMap<BasicBuildInfo, (Variants<RemoteBuild>, Vec<&'a BuildRepo>)>,
) -> Vec<(Variants<RemoteBuild>, &'a BuildRepo)> {
    matches
        .into_iter()
        .filter_map(|(q, matches)| {
            if matches.len() == 1 {
                let vars = variant_map.remove(matches[0]).unwrap();
                return Some((vars.0, vars.1[0]));
            }

            debug!["{:#?}", matches];

            let choice_map: HashMap<String, (&BasicBuildInfo, &BuildRepo)> = {
                let mut x: Vec<_> = matches
                    .into_iter()
                    .flat_map(|b| {
                        let (_variants, repos) = variant_map.get(b).unwrap();

                        repos
                            .iter()
                            .map(move |r| (format!["{}/{}", r.nickname, b.ver], b, r))
                    })
                    .collect();

                x.sort_by_key(|(_, b, _)| (b.commit_dt, b.ver.clone()));

                let max_choice_size = x.iter().map(|(c, _, _)| c.len()).max().unwrap_or_default();

                x.into_iter()
                    .map(|(c, build, r)| {
                        (
                            // Apply padding and add the date to the end
                            format!["{:<cs$}  {}", c, build.commit_dt, cs = max_choice_size],
                            (build, *r),
                        )
                    })
                    .collect()
            };

            let choices: Vec<_> = choice_map.keys().cloned().collect();
            let last_idx = choices.len() - 1;

            println![];
            let inquiry = inquire::Select::new(
                &format![
                    "Multiple matches detected for {}! select which one you want to download",
                    q
                ],
                choices,
            )
            .with_starting_cursor(last_idx)
            .prompt();

            match inquiry {
                Ok(s) => {
                    let (info, repo) = choice_map[&s];
                    variant_map.remove(info).map(|(v, _)| (v, repo)) // This should always be Some anyways
                }
                Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
                    info!["Skipping this query"];
                    None
                }
                x => {
                    warn!["Failed to get user input! {:?}; skipping this query", x];
                    None
                }
            }
        })
        .collect()
}

fn resolve_variants(
    queue: Vec<(Variants<RemoteBuild>, &BuildRepo)>,
    all_platforms: bool,
) -> Vec<(RemoteBuild, &BuildRepo)> {
    queue
        .into_iter()
        .filter_map(|(variants, repo)| {
            let (resolve_txt, variants) = if !all_platforms {
                let v = variants.clone().filter_target(get_target_setup().unwrap());

                let v = if v.v.is_empty() { variants } else { v };

                (
                    "Failed to filter by platform! select which variant you want to download ",
                    v,
                )
            } else {
                ("Select which variant you want to download", variants)
            };

            // Resolve -- prompt the user which one to download
            if variants.v.len() == 1 {
                return Some((variants.v[0].b.clone(), repo));
            }

            let map: HashMap<String, BuildVariant<_>> = variants
                .v
                .into_iter()
                .map(|variant| (variant.to_string(), variant))
                .collect();

            let choices = map.keys().cloned().collect();

            let inquiry = inquire::Select::new(resolve_txt, choices).prompt();

            match inquiry {
                Ok(s) => Some((map[&s].b.clone(), repo)),
                _ => None,
            }
        })
        .collect()
}

async fn download_file(
    ppb: &ProgressBar,
    intv: &mut Interval,
    client: Client,
    url: Url,
    temporary_filepath: &Path,
    completed_filepath: &Path,
) -> Result<(), PullFailure> {
    // Make sure the temporary filepath exists
    std::fs::create_dir_all(&temporary_filepath.parent().unwrap())
        .map_err(|e| PullFailure::IoError(e))?;

    let mut file = async_std::fs::File::create(&temporary_filepath)
        .await
        .map_err(|_| PullFailure::FailedToCreate(temporary_filepath.into()))?;

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
                    .map_err(|_| PullFailure::FailedToWrite)?;
                intv.tick().await;
            }
            FetchStreamerState::Finished { response } => {
                if !response.status().is_success() {
                    return Err(PullFailure::ReturnCode(
                        response.status(),
                        response.status().canonical_reason(),
                    ));
                }

                file.flush().await.map_err(|_| PullFailure::FailedToWrite)?;
                file.close().await.map_err(|_| PullFailure::FailedToClose)?;

                async_std::fs::rename(&temporary_filepath, &completed_filepath)
                    .await
                    .map_err(|_| PullFailure::FailedToRename)?;

                intv.tick().await;
                break;
            }

            FetchStreamerState::Err(_) => {
                break;
            }
        }
    }

    // Moved out of the loop to gain ownership of the error
    if let FetchStreamerState::Err(error) = state {
        Err(PullFailure::ReqwestError(error))
    } else {
        Ok(())
    }
}

async fn extract_file<P>(
    ppb: &ProgressBar,
    intv: &mut Interval,
    filepath: P,
    destination: P,
) -> std::io::Result<bool>
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

            let file = XzDecoder::new(File::open(filepath)?);
            let mut archive = Archive::new(file);

            for entry in archive.entries()? {
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

                        async_std::fs::create_dir_all(pth.parent().unwrap()).await?;
                        entry.unpack(pth)?;

                        ppb.inc(unpacked_size);
                        intv.tick().await;
                    }
                    Err(e) => return Err(e),
                }
            }

            Ok(true)
        }
        _ => Ok(false),
    }
}
