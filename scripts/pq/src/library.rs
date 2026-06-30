use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::macros::MacroFile;
use crate::role::{parse_role, Role};

#[derive(Debug, Clone)]
pub(crate) struct Library {
    root: PathBuf,
}

impl Library {
    pub(crate) fn resolve(flag_dir: Option<&Path>) -> Result<Self> {
        if let Some(dir) = flag_dir {
            return Self::from_existing(dir, "--dir");
        }

        if let Ok(dir) = env::var("PQ_DIR") {
            return Self::from_existing(Path::new(&dir), "PQ_DIR");
        }

        if let Ok(current_exe) = env::current_exe() {
            if let Some(dir) = current_exe.parent() {
                if Self::looks_like_library(dir) {
                    return Ok(Self { root: dir.into() });
                }
            }
        }

        if let Ok(current_dir) = env::current_dir() {
            if Self::looks_like_library(&current_dir) {
                return Ok(Self { root: current_dir });
            }
        }

        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        if Self::looks_like_library(manifest_dir) {
            return Ok(Self {
                root: manifest_dir.into(),
            });
        }

        if let Some(config_dir) = dirs::config_dir() {
            let dir = config_dir.join("pq");
            if Self::looks_like_library(&dir) {
                return Ok(Self { root: dir });
            }
        }

        bail!("could not find a pq library; pass --dir, set PQ_DIR, or create ~/.config/pq")
    }

    #[cfg(test)]
    pub(crate) fn from_root(root: PathBuf) -> Self {
        Self { root }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn role_path(&self, name: &str) -> PathBuf {
        self.roles_dir().join(format!("{name}.md"))
    }

    pub(crate) fn macro_path(&self, name: &str) -> PathBuf {
        self.macros_dir().join(format!("{name}.yaml"))
    }

    pub(crate) fn load_role(&self, name: &str) -> Result<Role> {
        let path = self.role_path(name);
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read role {}", path.display()))?;
        parse_role(name, path, &raw)
    }

    pub(crate) fn load_macro(&self, name: &str) -> Result<MacroFile> {
        let path = self.macro_path(name);
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read macro {}", path.display()))?;
        serde_yml::from_str(&raw).with_context(|| format!("failed to parse {}", path.display()))
    }

    pub(crate) fn list_roles(&self) -> Result<Vec<ItemSummary>> {
        let mut roles = Vec::new();
        for path in read_yamlish_dir(&self.roles_dir(), "md")? {
            let Some(name) = file_stem(&path) else {
                continue;
            };
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("failed to read role {}", path.display()))?;
            let role = parse_role(&name, path, &raw)?;
            roles.push(ItemSummary {
                name,
                desc: role.desc,
            });
        }
        roles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(roles)
    }

    pub(crate) fn list_macros(&self) -> Result<Vec<ItemSummary>> {
        let mut macros = Vec::new();
        for path in read_yamlish_dir(&self.macros_dir(), "yaml")? {
            let Some(name) = file_stem(&path) else {
                continue;
            };
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("failed to read macro {}", path.display()))?;
            let macro_file: MacroFile = serde_yml::from_str(&raw)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            macros.push(ItemSummary {
                name,
                desc: macro_file.desc,
            });
        }
        macros.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(macros)
    }

    fn from_existing(dir: &Path, source: &str) -> Result<Self> {
        if !Self::looks_like_library(dir) {
            bail!(
                "{source} path is not a pq library: {} (expected roles/ or macros/)",
                dir.display()
            );
        }
        Ok(Self {
            root: dir.to_path_buf(),
        })
    }

    fn looks_like_library(dir: &Path) -> bool {
        dir.join("roles").is_dir() || dir.join("macros").is_dir()
    }

    fn roles_dir(&self) -> PathBuf {
        self.root.join("roles")
    }

    fn macros_dir(&self) -> PathBuf {
        self.root.join("macros")
    }
}

#[derive(Debug)]
pub(crate) struct ItemSummary {
    pub(crate) name: String,
    pub(crate) desc: Option<String>,
}

fn read_yamlish_dir(dir: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn file_stem(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToOwned::to_owned)
}
