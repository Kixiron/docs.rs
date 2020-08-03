use crate::{db::Pool, error::Result, Config};
use chrono::{DateTime, Utc};
use failure::err_msg;
use log::{debug, warn};
use regex::Regex;
use reqwest::{
    header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT},
    Client,
};
use serde::Deserialize;
use tokio::task;

const APP_USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    " ",
    include_str!(concat!(env!("OUT_DIR"), "/git_version"))
);

/// Fields we need use in cratesfyi
#[derive(Debug)]
struct GitHubFields {
    description: String,
    stars: i64,
    forks: i64,
    issues: i64,
    last_commit: DateTime<Utc>,
}

pub struct GithubUpdater {
    client: Client,
    pool: Pool,
    path_regex: Regex,
}

impl GithubUpdater {
    pub fn new(config: &Config, pool: Pool) -> Result<Self> {
        let github_auth = config.github_auth();

        let mut headers = HeaderMap::with_capacity(2 + github_auth.is_some() as usize);
        headers.insert(USER_AGENT, HeaderValue::from_static(APP_USER_AGENT));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        if let Some((username, accesstoken)) = github_auth {
            let basicauth = format!(
                "Basic {}",
                base64::encode(format!("{}:{}", username, accesstoken))
            );
            headers.insert(AUTHORIZATION, HeaderValue::from_str(&basicauth).unwrap());
        } else {
            warn!("No GitHub authorization specified, will be working with very low rate limits");
        }

        let client = Client::builder().default_headers(headers).build()?;

        Ok(GithubUpdater {
            client,
            pool,
            path_regex: Regex::new(r"https?://github\.com/([\w\._-]+)/([\w\._-]+)").unwrap(),
        })
    }

    /// Updates github fields in crates table
    pub async fn update_all_crates(&self) -> Result<()> {
        debug!("Starting update of all crates");

        if self.is_rate_limited().await? {
            warn!("Skipping update because of rate limit");

            return Ok(());
        }

        // TODO: This query assumes repository field in Cargo.toml is
        //       always the same across all versions of a crate
        let pool = self.pool.clone();
        let rows = task::spawn_blocking::<_, Result<_>>(move || {
            pool.get()?
                .query(
                    "SELECT DISTINCT ON (crates.name)
                            crates.name,
                            crates.id,
                            releases.repository_url
                     FROM crates
                     INNER JOIN releases ON releases.crate_id = crates.id
                     WHERE releases.repository_url ~ '^https?://github.com' AND
                           (crates.github_last_update < NOW() - INTERVAL '1 day' OR
                            crates.github_last_update IS NULL)
                     ORDER BY crates.name, releases.release_time DESC",
                    &[],
                )
                .map_err(Into::into)
        })
        .await??;

        for row in &rows {
            let crate_name: String = row.get(0);
            let crate_id: i32 = row.get(1);
            let repository_url: String = row.get(2);

            debug!("Updating {}", crate_name);

            if let Err(err) = self.update_crate(crate_id, &repository_url).await {
                if self.is_rate_limited().await? {
                    warn!("Skipping remaining updates because of rate limit");

                    return Ok(());
                }

                warn!("Failed to update {}: {}", crate_name, err);
            }
        }

        debug!("Completed all updates");
        Ok(())
    }

    async fn is_rate_limited(&self) -> Result<bool> {
        #[derive(Deserialize)]
        struct Response {
            resources: Resources,
        }

        #[derive(Deserialize)]
        struct Resources {
            core: Resource,
        }

        #[derive(Deserialize)]
        struct Resource {
            remaining: u64,
        }

        const RATE_LIMIT_URL: &str = "https://api.github.com/rate_limit";
        let response: Response = self
            .client
            .get(RATE_LIMIT_URL)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(response.resources.core.remaining == 0)
    }

    // TODO: Pass around a connection with sqlx
    async fn update_crate(&self, crate_id: i32, repository_url: &str) -> Result<()> {
        let path = self
            .get_github_path(repository_url)
            .ok_or_else(|| err_msg("Failed to get github path"))?;
        let fields = self.get_github_fields(&path).await?;

        let pool = self.pool.clone();
        task::spawn_blocking::<_, Result<_>>(move || {
            pool.get()?
                .execute(
                    "UPDATE crates
                 SET github_description = $1,
                     github_stars = $2, github_forks = $3,
                     github_issues = $4, github_last_commit = $5,
                     github_last_update = NOW()
                 WHERE id = $6",
                    &[
                        &fields.description,
                        &(fields.stars as i32),
                        &(fields.forks as i32),
                        &(fields.issues as i32),
                        &fields.last_commit.naive_utc(),
                        &crate_id,
                    ],
                )
                .map_err(Into::into)
        })
        .await??;

        Ok(())
    }

    async fn get_github_fields(&self, path: &str) -> Result<GitHubFields> {
        #[derive(Deserialize)]
        struct Response {
            #[serde(default)]
            description: Option<String>,
            #[serde(default)]
            stargazers_count: i64,
            #[serde(default)]
            forks_count: i64,
            #[serde(default)]
            open_issues: i64,
            #[serde(default = "Utc::now")]
            pushed_at: DateTime<Utc>,
        }

        let url = format!("https://api.github.com/repos/{}", path);
        let response: Response = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(GitHubFields {
            description: response.description.unwrap_or_default(),
            stars: response.stargazers_count,
            forks: response.forks_count,
            issues: response.open_issues,
            last_commit: response.pushed_at,
        })
    }

    fn get_github_path(&self, url: &str) -> Option<String> {
        self.path_regex.captures(url).map(|cap| {
            let username = cap.get(1).unwrap().as_str();
            let reponame = cap.get(2).unwrap().as_str();

            let reponame = if reponame.ends_with(".git") {
                reponame.split(".git").next().unwrap()
            } else {
                reponame
            };

            format!("{}/{}", username, reponame)
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_github_path() {
        let config = Config::from_env().unwrap();
        let updater = GithubUpdater::new(&config, Pool::new(&config).unwrap()).unwrap();

        assert_eq!(
            updater.get_github_path("https://github.com/onur/cratesfyi"),
            Some("onur/cratesfyi".to_string())
        );
        assert_eq!(
            updater.get_github_path("http://github.com/onur/cratesfyi"),
            Some("onur/cratesfyi".to_string())
        );
        assert_eq!(
            updater.get_github_path("https://github.com/onur/cratesfyi.git"),
            Some("onur/cratesfyi".to_string())
        );
        assert_eq!(
            updater.get_github_path("https://github.com/onur23cmD_M_R_L_/crates_fy-i"),
            Some("onur23cmD_M_R_L_/crates_fy-i".to_string())
        );
        assert_eq!(
            updater.get_github_path("https://github.com/docopt/docopt.rs"),
            Some("docopt/docopt.rs".to_string())
        );
    }
}
