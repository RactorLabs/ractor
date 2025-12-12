use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use sqlx::{mysql::MySqlPoolOptions, MySql, Pool};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct McpState {
    pub db: Arc<Pool<MySql>>,
    pub http: Client,
    pub sessions: Arc<Mutex<HashMap<String, String>>>,
}

impl McpState {
    pub fn new(db: Pool<MySql>) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build http client");
        Self {
            db: Arc::new(db),
            http,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

pub async fn init_pool(database_url: &str) -> anyhow::Result<Pool<MySql>> {
    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    Ok(pool)
}
