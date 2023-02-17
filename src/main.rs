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
        crates: vec![],
    };

    let mut lrproject_info = Crate {
        root_module: "src/main.rs".to_string(),
        edition: "2021".to_string(),
        deps: vec![],
        cfg: vec!["test".to_string()],
    };

    let stdout = Command::new("cargo").arg("metadata").output()?.stdout;
    let stdout = std::str::from_utf8(&stdout)?;
    let metadata: Metadata = serde_json::from_str(stdout)?;
    let manifest = cargo_toml::Manifest::from_slice(&std::fs::read("Cargo.toml")?)?;
    for (i, (dep, _)) in manifest.dependencies.into_iter().enumerate() {
        let lib_src = get_lib_source(&dep, &metadata)?;
        info.crates.push(Crate {
            root_module: lib_src,
            edition: "2021".to_string(),
            deps: vec![],
            cfg: vec![],
        });
        lrproject_info.deps.push(CrateDep {
            crate_index: i,
            name: dep,
        });
    }

    info.crates.push(lrproject_info);

    for file in glob::glob("course/*/*/src/main.rs")? {
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

    if std::env::var("TEST").is_ok() {
        println!("{}", json);
    } else {
        std::fs::write("rust-project.json", json)?;
    }

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

fn get_lib_source(lib_name: &str, metadata: &Metadata) -> anyhow::Result<String> {
    let lib_src = metadata
        .packages
        .iter()
        .find(|p| p.name == lib_name)
        .context(format!("failed to find package for lib {}", lib_name))?
        .targets
        .iter()
        .find_map(|t| {
            if t.kind.iter().any(|k| k == "lib") {
                Some(t.src_path.clone())
            } else {
                None
            }
        })
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
pub struct Metadata {
    packages: Vec<Package>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Package {
    name: String,
    targets: Vec<Target>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Target {
    kind: Vec<String>,
    src_path: String,
}
