use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use reqwest::{Client, Url};
use serde::Deserialize;
use std::collections::HashSet;
use tracing::{debug, trace, warn};

#[derive(Debug, Clone, Deserialize)]
pub struct Tweet {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub author_id: Option<String>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

impl Tweet {
    pub fn numeric_id(&self) -> Result<u64> {
        self.id
            .parse::<u64>()
            .with_context(|| format!("tweet id '{}' is not a number", self.id))
    }
}

#[derive(Debug, Deserialize)]
struct MentionsResponse {
    #[serde(default)]
    data: Vec<Tweet>,
    #[serde(default)]
    meta: ResponseMeta,
}

#[derive(Debug, Default, Deserialize)]
struct ResponseMeta {
    #[serde(default)]
    next_token: Option<String>,
    #[serde(default)]
    _newest_id: Option<String>,
    #[serde(default)]
    _result_count: Option<u64>,
}

pub struct TwitterClient {
    http: Client,
    base_url: Url,
    user_id: String,
    bearer_token: String,
}

impl TwitterClient {
    pub fn new(config: &Config) -> Result<Self> {
        let http = Client::builder()
            .user_agent("raworc-apps-askrepo/0.1")
            .build()
            .context("failed to build twitter reqwest client")?;

        let mut base_url = Url::parse(&config.twitter_api_base)
            .context("RAWORC_APPS_ASKREPO_TWITTER_API_BASE is not a valid URL")?;
        if base_url.path().is_empty() || base_url.path() == "/" {
            base_url.set_path("/");
        }

        Ok(Self {
            http,
            base_url,
            user_id: config.twitter_user_id.clone(),
            bearer_token: config.twitter_bearer_token.clone(),
        })
    }

    pub async fn fetch_mentions(&self, since_id: Option<&str>) -> Result<Vec<Tweet>> {
        let mut collected: Vec<Tweet> = Vec::new();
        let mut next_token: Option<String> = None;
        let mut seen_ids: HashSet<String> = HashSet::new();

        loop {
            let mut url = self
                .base_url
                .join(&format!("2/users/{}/mentions", self.user_id))
                .context("failed to build Twitter mentions URL")?;

            {
                let mut query = url.query_pairs_mut();
                query.append_pair("tweet.fields", "author_id,conversation_id,created_at");
                query.append_pair("max_results", "100");
                if let Some(id) = since_id {
                    query.append_pair("since_id", id);
                }
                if let Some(token) = next_token.as_deref() {
                    query.append_pair("pagination_token", token);
                }
            }

            trace!(since_id = ?since_id, pagination = ?next_token, "fetching Twitter mentions page");

            let response = self
                .http
                .get(url.clone())
                .bearer_auth(&self.bearer_token)
                .send()
                .await
                .with_context(|| format!("failed to call Twitter mentions endpoint: {}", url))?;

            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let headers = response.headers().clone();
                warn!(retry_after = ?headers.get("retry-after"), "twitter rate limit hit");
                return Err(anyhow!("rate limited by Twitter API"));
            }

            if !response.status().is_success() {
                let status = response.status();
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "<unable to read body>".to_string());
                return Err(anyhow!(
                    "Twitter API returned {} for mentions request (body: {})",
                    status,
                    body
                ));
            }

            let payload: MentionsResponse = response
                .json()
                .await
                .context("failed to parse Twitter mentions response JSON")?;

            for tweet in payload.data {
                if seen_ids.insert(tweet.id.clone()) {
                    trace!(tweet_id = %tweet.id, "collected mention");
                    collected.push(tweet);
                }
            }

            if let Some(meta) = payload.meta.next_token {
                next_token = Some(meta);
                continue;
            }

            debug!(count = collected.len(), "fetched mentions from Twitter");
            break;
        }

        Ok(collected)
    }
}
