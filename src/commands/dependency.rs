use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use colored::Colorize;
use dialoguer::{Confirm, Input, MultiSelect};
use std::path::PathBuf;
use std::fs;
use std::process::Command;

#[derive(Debug, Args)]
pub struct DependencyArgs {
    #[command(subcommand)]
    command: DependencyCommands,
}

#[derive(Debug, Subcommand)]
pub enum DependencyCommands {
    /// Add dependencies to your project
    Add(AddArgs),
    
    /// Remove dependencies from your project
    Remove(RemoveArgs),
    
    /// Update dependencies in your project
    Update(UpdateArgs),
    
    /// Analyze dependencies in your project
    Analyze(AnalyzeArgs),
}

#[derive(Debug, Args)]
pub struct AddArgs {
    /// Dependencies to add (e.g., serde, tokio, etc.)
    #[arg(required = false)]
    pub dependencies: Vec<String>,
    
    /// Add as development dependency
    #[arg(short, long)]
    pub dev: bool,
    
    /// Add with specific features (comma separated)
    #[arg(short, long)]
    pub features: Option<String>,
    
    /// Add with specific version
    #[arg(short, long)]
    pub version: Option<String>,
    
    /// Path to the project (defaults to current directory)
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct RemoveArgs {
    /// Dependencies to remove
    #[arg(required = false)]
    pub dependencies: Vec<String>,
    
    /// Path to the project (defaults to current directory)
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Dependencies to update (if empty, updates all)
    #[arg(required = false)]
    pub dependencies: Vec<String>,
    
    /// Path to the project (defaults to current directory)
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AnalyzeArgs {
    /// Path to the project (defaults to current directory)
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

/// Execute the dependency command
pub fn execute(args: DependencyArgs) -> Result<()> {
    match args.command {
        DependencyCommands::Add(args) => add_dependencies(args),
        DependencyCommands::Remove(args) => remove_dependencies(args),
        DependencyCommands::Update(args) => update_dependencies(args),
        DependencyCommands::Analyze(args) => analyze_dependencies(args),
    }
}

/// Add dependencies to a project
pub fn add_dependencies(args: AddArgs) -> Result<()> {
    let project_dir = args.path.unwrap_or_else(|| PathBuf::from("."));
    
    // Verify this is a Rust project
    let cargo_toml_path = project_dir.join("Cargo.toml");
    if !cargo_toml_path.exists() {
        return Err(anyhow::anyhow!("No Cargo.toml found in the specified directory. Are you sure this is a Rust project?"));
    }
    
    // If no dependencies were provided, prompt for them
    let dependencies = if args.dependencies.is_empty() {
        let input: String = Input::new()
            .with_prompt("Enter dependencies to add (comma separated)")
            .interact_text()?;
        
        input.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        args.dependencies
    };
    
    if dependencies.is_empty() {
        return Err(anyhow::anyhow!("No dependencies specified"));
    }
    
    // For each dependency, prompt for features if not provided
    for dependency in &dependencies {
        let mut cargo_add_args = vec!["add", dependency];
        
        // Handle dev dependencies
        if args.dev {
            cargo_add_args.push("--dev");
        }
        
        // Handle features
        if let Some(features) = &args.features {
            cargo_add_args.push("--features");
            cargo_add_args.push(features);
        } else {
            // Suggest popular features for common crates
            suggest_features(dependency, &mut cargo_add_args)?;
        }
        
        // Handle version
        if let Some(version) = &args.version {
            cargo_add_args.push("--version");
            cargo_add_args.push(version);
        }
        
        // Run cargo add
        println!("{} {}", "Adding dependency:".green(), dependency);
        
        let output = Command::new("cargo")
            .args(&cargo_add_args)
            .current_dir(&project_dir)
            .output()
            .context(format!("Failed to add dependency {}", dependency))?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to add dependency {}: {}", dependency, error));
        }
        
        println!("{} {}", "Successfully added:".green(), dependency);
    }
    
    Ok(())
}

/// Suggest popular features for common crates
fn suggest_features(dependency: &str, cargo_add_args: &mut Vec<&str>) -> Result<()> {
    // Map of common crates and their popular features
    let features_map = match dependency {
        "tokio" => Some(vec!["full", "rt", "rt-multi-thread", "macros", "io-util", "time"]),
        "serde" => Some(vec!["derive"]),
        "reqwest" => Some(vec!["json", "blocking", "rustls-tls", "cookies", "gzip"]),
        "axum" => Some(vec!["headers", "http2", "macros", "multipart", "ws"]),
        "diesel" => Some(vec!["postgres", "mysql", "sqlite", "r2d2", "chrono"]),
        "sqlx" => Some(vec!["runtime-tokio-rustls", "postgres", "mysql", "sqlite", "macros"]),
        "clap" => Some(vec!["derive", "cargo", "env", "wrap_help"]),
        _ => None,
    };
    
    if let Some(suggested_features) = features_map {
        println!("Suggested features for {}:", dependency.green());
        
        let selections = MultiSelect::new()
            .items(&suggested_features)
            .interact()?;
        
        if !selections.is_empty() {
            let selected_features = selections.iter()
                .map(|&i| suggested_features[i])
                .collect::<Vec<_>>()
                .join(",");
            
            cargo_add_args.push("--features");
            // Create a static string to avoid the lifetime issue
            let features_str = Box::leak(selected_features.into_boxed_str());
            cargo_add_args.push(features_str);
        }
    }
    
    Ok(())
}

/// Remove dependencies from a project
pub fn remove_dependencies(args: RemoveArgs) -> Result<()> {
    let project_dir = args.path.unwrap_or_else(|| PathBuf::from("."));
    
    // Verify this is a Rust project
    let cargo_toml_path = project_dir.join("Cargo.toml");
    if !cargo_toml_path.exists() {
        return Err(anyhow::anyhow!("No Cargo.toml found in the specified directory. Are you sure this is a Rust project?"));
    }
    
    // If no dependencies were provided, list current dependencies and prompt
    let dependencies = if args.dependencies.is_empty() {
        // Parse Cargo.toml to get current dependencies
        let cargo_toml_content = fs::read_to_string(&cargo_toml_path)
            .context("Failed to read Cargo.toml")?;
        
        let cargo_toml: toml::Value = toml::from_str(&cargo_toml_content)
            .context("Failed to parse Cargo.toml")?;
        
        let mut all_deps = Vec::new();
        
        // Get regular dependencies
        if let Some(deps) = cargo_toml.get("dependencies").and_then(|d| d.as_table()) {
            all_deps.extend(deps.keys().map(|k| k.to_string()));
        }
        
        // Get dev-dependencies
        if let Some(deps) = cargo_toml.get("dev-dependencies").and_then(|d| d.as_table()) {
            all_deps.extend(deps.keys().map(|k| k.to_string()));
        }
        
        // Get build-dependencies
        if let Some(deps) = cargo_toml.get("build-dependencies").and_then(|d| d.as_table()) {
            all_deps.extend(deps.keys().map(|k| k.to_string()));
        }
        
        if all_deps.is_empty() {
            return Err(anyhow::anyhow!("No dependencies found in the project"));
        }
        
        // Prompt user to select dependencies to remove
        let selections = MultiSelect::new()
            .with_prompt("Select dependencies to remove")
            .items(&all_deps)
            .interact()?;
        
        if selections.is_empty() {
            return Ok(());
        }
        
        selections.into_iter()
            .map(|i| all_deps[i].clone())
            .collect()
    } else {
        args.dependencies
    };
    
    // Remove each dependency
    for dependency in &dependencies {
        println!("{} {}", "Removing dependency:".yellow(), dependency);
        
        let output = Command::new("cargo")
            .args(["remove", dependency])
            .current_dir(&project_dir)
            .output()
            .context(format!("Failed to remove dependency {}", dependency))?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to remove dependency {}: {}", dependency, error));
        }
        
        println!("{} {}", "Successfully removed:".green(), dependency);
    }
    
    Ok(())
}

/// Update dependencies in a project
pub fn update_dependencies(args: UpdateArgs) -> Result<()> {
    let project_dir = args.path.unwrap_or_else(|| PathBuf::from("."));
    
    // Verify this is a Rust project
    let cargo_toml_path = project_dir.join("Cargo.toml");
    if !cargo_toml_path.exists() {
        return Err(anyhow::anyhow!("No Cargo.toml found in the specified directory. Are you sure this is a Rust project?"));
    }
    
    // If specific dependencies were provided, update only those
    if !args.dependencies.is_empty() {
        for dependency in &args.dependencies {
            println!("{} {}", "Updating dependency:".blue(), dependency);
            
            let output = Command::new("cargo")
                .args(["update", dependency])
                .current_dir(&project_dir)
                .output()
                .context(format!("Failed to update dependency {}", dependency))?;
            
            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("Failed to update dependency {}: {}", dependency, error));
            }
            
            println!("{} {}", "Successfully updated:".green(), dependency);
        }
    } else {
        // Update all dependencies
        println!("{}", "Updating all dependencies...".blue());
        
        let output = Command::new("cargo")
            .args(["update"])
            .current_dir(&project_dir)
            .output()
            .context("Failed to update dependencies")?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to update dependencies: {}", error));
        }
        
        println!("{}", "Successfully updated all dependencies".green());
    }
    
    Ok(())
}

