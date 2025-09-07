use anyhow::Result;
use async_trait::async_trait;

use super::tool_registry::Tool;
use super::tools::run_bash;

/// Tool for getting information about the execution environment
pub struct EnvironmentInfoTool;

#[async_trait]
impl Tool for EnvironmentInfoTool {
    fn name(&self) -> &str {
        "environment_info"
    }

    fn description(&self) -> &str {
        "Get information about the execution environment, including available packages and system info"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "info_type": {
                    "type": "string",
                    "enum": ["python_packages", "system", "python_version", "pip_list", "all"],
                    "description": "Type of information to retrieve"
                }
            },
            "required": ["info_type"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let info_type = args.get("info_type").and_then(|v| v.as_str()).unwrap_or("all");

        match info_type {
            "python_packages" => get_python_packages().await,
            "system" => get_system_info().await,
            "python_version" => get_python_version().await,
            "pip_list" => get_pip_list().await,
            "all" => get_all_info().await,
            _ => Ok("[environment_info error] Invalid info_type".to_string()),
        }
    }
}

async fn get_python_packages() -> Result<String> {
    let common_packages = [
        "os", "sys", "json", "re", "datetime", "time", "math", "random",
        "urllib", "http", "pathlib", "collections", "itertools", "functools",
        "subprocess", "threading", "multiprocessing", "sqlite3", "csv",
    ];

    let mut results = Vec::new();
    results.push("Built-in Python packages (always available):".to_string());
    
    for package in &common_packages {
        results.push(format!("  - {}", package));
    }

    // Check for commonly used third-party packages
    let third_party = [
        "requests", "numpy", "pandas", "matplotlib", "seaborn", "plotly",
        "scipy", "sklearn", "tensorflow", "torch", "PIL", "cv2",
        "yfinance", "beautifulsoup4", "lxml", "openpyxl", "xlrd",
    ];

    results.push("\nChecking third-party packages:".to_string());
    
    for package in &third_party {
        let cmd = format!("python -c \"import {}\" 2>/dev/null && echo \"  ✓ {}\" || echo \"  ✗ {} (not installed)\"", 
                         package, package, package);
        match run_bash(&cmd).await {
            Ok(output) => results.push(output.trim().to_string()),
            Err(_) => results.push(format!("  ✗ {} (not installed)", package)),
        }
    }

    Ok(format!("[environment_info python_packages]\n{}", results.join("\n")))
}

async fn get_system_info() -> Result<String> {
    let cmd = "uname -a && echo && python --version && echo && which python && echo && which pip";
    match run_bash(cmd).await {
        Ok(output) => Ok(format!("[environment_info system]\n{}", output)),
        Err(e) => Ok(format!("[environment_info system error] {}", e)),
    }
}

async fn get_python_version() -> Result<String> {
    let cmd = "python --version && python -c \"import sys; print(f'Python executable: {sys.executable}'); print(f'Python path: {sys.path[:3]}...')\"";
    match run_bash(cmd).await {
        Ok(output) => Ok(format!("[environment_info python_version]\n{}", output)),
        Err(e) => Ok(format!("[environment_info python_version error] {}", e)),
    }
}

async fn get_pip_list() -> Result<String> {
    let cmd = "pip list --format=columns 2>/dev/null | head -20";
    match run_bash(cmd).await {
        Ok(output) => Ok(format!("[environment_info pip_list]\n{}", output)),
        Err(e) => Ok(format!("[environment_info pip_list error] {}", e)),
    }
}

async fn get_all_info() -> Result<String> {
    let system = get_system_info().await?;
    let python_version = get_python_version().await?;
    let packages = get_python_packages().await?;
    let pip_list = get_pip_list().await?;

    Ok(format!("{}\n\n{}\n\n{}\n\n{}", system, python_version, packages, pip_list))
}