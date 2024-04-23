#![feature(lazy_cell)]
#![feature(let_chains)]

use anyhow::{anyhow, Result};
use die_exit::Die;
use reqwest::Url;
use std::env;
use std::path::PathBuf;
use std::sync::LazyLock as Lazy;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
static REQUEST_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent(APP_USER_AGENT)
        .build()
        .expect("An error occured in building request client.")
});

#[tokio::main]
async fn main() -> Result<()> {
    let folder = env::args()
        .nth(1)
        .die("please enter a folder name. Ex: `auto-update bucket`");
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        if let Some(ext) = entry.path().extension()
            && ext == "json"
        {
            let path = entry.path();
            let ret = process_file(&path).await;
            if let Err(e) = ret {
                eprintln!("Err in {}: {}", path.display(), e);
            }
        }
    }
    Ok(())
}

async fn process_file(path: &PathBuf) -> Result<()> {
    let content = tokio::fs::read_to_string(&path).await?;
    let mut json_content: serde_json::Value = serde_json::from_str(&content)?;
    let old_version = json_content
        .get("version")
        .ok_or(anyhow!("No version found in {}.", path.display()))?
        .as_str()
        .ok_or(anyhow!("version cannot be parsed to str."))?
        .to_owned();
    let homepage = json_content
        .get("checkver")
        .and_then(|cv| cv.get("github"))
        .ok_or(anyhow!("No `checkver.github` found in {}.", path.display()))?
        .as_str()
        .ok_or(anyhow!("checkver url cannot be parsed to str."))?;
    let mut new_version = get_last_version(Url::parse(homepage)?).await?;
    if !old_version.starts_with('v')
        && let Some(ret) = new_version.strip_prefix('v')
    {
        new_version = ret.to_owned();
    }
    println!("{} -> {}", old_version, new_version);
    json_content["version"] = new_version.clone().into();
    tokio::fs::write(
        path,
        serde_json::to_string_pretty(&json_content)?.replace(&old_version, &new_version),
    )
    .await?;
    Ok(())
}

/// Join all strings to a [`Url`] object.
pub trait UrlJoinAll<'a> {
    fn join_all<I: IntoIterator<Item = String>>(&self, paths: I) -> Result<Url>;
    fn join_all_str<I: IntoIterator<Item = &'a str>>(&self, paths: I) -> Result<Url>;
}

impl<'a> UrlJoinAll<'a> for Url {
    /// Join all [`String`] to a [`Url`] object. The result [`Url`] must not
    /// have trailing slash.
    fn join_all<I: IntoIterator<Item = String>>(&self, paths: I) -> Result<Url> {
        let mut url = self.clone();
        for mut path in paths {
            if !path.ends_with('/') {
                path.push('/');
            }
            url = url.join(path.as_str())?;
        }
        let _ = url
            .path_segments_mut()
            .expect(
                "An error occurs in popping trailing slash of a url; the given url cannot be base.",
            )
            .pop_if_empty();
        Ok(url)
    }
    /// Join all &str to a [`Url`] object. The result [`Url`] must not
    /// have trailing slash.
    fn join_all_str<I: IntoIterator<Item = &'a str>>(&self, paths: I) -> Result<Url> {
        self.join_all(paths.into_iter().map(std::string::ToString::to_string))
    }
}

async fn get_last_version(url: Url) -> Result<String> {
    let mut iter = url.path().trim_matches('/').split('/');
    let repo_owner = Some(
        iter.next()
            .ok_or(anyhow!("An error occurs in parsing full name 1st part"))?
            .to_string(),
    );
    let repo_name = Some(
        iter.next()
            .ok_or(anyhow!("An error occurs in parsing full name 2nd part"))?
            .to_string(),
    );
    let api_base = Url::parse("https://api.github.com").expect("hardcoded URL should be valid");
    let api = api_base
        .join_all_str([
            "repos",
            repo_owner.as_deref().unwrap(),
            repo_name.as_deref().unwrap(),
            "releases",
            "latest",
        ])
        .expect("Invalid path.");
    println!("Get assets from API: {}", api);
    match REQUEST_CLIENT.get(api).send().await {
        Ok(response) if response.status().is_success() => {
            let releases: serde_json::Value = response.json().await?;
            Ok(releases["tag_name"]
                .as_str()
                .die("cannot found tag_name of this release")
                .to_owned())
        }
        Ok(response) => Err(anyhow!(
            "Invalid assets API response: {}",
            response.status()
        )),
        Err(err) => Err(err.into()),
    }
}
