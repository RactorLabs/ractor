use anyhow::Result;
use std::fs;
use std::process;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::api::rest::create_router;
use crate::shared::init_database;

pub async fn run_rest_server() -> Result<()> {
    // Environment variables are loaded via cargo run or runtime

    // Write PID file for process management
    let pid = process::id();
    let pid_file = "/tmp/raworc.pid";

    if let Err(e) = fs::write(pid_file, pid.to_string()) {
        warn!("Could not write PID file: {}", e);
    }

    // Set up cleanup on exit
    let pid_file_cleanup = pid_file.to_string();
    ctrlc::set_handler(move || {
        info!("Shutting down Raworc API...");
        let _ = fs::remove_file(&pid_file_cleanup);
        std::process::exit(0);
    })?;

    // Log startup banner
    info!(
        r#"
 ____                                
|  _ \ __ ___      _____  _ __ ___ 
| |_) / _` \ \ /\ / / _ \| '__/ __|
|  _ < (_| |\ V  V / (_) | | | (__ 
|_| \_\__,_| \_/\_/ \___/|_|  \___|
                                  
Starting Raworc REST API service...
PID: {}
"#,
        pid
    );

    // Initialize database connection and app state
    info!("Connecting to MySQL database...");

    // Get environment variables or use defaults
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://root:root@localhost:3306/raworc".to_string());
    let jwt_secret =
        std::env::var("JWT_SECRET").unwrap_or_else(|_| "development-secret-key".to_string());
    let host = std::env::var("RAWORC_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("RAWORC_PORT").unwrap_or_else(|_| "9000".to_string());

    let app_state = match init_database(&database_url, jwt_secret).await {
        Ok(state) => {
            info!("Connected to database successfully!");
            Arc::new(state)
        }
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            error!("Please ensure MySQL is running and DATABASE_URL is set correctly");
            error!("Example: DATABASE_URL=mysql://user:password@host:port/database");
            return Err(anyhow::anyhow!(
                "Database not available. Please check your configuration."
            ));
        }
    };

    // Build REST router
    info!("Building REST API routes...");
    let app = create_router(app_state);

    // Start server
    let bind_addr = format!("{host}:{port}");
    info!("Binding to: {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    info!("API started successfully!");
    info!("REST API Endpoint: http://{}:{}/api/v0", host, port);
    info!("Ready to accept requests...");

    // Run REST API server
    let rest_server_result = axum::serve(listener, app).await;

    // Clean up PID file on exit
    let _ = fs::remove_file(pid_file);

    // REST server finished; clean up and exit

    rest_server_result?;
    Ok(())
}

