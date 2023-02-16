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

    let root = serde_json::from_str::<Root>(stdout)?;

    let package = root
        .packages
        .iter()
        .find(|p| p.name == lib_name)
        .context(format!("failed to find package for lib {}", lib_name))?;
    let lib_src = package
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

use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub packages: Vec<Package>,
    #[serde(rename = "workspace_members")]
    pub workspace_members: Vec<String>,
    pub resolve: Resolve,
    #[serde(rename = "target_directory")]
    pub target_directory: String,
    pub version: i64,
    #[serde(rename = "workspace_root")]
    pub workspace_root: String,
    pub metadata: Value,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub name: String,
    pub version: String,
    pub id: String,
    pub license: Option<String>,
    #[serde(rename = "license_file")]
    pub license_file: Value,
    pub description: Option<String>,
    pub source: Option<String>,
    pub dependencies: Vec<Dependency>,
    pub targets: Vec<Target>,
    pub features: Features,
    #[serde(rename = "manifest_path")]
    pub manifest_path: String,
    pub metadata: Option<Metadata>,
    pub publish: Value,
    pub authors: Vec<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub readme: String,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub edition: String,
    pub links: Value,
    #[serde(rename = "default_run")]
    pub default_run: Value,
    #[serde(rename = "rust_version")]
    pub rust_version: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub name: String,
    pub source: String,
    pub req: String,
    pub kind: Option<String>,
    pub rename: Value,
    pub optional: bool,
    #[serde(rename = "uses_default_features")]
    pub uses_default_features: bool,
    pub features: Vec<String>,
    pub target: Value,
    pub registry: Value,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub kind: Vec<String>,
    #[serde(rename = "crate_types")]
    pub crate_types: Vec<String>,
    pub name: String,
    #[serde(rename = "src_path")]
    pub src_path: String,
    pub edition: String,
    pub doc: bool,
    pub doctest: bool,
    pub test: bool,
    #[serde(rename = "required-features")]
    pub required_features: Option<Vec<String>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Features {
    #[serde(default)]
    pub backtrace: Vec<String>,
    #[serde(default)]
    pub default: Vec<String>,
    #[serde(default)]
    pub std: Vec<String>,
    #[serde(rename = "no-panic")]
    #[serde(default)]
    pub no_panic: Vec<String>,
    #[serde(default)]
    pub nightly: Vec<Value>,
    #[serde(rename = "proc-macro")]
    #[serde(default)]
    pub proc_macro: Vec<String>,
    #[serde(rename = "span-locations")]
    #[serde(default)]
    pub span_locations: Vec<Value>,
    #[serde(default)]
    pub small: Vec<Value>,
    #[serde(default)]
    pub alloc: Vec<String>,
    #[serde(default)]
    pub derive: Vec<String>,
    #[serde(default)]
    pub rc: Vec<Value>,
    #[serde(rename = "serde_derive")]
    #[serde(default)]
    pub serde_derive: Vec<String>,
    #[serde(default)]
    pub unstable: Vec<Value>,
    #[serde(rename = "deserialize_in_place")]
    #[serde(default)]
    pub deserialize_in_place: Vec<Value>,
    #[serde(rename = "arbitrary_precision")]
    #[serde(default)]
    pub arbitrary_precision: Vec<Value>,
    #[serde(rename = "float_roundtrip")]
    #[serde(default)]
    pub float_roundtrip: Vec<Value>,
    #[serde(default)]
    pub indexmap: Vec<String>,
    #[serde(rename = "preserve_order")]
    pub preserve_order: Option<Vec<String>>,
    #[serde(rename = "raw_value")]
    #[serde(default)]
    pub raw_value: Vec<Value>,
    #[serde(rename = "unbounded_depth")]
    #[serde(default)]
    pub unbounded_depth: Vec<Value>,
    #[serde(rename = "clone-impls")]
    #[serde(default)]
    pub clone_impls: Vec<Value>,
    #[serde(rename = "extra-traits")]
    #[serde(default)]
    pub extra_traits: Vec<Value>,
    #[serde(default)]
    pub fold: Vec<Value>,
    #[serde(default)]
    pub full: Vec<Value>,
    #[serde(default)]
    pub parsing: Vec<Value>,
    #[serde(default)]
    pub printing: Vec<String>,
    #[serde(default)]
    pub quote: Vec<String>,
    #[serde(default)]
    pub test: Vec<String>,
    #[serde(default)]
    pub visit: Vec<Value>,
    #[serde(rename = "visit-mut")]
    #[serde(default)]
    pub visit_mut: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub docs: Docs,
    pub playground: Option<Playground>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Docs {
    pub rs: Rs,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rs {
    pub targets: Vec<String>,
    #[serde(rename = "all-features")]
    pub all_features: Option<bool>,
    #[serde(rename = "rustdoc-args")]
    #[serde(default)]
    pub rustdoc_args: Vec<String>,
    pub features: Option<Vec<String>>,
    #[serde(rename = "rustc-args")]
    pub rustc_args: Option<Vec<String>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playground {
    pub features: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolve {
    pub nodes: Vec<Node>,
    pub root: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    pub dependencies: Vec<String>,
    pub deps: Vec<Dep>,
    pub features: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dep {
    pub name: String,
    pub pkg: String,
    #[serde(rename = "dep_kinds")]
    pub dep_kinds: Vec<DepKind>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepKind {
    pub kind: Value,
    pub target: Value,
}
