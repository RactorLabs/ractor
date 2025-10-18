use anyhow::Result;
use async_trait::async_trait;

use super::tool_registry::Tool;
use super::tools::run_bash;

/// Tool for checking and installing Python packages
pub struct PythonPackageTool;

#[async_trait]
impl Tool for PythonPackageTool {
    fn name(&self) -> &str {
        "python_package"
    }

    fn description(&self) -> &str {
        "Check if Python packages are available and install them if needed"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["check", "install", "check_and_install"],
                    "description": "Action to perform: check availability, install packages, or both"
                },
                "packages": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of Python package names to check/install"
                },
                "upgrade": {
                    "type": "boolean",
                    "description": "Whether to upgrade packages if they exist (default: false)"
                }
            },
            "required": ["action", "packages"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("check");
        let packages = args.get("packages").and_then(|v| v.as_array());
        let upgrade = args.get("upgrade").and_then(|v| v.as_bool()).unwrap_or(false);

        if packages.is_none() {
            return Ok(serde_json::json!({"status":"error","tool":"python_package","error":"No packages specified"}));
        }

        let packages: Vec<String> = packages.unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.to_string())
            .collect();

        if packages.is_empty() {
            return Ok(serde_json::json!({"status":"error","tool":"python_package","error":"No valid packages specified"}));
        }

        match action {
            "check" => check_packages(&packages).await,
            "install" => install_packages(&packages, upgrade).await,
            "check_and_install" => check_and_install_packages(&packages, upgrade).await,
            _ => Ok(serde_json::json!({"status":"error","tool":"python_package","error":"Invalid action"})),
        }
    }
}

async fn check_packages(packages: &[String]) -> Result<serde_json::Value> {
    let mut results = Vec::new();
    
    for package in packages {
        let cmd = format!("python -c \"import {}\" 2>/dev/null && echo \"{}:available\" || echo \"{}:missing\"", 
                         package, package, package);
        match run_bash(&cmd).await {
            Ok(output) => results.push(output.trim().to_string()),
            Err(e) => results.push(format!("{}:error - {}", package, e)),
        }
    }
    
    Ok(serde_json::json!({"status":"ok","tool":"python_package","action":"check","output": results.join("\n")}))
}

async fn install_packages(packages: &[String], upgrade: bool) -> Result<serde_json::Value> {
    let upgrade_flag = if upgrade { " --upgrade" } else { "" };
    let packages_str = packages.join(" ");
    let cmd = format!("pip install{} {}", upgrade_flag, packages_str);
    
    match run_bash(&cmd).await {
        Ok(output) => Ok(serde_json::json!({"status":"ok","tool":"python_package","action":"install","output": output})),
        Err(e) => Ok(serde_json::json!({"status":"error","tool":"python_package","action":"install","error": e.to_string()})),
    }
}

async fn check_and_install_packages(packages: &[String], upgrade: bool) -> Result<serde_json::Value> {
    // First check which packages are missing
    let mut missing_packages = Vec::new();
    let mut available_packages = Vec::new();
    
    for package in packages {
        let cmd = format!("python -c \"import {}\" 2>/dev/null", package);
        match run_bash(&cmd).await {
            Ok(_) => available_packages.push(package.clone()),
            Err(_) => missing_packages.push(package.clone()),
        }
    }
    
    let mut result = Vec::new();
    
    if !available_packages.is_empty() {
        result.push(format!("Available packages: {}", available_packages.join(", ")));
    }
    
    if !missing_packages.is_empty() {
        result.push(format!("Missing packages: {}", missing_packages.join(", ")));
        
        // Install missing packages
        let upgrade_flag = if upgrade { " --upgrade" } else { "" };
        let packages_str = missing_packages.join(" ");
        let cmd = format!("pip install{} {}", upgrade_flag, packages_str);
        
        match run_bash(&cmd).await {
            Ok(output) => {
                result.push("Installation output:".to_string());
                result.push(output);
            },
            Err(e) => result.push(format!("Installation failed: {}", e)),
        }
    } else {
        result.push("All packages are already available.".to_string());
    }
    
    Ok(serde_json::json!({"status":"ok","tool":"python_package","action":"check_and_install","output": result.join("\n")}))
}

