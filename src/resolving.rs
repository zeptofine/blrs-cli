use std::collections::HashMap;

use blrs::repos::BuildVariant;
use blrs::{
    downloading::extensions::get_target_setup, fetching::build_repository::BuildRepo,
    repos::Variants, search::query::VersionSearchQuery, BasicBuildInfo, RemoteBuild,
};

fn get_choice_map<'a>(
    matches: &'a [(BasicBuildInfo, &BuildRepo)],
) -> HashMap<String, &'a BasicBuildInfo> {
    let mut x: Vec<_> = matches
        .iter()
        .map(|(b, repo)| (format!["{}/{}", repo.nickname, b.ver], b))
        .collect();
    x.sort_by_key(|(_, b)| (b.commit_dt, b.ver.clone()));
    let max_choice_size = x.iter().map(|(c, _)| c.len()).max().unwrap_or_default();
    x.into_iter()
        .map(|(c, build)| {
            (
                // Apply padding and add the date to the end
                format!["{:<cs$}  {}", c, build.commit_dt, cs = max_choice_size],
                build,
            )
        })
        .collect()
}

// If necessary, prompt the user to select which build to download
pub fn resolve_match<'a>(
    q: &VersionSearchQuery,
    matches: &'a [(BasicBuildInfo, &BuildRepo)],
) -> Option<&'a BasicBuildInfo> {
    if matches.len() == 1 {
        return Some(&matches[0].0);
    }

    let choice_map = get_choice_map(matches);

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
        Ok(s) => Some(choice_map[&s]),
        _ => None,
    }
}

pub fn resolve_variant(
    variants: Variants<RemoteBuild>,
    all_platforms: bool,
) -> Option<RemoteBuild> {
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
