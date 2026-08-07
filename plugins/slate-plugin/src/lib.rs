//! `slate-plugin` — Slate's plugin system.
//!
//! A plugin is a directory with a `plugin.toml` manifest and an executable:
//!
//! ```toml
//! name = "hello"
//! version = "0.1.0"
//! description = "Greets the world"
//! author = "You"
//! exec = "hello.sh"          # relative to the plugin directory
//! provides = ["hello"]       # command names it registers (informational)
//! ```
//!
//! Execution is *sandboxed*: the plugin runs as a subprocess with a scrubbed
//! environment (see [`SANDBOX_ENV`]), its working directory set to the plugin
//! dir, stdin closed, output captured. WASM plugins are on the roadmap (v0.2+
//! ) and will implement the same manifest spec.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use slate_config::{Document, Value};
use slate_core::error::{Error, Result};

/// Environment variables passed to plugins (everything else is scrubbed).
pub const SANDBOX_ENV: &[&str] = &["PATH", "LANG", "LC_ALL", "HOME", "USER", "TERM"];

/// Plugin API version reported via `SLATE_API_VERSION`.
pub const API_VERSION: &str = "1";

/// The parsed `plugin.toml`.
#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    /// Executable path, relative to the plugin directory.
    pub exec: String,
    /// Commands the plugin contributes (documentation/search metadata).
    pub provides: Vec<String>,
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Manifest> {
        let src = fs::read_to_string(path)?;
        Self::parse(&src).map_err(|e| Error::parse("plugin.toml", format!("{}: {e}", path.display())))
    }

    pub fn parse(src: &str) -> Result<Manifest> {
        let doc = Document::parse(src)?;
        let get = |key: &str, required: bool| -> Result<String> {
            match doc_get(&doc, key) {
                Some(v) => Ok(v),
                None if required => Err(Error::parse("plugin.toml", format!("missing '{key}'"))),
                None => Ok(String::new()),
            }
        };
        let name = get("name", true)?;
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(Error::parse(
                "plugin.toml",
                "name may only contain letters, digits, '-' and '_'",
            ));
        }
        let exec = get("exec", true)?;
        if exec.starts_with('/') || exec.contains("..") {
            return Err(Error::parse(
                "plugin.toml",
                "exec must be a relative path inside the plugin directory",
            ));
        }
        let provides = match slate_config::Document::get(&doc, "provides") {
            Some(Value::Array(items)) => {
                items.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
            }
            _ => Vec::new(),
        };
        Ok(Manifest {
            name,
            version: get("version", false).map(|v| if v.is_empty() { "0.0.0".into() } else { v })?,
            description: get("description", false)?,
            author: get("author", false)?,
            exec,
            provides,
        })
    }
}

fn doc_get(doc: &slate_config::Document, key: &str) -> Option<String> {
    doc.get(key).and_then(Value::as_str).map(str::to_string)
}

/// An installed plugin.
#[derive(Clone, Debug)]
pub struct Plugin {
    pub manifest: Manifest,
    pub dir: PathBuf,
    pub enabled: bool,
}

impl Plugin {
    pub fn exec_path(&self) -> PathBuf {
        self.dir.join(&self.manifest.exec)
    }
}

/// The scanned plugin directory.
pub struct PluginRegistry {
    dir: PathBuf,
    plugins: Vec<Plugin>,
}

impl PluginRegistry {
    /// Scan `dir` for child directories with a `plugin.toml`.
    pub fn load(dir: PathBuf) -> Self {
        let mut plugins = Vec::new();
        if let Ok(rd) = fs::read_dir(&dir) {
            for entry in rd.flatten() {
                let pdir = entry.path();
                if !pdir.is_dir() {
                    continue;
                }
                let manifest_path = pdir.join("plugin.toml");
                if !manifest_path.exists() {
                    continue;
                }
                if let Ok(manifest) = Manifest::load(&manifest_path) {
                    let enabled = !pdir.join(".disabled").exists();
                    plugins.push(Plugin { manifest, dir: pdir, enabled });
                }
            }
        }
        plugins.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
        PluginRegistry { dir, plugins }
    }

