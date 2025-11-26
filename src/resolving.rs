use std::collections::HashMap;
use std::fmt::Display;

use blrs::build_targets::get_target_setup;
use blrs::repos::{BuildVariant, Variants};
use blrs::search::VersionSearchQuery;
use blrs::{BasicBuildInfo, RemoteBuild};

// type RepoNickname = String;

pub fn get_choice_map<Binfo, RepoNick>(matches: &[(Binfo, RepoNick)]) -> HashMap<String, &Binfo>
where
    Binfo: AsRef<BasicBuildInfo>,
    RepoNick: Display,
{
    let mut x: Vec<_> = matches
        .iter()
        .map(|(b, nick)| {
            (
                format![
                    "{}/{}",
                    nick,
                    VersionSearchQuery::from(b.as_ref().clone()).with_commit_dt(None)
                ],
                b,
            )
        })
        .collect();
    x.sort_by_key(|(_, b)| (b.as_ref().commit_dt, b.as_ref().ver.clone()));
    let max_choice_size = x.iter().map(|(c, _)| c.len()).max().unwrap_or_default();
    x.into_iter()
        .map(|(c, build)| {
            (
                // Apply padding and add the date to the end
                format![
                    "{:<cs$}  {}",
                    c,
                    build.as_ref().commit_dt,
                    cs = max_choice_size
                ],
                build,
            )
        })
        .collect()
}

// If necessary, prompt the user to select which build to download
pub fn resolve_match<'a, B, N>(matches: &'a [(B, N)], prompt: &str) -> Option<&'a B>
where
    B: AsRef<BasicBuildInfo>,
    N: Display,
{
    if matches.len() == 1 {
        return Some(&matches[0].0);
    }

    let choice_map = get_choice_map(matches);

    let mut choices: Vec<_> = choice_map.keys().collect();

    // Sort the matches by the commit date, then the version
    choices.sort_by_cached_key(|b| {
        let build = choice_map[*b].as_ref();
        build
    });

    let last_idx = choices.len() - 1;

    println![];
    let inquiry = inquire::Select::new(prompt, choices)
        .with_starting_cursor(last_idx)
        .prompt();

    match inquiry {
        Ok(s) => Some(choice_map[s]),
        _ => None,
    }
}

pub fn resolve_variant(
    variants: Variants<RemoteBuild>,
    all_platforms: bool,
) -> Option<RemoteBuild> {
    let (resolve_txt, variants) = if all_platforms {
        ("Select which variant you want to download", variants)
    } else {
        let mut v = variants.clone().filter_target(get_target_setup().unwrap());
        v.v.sort_by_key(BuildVariant::to_string);

        let v = if v.v.is_empty() { variants } else { v };

        (
            "Failed to filter by platform! select which variant you want to download ",
            v,
        )
    };

    // Resolve -- prompt the user which one to download
    if variants.v.len() == 1 {
        return Some(variants.v[0].b.clone());
    }

    let map: HashMap<String, BuildVariant<_>> = variants
        .v
        .into_iter()
        .map(|variant| (variant.to_string(), variant))
        .collect();

    let choices = map.keys().cloned().collect();

    let inquiry = inquire::Select::new(resolve_txt, choices).prompt();

    match inquiry {
        Ok(s) => Some(map[&s].b.clone()),
        _ => None,
    }
}
