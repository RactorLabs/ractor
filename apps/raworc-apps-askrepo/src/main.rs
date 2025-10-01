mod config;
mod raworc;
mod twitter;

use anyhow::{bail, Result};
use config::Config;
use raworc::{NewAgentPayload, RaworcClient};
use tokio::signal;
use tokio::time::{self, MissedTickBehavior};
use tracing::{debug, error, info, warn};
use twitter::TwitterClient;

use serde_json::json;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    let config = Config::from_env()?;
    info!("starting raworc-apps-askrepo");

    let twitter_client = TwitterClient::new(&config)?;
    let raworc_client = RaworcClient::new(&config)?;

    let mut since_id = config.initial_since_id.clone();

    match process_mentions_cycle(
        &twitter_client,
        &raworc_client,
        since_id.as_deref(),
        &config,
    )
    .await
    {
        Ok(new_id) => {
            if let Some(id) = new_id {
                since_id = Some(id);
            }
        }
        Err(err) => {
            error!(?err, "initial poll failed");
        }
    }

    let mut ticker = time::interval(config.poll_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("received shutdown signal; exiting");
                break;
            }
            _ = ticker.tick() => {
                match process_mentions_cycle(&twitter_client, &raworc_client, since_id.as_deref(), &config).await {
                    Ok(new_id) => {
                        if let Some(id) = new_id {
                            since_id = Some(id);
                        }
                    }
                    Err(err) => {
                        error!(?err, "polling cycle failed");
                    }
                }
            }
        }
    }

    Ok(())
}

async fn process_mentions_cycle(
    twitter: &TwitterClient,
    raworc: &RaworcClient,
    since_id: Option<&str>,
    config: &Config,
) -> Result<Option<String>> {
    let mut tweets = twitter.fetch_mentions(since_id).await?;
    if tweets.is_empty() {
        debug!("no new mentions returned by Twitter");
        return Ok(None);
    }

    let mut tweets_with_ids = Vec::new();
    for tweet in tweets.drain(..) {
        match tweet.numeric_id() {
            Ok(id) => tweets_with_ids.push((id, tweet)),
            Err(err) => {
                warn!(?err, "skipping tweet with non-numeric id");
            }
        }
    }

    if tweets_with_ids.is_empty() {
        return Ok(None);
    }

    tweets_with_ids.sort_by_key(|(id, _)| *id);

    let mut last_success_id = since_id.and_then(|id| id.parse::<u64>().ok());
    let mut had_error = false;

    for (numeric_id, tweet) in tweets_with_ids {
        match ensure_agent_for_tweet(raworc, &tweet, config).await {
            Ok(_) => {
                let updated = Some(last_success_id.map_or(numeric_id, |curr| curr.max(numeric_id)));
                last_success_id = updated;
            }
            Err(err) => {
                error!(?err, tweet_id = %tweet.id, "failed to ensure agent for tweet");
                had_error = true;
            }
        }
    }

    if had_error {
        bail!("one or more tweets failed to process");
    }

    Ok(last_success_id.map(|id| id.to_string()))
}

async fn ensure_agent_for_tweet(
    raworc: &RaworcClient,
    tweet: &twitter::Tweet,
    config: &Config,
) -> Result<()> {
    let tweet_id = tweet.id.as_str();
    let agent_name = format!("tweet-{}", tweet_id);
    if raworc.agent_exists(&agent_name).await? {
        debug!(agent = %agent_name, "agent already exists; skipping creation");
        return Ok(());
    }

    let metadata = json!({
        "source": "askrepo",
        "tweet": {
            "id": tweet_id,
            "text": tweet.text.clone(),
            "author_id": tweet.author_id.clone(),
            "conversation_id": tweet.conversation_id.clone(),
            "created_at": tweet.created_at.clone(),
        }
    });

    let tags = vec![
        "askrepo".to_string(),
        "twitter".to_string(),
        format!("tweet{}", tweet_id),
    ];

    let prompt = build_initial_prompt(tweet);
    let instructions_overview = "You are AskRepo. Review repository questions sourced from Twitter mentions and follow the initial task details.".to_string();

    let secrets = config.agent_secrets();

    let payload = NewAgentPayload::new(agent_name.clone(), metadata)
        .with_description(Some(format!(
            "AskRepo agent bootstrap for tweet {}",
            tweet_id
        )))
        .with_tags(tags)
        .with_instructions(instructions_overview)
        .with_prompt(prompt)
        .with_idle_timeout(Some(900))
        .with_busy_timeout(Some(1800))
        .with_secrets(secrets);

    raworc.create_agent(&payload).await?;
    info!(agent = %agent_name, "created new AskRepo agent");
    Ok(())
}

fn build_initial_prompt(tweet: &twitter::Tweet) -> String {
    let tweet_id = tweet.id.as_str();
    format!(
        r#"You are AskRepo, a RAWORC code-review assistant responding to Twitter mention {tweet_id}.

Tweet details:
- id: {tweet_id}

Task:
- Read the conversation referenced by {tweet_id}.
- Clone https://github.com/raworc/twitter_api_client and run `twitter_api_client/get-tweet.py` to pull the full thread content.
- Apply guardrails: only proceed if the user is asking about a software repository *and* the thread contains a repository URL or explicit owner/repo reference. Otherwise, explain why the request is skipped.
- Identify the repository in question from the thread, clone it locally, and inspect the codebase.
- Craft a precise, evidence-based answer addressing the user's question using repository context, and present the explanation in well-structured paragraphs (no bullet dumps) of two to three sentences each so it reads like a thoughtful write-up.
- Keep the final tweet at 280 characters or fewer; shorten language ahead of time so the post fits without truncation.
- Post the final response back to the original tweet through the twitter_api_client tooling. If the post attempt returns HTTP 403, shorten the text (for example, drop the last sentence) and retry once.
- Do not cite or list file paths directly in the tweet; summarize findings instead."#,
        tweet_id = tweet_id
    )
}
