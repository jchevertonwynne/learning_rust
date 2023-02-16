use std::{path::Path, process::Command};

use anyhow::Context;
use serde::{Deserialize, Serialize};

fn main() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    if !cwd.ends_with("learning_rust") {
        anyhow::bail!("this must be ran from the learning_rust folder");
    }

    let mut info = ProjectsInfo {
        sysroot_src: get_stdlib_path()?,
        crates: vec![
            Crate {
                root_module: get_lib_source("anyhow")?,
                edition: "2021".to_string(),
                deps: vec![],
                cfg: vec!["test".to_string()],
            },
            Crate {
                root_module: get_lib_source("glob")?,
                edition: "2021".to_string(),
                deps: vec![],
                cfg: vec!["test".to_string()],
            },
            Crate {
                root_module: get_lib_source("serde")?,
                edition: "2021".to_string(),
                deps: vec![],
                cfg: vec!["test".to_string()],
            },
            Crate {
                root_module: get_lib_source("serde_json")?,
                edition: "2021".to_string(),
                deps: vec![],
                cfg: vec!["test".to_string()],
            },
            Crate {
                root_module: "src/main.rs".to_string(),
                edition: "2021".to_string(),
                deps: vec![
                    CrateDep {
                        crate_index: 0,
                        name: "anyhow".to_string(),
                    },
                    CrateDep {
                        crate_index: 1,
                        name: "glob".to_string(),
                    },
                    CrateDep {
                        crate_index: 2,
                        name: "serde".to_string(),
                    },
                    CrateDep {
                        crate_index: 3,
                        name: "serde_json".to_string(),
                    },
                ],
                cfg: vec!["test".to_string()],
            },
        ],
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

    // println!("{}", json);

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

fn get_lib_source(lib_name: &str) -> anyhow::Result<String> {
    let stdout = Command::new("cargo").arg("metadata").output()?.stdout;
    let stdout = std::str::from_utf8(&stdout)?;

    let metadata: Metadata = serde_json::from_str(stdout)?;

    let lib_src = metadata
        .packages
        .iter()
        .find(|p| p.name == lib_name)
        .context(format!("failed to find package for lib {}", lib_name))?
        .targets
        .iter()
        .find(|t| t.kind.contains(&String::from("lib")))
        .map(|t| t.src_path.clone())
        .context(format!("failed to find lib src for {}", lib_name))?;
    Ok(lib_src)
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
    deps: Vec<CrateDep>,
    cfg: Vec<String>,
}

#[derive(Serialize)]
struct CrateDep {
    #[serde(rename = "crate")]
    crate_index: usize,
    name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    packages: Vec<Package>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    name: String,
    targets: Vec<Target>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    kind: Vec<String>,
    #[serde(rename = "src_path")]
    src_path: String,
}
