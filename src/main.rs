use std::{path::Path, process::Command};

use anyhow::Context;
use serde::Serialize;

fn main() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    if !cwd.ends_with("learning_rust") {
        anyhow::bail!("this must be ran from the learning_rust folder");
    }

    let mut info = ProjectsInfo {
        sysroot_src: get_stdlib_path()?,
        crates: vec![Crate {
            root_module: "src/main.rs".to_string(),
            edition: "2021".to_string(),
            deps: vec![],
            cfg: vec!["test".to_string()],
        }],
    };

    for file in glob::glob("course/*/*/*/main.rs")? {
        let path = file?
            .as_os_str()
            .to_str()
            .context("path contained non utf8 characters")?
            .to_string();
        
        info.crates.push(Crate {
            root_module: path,
            edition: "2021".to_string(),
            deps: vec![],
            cfg: vec!["test".to_string()],
        });
    }

    let json = serde_json::to_string_pretty(&info)?;

    std::fs::write("rust-project.json", json)?;

    Ok(())
}

fn get_stdlib_path() -> anyhow::Result<String> {
    let stdout = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()?
        .stdout;

    let toolchain = std::str::from_utf8(&stdout)?.trim();

    let stdlib_path = Path::new(&toolchain)
        .join("lib")
        .join("rustlib")
        .join("src")
        .join("rust")
        .join("library")
        .to_str()
        .context("path contained non utf8 characters")?
        .to_string();

    Ok(stdlib_path)
}

#[derive(Serialize)]
struct ProjectsInfo {
    sysroot_src: String,
    crates: Vec<Crate>,
}

#[derive(Serialize)]
struct Crate {
    root_module: String,
    edition: String,
    deps: Vec<Dep>,
    cfg: Vec<String>,
}

#[derive(Serialize)]
struct Dep {}