    pub fn empty(dir: PathBuf) -> Self {
        PluginRegistry { dir, plugins: Vec::new() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn list(&self) -> &[Plugin] {
        &self.plugins
    }

    pub fn reload(&mut self) {
        *self = PluginRegistry::load(self.dir.clone());
    }

    pub fn find(&self, name: &str) -> Option<&Plugin> {
        self.plugins.iter().find(|p| p.manifest.name == name)
    }

    /// Enable/disable by touching the `.disabled` marker.
    pub fn set_enabled(&mut self, name: &str, enabled: bool) -> Result<()> {
        let Some(plugin) = self.plugins.iter().find(|p| p.manifest.name == name) else {
            return Err(Error::not_found(format!("plugin '{name}'")));
        };
        let marker = plugin.dir.join(".disabled");
        if enabled {
            if marker.exists() {
                fs::remove_file(marker)?;
            }
        } else {
            fs::create_dir_all(&self.dir)?;
            fs::write(marker, b"disabled\n")?;
        }
        self.reload();
        Ok(())
    }

    /// Install from a source directory containing `plugin.toml`. Returns the
    /// installed plugin name. Also accepts a direct path to a `plugin.toml`.
    pub fn install(&mut self, src: &Path) -> Result<String> {
        let src_dir = if src.is_file() { src.parent().unwrap_or(src) } else { src };
        let manifest = Manifest::load(&src_dir.join("plugin.toml"))?;
        let dest = self.dir.join(&manifest.name);
        if dest.exists() {
            return Err(Error::invalid(format!(
                "plugin '{}' is already installed (remove it first)",
                manifest.name
            )));
        }
        fs::create_dir_all(&self.dir)?;
        copy_dir(src_dir, &dest)?;
        self.reload();
        Ok(manifest.name)
    }

    pub fn remove(&mut self, name: &str) -> Result<()> {
        let Some(plugin) = self.plugins.iter().find(|p| p.manifest.name == name) else {
            return Err(Error::not_found(format!("plugin '{name}'")));
        };
        fs::remove_dir_all(&plugin.dir)?;
        self.reload();
        Ok(())
    }

    /// Run an enabled plugin with arguments, capturing output.
    pub fn run(&self, name: &str, args: &[String]) -> Result<String> {
        let Some(plugin) = self.plugins.iter().find(|p| p.manifest.name == name) else {
            return Err(Error::not_found(format!("plugin '{name}'")));
        };
        if !plugin.enabled {
            return Err(Error::invalid(format!("plugin '{name}' is disabled")));
        }
        let exec = plugin.exec_path();
        if !exec.exists() {
            return Err(Error::not_found(format!(
                "{} (declared in plugin.toml as exec)",
                exec.display()
            )));
        }
        let mut cmd = Command::new(&exec);
        cmd.args(args)
            .current_dir(&plugin.dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Sandboxed environment: scrub everything, then allow-list.
        cmd.env_clear();
        for &key in SANDBOX_ENV {
            if let Some(v) = std::env::var_os(key) {
                cmd.env(key, v);
            }
        }
        cmd.env("SLATE_API_VERSION", API_VERSION);
        cmd.env("SLATE_PLUGIN_DIR", &plugin.dir);
        cmd.env("SLATE_PLUGIN_NAME", &plugin.manifest.name);
        let out = cmd.output()?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        if out.status.success() {
            Ok(stdout)
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(Error::other(format!(
                "plugin '{name}' failed ({}): {}",
                out.status,
                if stderr.is_empty() { "no error output" } else { &stderr }
            )))
        }
    }

    /// Substring search over installed plugin metadata (`plugin search`).
    pub fn search(&self, query: &str) -> Vec<&Plugin> {
        let q = query.to_lowercase();
        self.plugins
            .iter()
            .filter(|p| {
                p.manifest.name.to_lowercase().contains(&q)
                    || p.manifest.description.to_lowercase().contains(&q)
                    || p.manifest.provides.iter().any(|c| c.to_lowercase().contains(&q))
            })
            .collect()
    }
}

fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&path) {
                fs::set_permissions(&target, fs::Permissions::from_mode(meta.permissions().mode()))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> PathBuf {
        std::env::temp_dir().join(format!("slate-plugin-test-{}", std::process::id()))
    }

    fn make_source(root: &Path, name: &str) -> PathBuf {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.toml"),
            format!(
                "name = \"{name}\"\nversion = \"1.0.0\"\ndescription = \"Says hi\"\nauthor = \"test\"\nexec = \"run.sh\"\nprovides = [\"{name}\"]\n"
            ),
        )
        .unwrap();
        let script = dir.join("run.sh");
        fs::write(&script, "#!/bin/sh\necho \"hello from $SLATE_PLUGIN_NAME: $1\"\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        }
        dir
    }

    #[test]
    fn manifest_validation() {
        assert!(Manifest::parse("name = \"x\"\nexec = \"a.sh\"").is_ok());
        assert!(Manifest::parse("exec = \"a.sh\"").is_err()); // missing name
        assert!(Manifest::parse("name = \"x\"\nexec = \"/abs\"").is_err()); // absolute exec
        assert!(Manifest::parse("name = \"x\"\nexec = \"../up\"").is_err()); // traversal
        assert!(Manifest::parse("name = \"bad name!\"\nexec = \"a\"").is_err()); // bad chars
        let m = Manifest::parse("name = \"x\"\nexec = \"a\"").unwrap();
        assert_eq!(m.version, "0.0.0");
    }

    #[test]
    fn install_run_enable_disable_remove() {
        let root = base();
        let _ = fs::remove_dir_all(&root);
        let src = make_source(&root, "hello");
        let plugins_dir = root.join("installed");
        let mut reg = PluginRegistry::load(plugins_dir.clone());
        assert!(reg.list().is_empty());

        let name = reg.install(&src).unwrap();
        assert_eq!(name, "hello");
        assert_eq!(reg.list().len(), 1);
        assert!(reg.install(&src).is_err()); // duplicate refused

        let out = reg.run("hello", &["world".to_string()]).unwrap();
        assert_eq!(out.trim(), "hello from hello: world");

        reg.set_enabled("hello", false).unwrap();
        assert!(!reg.find("hello").unwrap().enabled);
        assert!(reg.run("hello", &[]).is_err()); // disabled refuses to run
        reg.set_enabled("hello", true).unwrap();
        assert!(reg.find("hello").unwrap().enabled);

        assert_eq!(reg.search("says hi").len(), 1);
        assert!(reg.search("zzz").is_empty());

        reg.remove("hello").unwrap();
        assert!(reg.list().is_empty());
        assert!(reg.remove("hello").is_err());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn install_accepts_manifest_path_directly() {
        let root = std::env::temp_dir().join(format!("slate-plugin-path-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let src = make_source(&root, "byman");
        let mut reg = PluginRegistry::load(root.join("installed-path"));
        let name = reg.install(&src.join("plugin.toml")).unwrap();
        assert_eq!(name, "byman");
        let _ = fs::remove_dir_all(&root);
    }
}
