use std::path::Path;
use std::process::Command;

fn main() {
    // Skip dashboard build in docs.rs
    if std::env::var("DOCS_RS").is_ok() {
        return;
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = Path::new(&manifest_dir).parent().unwrap();
    let dashboard_dir = workspace_root.join("dashboard");
    let dist_dir = dashboard_dir.join("dist");

    // Tell Cargo to rerun this if dashboard source changes
    println!("cargo:rerun-if-changed={}/src", dashboard_dir.display());
    println!("cargo:rerun-if-changed={}/index.html", dashboard_dir.display());
    println!("cargo:rerun-if-changed={}/package.json", dashboard_dir.display());
    println!("cargo:rerun-if-changed={}/vite.config.ts", dashboard_dir.display());

    // Skip if SKIP_DASHBOARD_BUILD is set
    if std::env::var("SKIP_DASHBOARD_BUILD").is_ok() {
        println!("cargo:warning=Skipping dashboard build (SKIP_DASHBOARD_BUILD is set)");
        return;
    }

    // Skip if dist already exists in dev mode
    if dist_dir.exists() {
        let profile = std::env::var("PROFILE").unwrap_or_default();
        if profile != "release" {
            println!("cargo:warning=Dashboard already built, skipping (delete dist/ to rebuild)");
            return;
        }
    }

    // Check if npm is available
    if Command::new("npm").arg("--version").output().is_err() {
        println!("cargo:warning=npm not found, skipping dashboard build");
        return;
    }

    // Check if package.json exists
    if !dashboard_dir.join("package.json").exists() {
        println!("cargo:warning=dashboard/package.json not found, skipping dashboard build");
        return;
    }

    // Check if node_modules exists, if not run npm install
    let node_modules = dashboard_dir.join("node_modules");
    if !node_modules.exists() {
        println!("cargo:warning=Installing dashboard dependencies...");
        let status = Command::new("npm")
            .arg("install")
            .current_dir(&dashboard_dir)
            .status()
            .expect("Failed to run npm install");

        if !status.success() {
            println!("cargo:warning=npm install failed, skipping dashboard build");
            return;
        }
    }

    // Build the dashboard
    println!("cargo:warning=Building dashboard...");
    let status = Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir(&dashboard_dir)
        .status()
        .expect("Failed to run npm build");

    if !status.success() {
        println!("cargo:warning=Dashboard build failed");
        return;
    }

    println!("cargo:warning=Dashboard built successfully at {:?}", dist_dir);
}