/// Analyze dependencies in a project
pub fn analyze_dependencies(args: AnalyzeArgs) -> Result<()> {
    let project_dir = args.path.unwrap_or_else(|| PathBuf::from("."));
    
    // Verify this is a Rust project
    let cargo_toml_path = project_dir.join("Cargo.toml");
    if !cargo_toml_path.exists() {
        return Err(anyhow::anyhow!("No Cargo.toml found in the specified directory. Are you sure this is a Rust project?"));
    }
    
    println!("{}", "Analyzing dependencies...".blue());
    
    // Check if cargo-audit is installed
    let audit_installed = Command::new("cargo")
        .args(["audit", "--version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    
    if !audit_installed {
        println!("{}", "cargo-audit is not installed. It's recommended for security analysis.".yellow());
        if Confirm::new()
            .with_prompt("Would you like to install cargo-audit?")
            .interact()?
        {
            println!("{}", "Installing cargo-audit...".blue());
            
            let output = Command::new("cargo")
                .args(["install", "cargo-audit"])
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .status()
                .context("Failed to install cargo-audit")?;
            
            if !output.success() {
                println!("{} Check your internet connection and try again", "Failed to install cargo-audit:".red());
            } else {
                println!("{}", "Successfully installed cargo-audit".green());
            }
        }
    }
    
    // Run cargo tree
    println!("\n{}", "Dependency tree:".blue());
    let tree_output = Command::new("cargo")
        .args(["tree"])
        .current_dir(&project_dir)
        .output()
        .context("Failed to run cargo tree")?;
    
    println!("{}", String::from_utf8_lossy(&tree_output.stdout));
    
    // Run cargo audit if available
    if audit_installed || Command::new("cargo")
        .args(["audit", "--version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
    {
        println!("\n{}", "Security audit:".blue());
        let audit_output = Command::new("cargo")
            .args(["audit"])
            .current_dir(&project_dir)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .context("Failed to run cargo audit")?;
        
        if !audit_output.success() {
            println!("{}", "Security vulnerabilities found in your dependencies. Please review and update.".yellow());
        }
    }
    
    Ok(())
}
