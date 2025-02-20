use std::path::{Path, PathBuf};

use blrs::{info::launching::OSLaunchTarget, BLRSConfig, LocalBuild};
use log::{debug, error, info};

use crate::errs::CommandError as CE;

#[inline]
fn is_dir_or_link_to_dir(p: &Path) -> bool {
    p.is_dir() || p.read_link().is_ok_and(|p| p.is_dir())
}

pub fn verify(cfg: &BLRSConfig, repos: Option<Vec<String>>) -> Result<(), CE> {
    let mut folders: Vec<PathBuf> = cfg
        .paths
        .library
        .read_dir()
        .map_err(CE::reading(cfg.paths.library.clone()))?
        .filter_map(|item| {
            let item = item.ok()?;
            item.file_type().ok()?.is_dir().then(|| item.path())
        })
        .collect();

    folders = match repos {
        Some(v) => folders
            .into_iter()
            .filter(|pth| v.iter().any(|r| pth.ends_with(r)))
            .collect(),
        None => folders,
    };

    debug!["Reading folders: {:?}", folders];

    for folder in folders {
        let _: Vec<_> = folder
            .read_dir().map_err(CE::reading(folder))?
            .filter_map(|build_folder| {
                let build_folder = build_folder.ok()?;
                let path = build_folder.path();
                if is_dir_or_link_to_dir(&build_folder.path()){

                    match LocalBuild::read(&path) {
                        Ok(build) => {
                            debug!["Successfully read {:?}", build];

                            Some(())
                        }
                        Err(e) => {
                            error!["Failed to read build: {:?}\n Attempting to read the build for more info", e];
                            let executable = path.join(OSLaunchTarget::try_default().unwrap().exe_name());
                            match LocalBuild::generate_from_exe(&executable) {
                                Ok(b) => {
                                    debug!["{:?}", b];
                                    info!["Success! Saving build..."];
                                    let r = b.write();
                                    info!["{:?}", r];

                                    Some(())
                                },
                                Err(e) => {
                                    println!{"Error: {:?}", e};
                                    None
                                },
                            }



                        }
                    }
                } else {
                    debug!["Skipping file {:?}", build_folder];
                    None
                }
            })
            .collect();
    }

    Ok(())
}
