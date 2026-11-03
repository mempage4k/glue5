//! Versioned optional game-content manifests. Packages are logical sets of
//! ordinary catalog files; they are not the transport packs in `pack.rs`.
//!
//! The manifest is a sidecar (`packages.json`) in the game root and is sent
//! as the optional `packages` blob. Old clients ignore that blob and keep
//! using the existing catalog/stream protocol.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::catalog::{Catalog, safe_path};
use crate::protocol::PROTOCOL;

pub const MANIFEST_FILE: &str = "packages.json";
pub const MANIFEST_BLOB: &str = "packages";
pub const SCHEMA_VERSION: u64 = 1;
pub const MAX_MANIFEST_BYTES: usize = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferPhase {
    Downloading,
    Paused,
    Verifying,
    Installed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferProgress {
    pub package_id: String,
    pub phase: TransferPhase,
    pub file_path: Option<String>,
    pub file_done: u64,
    pub file_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

/// Thread-safe pause/cancel handle for a blocking package transfer. The
/// owner can keep an `Arc` while the transfer runs on a worker thread.
#[derive(Default)]
pub struct TransferControl {
    paused: AtomicBool,
    cancelled: AtomicBool,
    gate: Mutex<()>,
    wake: Condvar,
}

impl TransferControl {
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
        self.wake.notify_all();
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.wake.notify_all();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    pub(crate) fn wait_until_resumed(&self) -> bool {
        let mut guard = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        while self.paused.load(Ordering::Acquire) && !self.is_cancelled() {
            let result = self.wake.wait_timeout(guard, Duration::from_millis(250));
            guard = result.unwrap_or_else(|e| e.into_inner()).0;
        }
        !self.is_cancelled()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub catalog_id: u32,
    pub dependencies: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GamePackage {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Extensible string, not an enum: new package kinds remain forward-compatible.
    pub kind: String,
    pub version: String,
    pub target_build: String,
    pub catalog_hash: String,
    pub protocol_min: u64,
    pub protocol_max: u64,
    pub dependencies: Vec<String>,
    pub files: Vec<PackageFile>,
    pub total_size: u64,
    pub optional: bool,
    pub available: bool,
    pub locale: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageManifest {
    pub schema_version: u64,
    pub server_id: String,
    pub build: String,
    pub catalog_hash: String,
    pub protocol_min: u64,
    pub protocol_max: u64,
    pub packages: Vec<GamePackage>,
}

impl PackageManifest {
    /// Parse and bind an untrusted manifest to the exact server/build/catalog
    /// that announced it. SHA-256 values are validated syntactically here;
    /// file bytes are verified by the installer before installation.
    pub fn parse(bytes: &[u8], catalog: &Catalog, expected_server: Option<&str>) -> Result<Self> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            bail!("package manifest exceeds {MAX_MANIFEST_BYTES} bytes");
        }
        let value: serde_json::Value = serde_json::from_slice(bytes).context("parsing package manifest")?;
        let schema_version = uint(&value, "schema_version")?;
        if schema_version != SCHEMA_VERSION {
            bail!("unsupported package manifest schema {schema_version}");
        }
        let manifest = Self {
            schema_version,
            server_id: text(&value, "server_id")?,
            build: text(&value, "build")?,
            catalog_hash: text(&value, "catalog_hash")?,
            protocol_min: uint(&value, "protocol_min")?,
            protocol_max: uint(&value, "protocol_max")?,
            packages: value["packages"]
                .as_array()
                .context("package manifest has no packages array")?
                .iter()
                .map(parse_package)
                .collect::<Result<_>>()?,
        };
        manifest.validate(catalog, expected_server)?;
        Ok(manifest)
    }

    /// Resolves the dependency closure in install order (dependencies first),
    /// deduplicating shared dependencies.
    pub fn install_order(&self, package_id: &str) -> Result<Vec<&GamePackage>> {
        let by_id: std::collections::HashMap<_, _> = self.packages.iter().map(|p| (p.id.as_str(), p)).collect();
        let target = *by_id.get(package_id).with_context(|| format!("package {package_id:?} is not in this manifest"))?;
        if !target.available {
            bail!("package {package_id} is not currently available");
        }
        fn visit<'a>(
            package: &'a GamePackage,
            by_id: &std::collections::HashMap<&str, &'a GamePackage>,
            seen: &mut std::collections::HashSet<&'a str>,
            order: &mut Vec<&'a GamePackage>,
        ) -> Result<()> {
            if !seen.insert(package.id.as_str()) {
                return Ok(());
            }
            for dep in &package.dependencies {
                let dependency = by_id.get(dep.as_str()).with_context(|| format!("missing dependency {dep}"))?;
                if !dependency.available {
                    bail!("dependency {dep} is not currently available");
                }
                visit(*dependency, by_id, seen, order)?;
            }
            order.push(package);
            Ok(())
        }
        let mut seen = std::collections::HashSet::new();
        let mut order = Vec::new();
        visit(target, &by_id, &mut seen, &mut order)?;
        Ok(order)
    }

    pub fn validate(&self, catalog: &Catalog, expected_server: Option<&str>) -> Result<()> {
        if self.server_id.trim().is_empty() || self.server_id.len() > 128 {
            bail!("invalid package manifest server_id");
        }
        if expected_server.is_some_and(|id| id != self.server_id) {
            bail!("package manifest server identity does not match HELLO");
        }
        if self.build != catalog.build {
            bail!("package manifest is for build {}, not {}", self.build, catalog.build);
        }
        if self.catalog_hash != catalog.hash {
            bail!("package manifest catalog hash does not match the server catalog");
        }
        if self.protocol_min > self.protocol_max || !(self.protocol_min..=self.protocol_max).contains(&PROTOCOL) {
            bail!("package manifest does not support protocol {PROTOCOL}");
        }
        let mut by_id = std::collections::HashMap::new();
        let mut files_by_path: std::collections::HashMap<String, (u64, String, u32)> = std::collections::HashMap::new();
        for (i, package) in self.packages.iter().enumerate() {
            if !valid_id(&package.id) || by_id.insert(package.id.as_str(), i).is_some() {
                bail!("invalid or duplicate package id {:?}", package.id);
            }
            if package.name.trim().is_empty() || package.kind.trim().is_empty() || package.version.trim().is_empty() {
                bail!("package {} is missing name, kind or version", package.id);
            }
            if package.target_build != self.build || package.catalog_hash != self.catalog_hash {
                bail!("package {} targets a different build or catalog", package.id);
            }
            if package.protocol_min > package.protocol_max
                || package.protocol_min < self.protocol_min
                || package.protocol_max > self.protocol_max
                || !(package.protocol_min..=package.protocol_max).contains(&PROTOCOL)
            {
                bail!("package {} has incompatible protocol requirements", package.id);
            }
            unique_ids(&package.dependencies, &format!("package {} dependencies", package.id))?;
            let mut total = 0u64;
            let mut paths = std::collections::HashSet::new();
            for file in &package.files {
                if !safe_path(&file.path) || !file.path.starts_with("data/") {
                    bail!("package {} has an unsafe or non-game path {:?}", package.id, file.path);
                }
                if !paths.insert(file.path.to_ascii_lowercase()) {
                    bail!("package {} lists {} more than once", package.id, file.path);
                }
                if file.size == 0 {
                    bail!("package {} file {} is empty", package.id, file.path);
                }
                if file.sha256.len() != 64 || !file.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                    bail!("package {} has an invalid SHA-256 for {}", package.id, file.path);
                }
                let id = catalog.find(&file.path).with_context(|| format!("package {} references file absent from catalog: {}", package.id, file.path))?;
                let entry = &catalog.entries[id as usize];
                if file.catalog_id != id || entry.path != file.path || entry.size != file.size {
                    bail!("package {} file {} does not match its catalog entry", package.id, file.path);
                }
                if let Some((size, sha, catalog_id)) = files_by_path.get(&file.path) {
                    if *size != file.size || sha != &file.sha256 || *catalog_id != file.catalog_id {
                        bail!("packages disagree about the content of {}", file.path);
                    }
                } else {
                    files_by_path.insert(file.path.clone(), (file.size, file.sha256.clone(), file.catalog_id));
                }
                total = total.checked_add(file.size).context("package size overflow")?;
                unique_paths(&file.dependencies, &format!("file {} dependencies", file.path))?;
                for dependency in &file.dependencies {
                    if !dependency.starts_with("data/") || catalog.find(dependency).is_none() {
                        bail!("package {} file {} depends on a path absent from the game catalog: {dependency}", package.id, file.path);
                    }
                }
            }
            if total != package.total_size {
                bail!("package {} total_size is {}, expected {total}", package.id, package.total_size);
            }
            if package.files.is_empty() {
                bail!("package {} contains no files", package.id);
            }
        }
        for package in &self.packages {
            for dep in &package.dependencies {
                if !by_id.contains_key(dep.as_str()) || dep == &package.id {
                    bail!("package {} has unknown or self dependency {dep}", package.id);
                }
            }
        }
        check_dependency_cycles(&self.packages, &by_id)?;
        Ok(())
    }
}

fn parse_package(v: &serde_json::Value) -> Result<GamePackage> {
    let files = v["files"].as_array().context("package has no files array")?.iter().map(|f| {
        Ok(PackageFile {
            path: text(f, "path")?,
            size: uint(f, "size")?,
            sha256: text(f, "sha256")?.to_ascii_lowercase(),
            catalog_id: uint(f, "catalog_id")?.try_into().context("catalog_id exceeds u32")?,
            dependencies: strings(f, "dependencies")?,
        })
    }).collect::<Result<Vec<_>>>()?;
    Ok(GamePackage {
        id: text(v, "id")?,
        name: text(v, "name")?,
        description: v["description"].as_str().unwrap_or_default().to_owned(),
        kind: text(v, "kind")?,
        version: text(v, "version")?,
        target_build: text(v, "target_build")?,
        catalog_hash: text(v, "catalog_hash")?,
        protocol_min: uint(v, "protocol_min")?,
        protocol_max: uint(v, "protocol_max")?,
        dependencies: strings(v, "dependencies")?,
        files,
        total_size: uint(v, "total_size")?,
        optional: v["optional"].as_bool().context("package has no optional flag")?,
        available: v["available"].as_bool().context("package has no available flag")?,
        locale: v["locale"].as_str().map(str::to_owned),
        created_at: v["created_at"].as_str().map(str::to_owned),
    })
}

fn text(v: &serde_json::Value, key: &str) -> Result<String> {
    v[key].as_str().map(str::to_owned).with_context(|| format!("missing or invalid {key}"))
}

fn uint(v: &serde_json::Value, key: &str) -> Result<u64> {
    v[key].as_u64().with_context(|| format!("missing or invalid {key}"))
}

fn strings(v: &serde_json::Value, key: &str) -> Result<Vec<String>> {
    let values = v[key].as_array().with_context(|| format!("missing or invalid {key}"))?;
    values.iter().map(|item| item.as_str().map(str::to_owned).context("expected string array")).collect()
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

fn unique_ids(values: &[String], context: &str) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    if values.iter().any(|v| !valid_id(v) || !seen.insert(v)) {
        bail!("{context} contains an invalid or duplicate id");
    }
    Ok(())
}

fn unique_paths(values: &[String], context: &str) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    if values.iter().any(|v| !safe_path(v) || !seen.insert(v)) {
        bail!("{context} contains an invalid or duplicate path");
    }
    Ok(())
}

fn check_dependency_cycles(packages: &[GamePackage], ids: &std::collections::HashMap<&str, usize>) -> Result<()> {
    fn visit(i: usize, packages: &[GamePackage], ids: &std::collections::HashMap<&str, usize>, state: &mut [u8]) -> Result<()> {
        match state[i] {
            1 => bail!("package dependency cycle includes {}", packages[i].id),
            2 => return Ok(()),
            _ => {}
        }
        state[i] = 1;
        for dep in &packages[i].dependencies {
            visit(ids[dep.as_str()], packages, ids, state)?;
        }
        state[i] = 2;
        Ok(())
    }
    let mut state = vec![0; packages.len()];
    for i in 0..packages.len() {
        visit(i, packages, ids, &mut state)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (Catalog, Vec<u8>) {
        let catalog = Catalog::new("build-a".into(), vec![
            crate::catalog::Entry { path: "data/audio/a.rpf".into(), size: 7, mtime: 5 },
        ], 1);
        let json = serde_json::json!({
            "schema_version": 1,
            "server_id": "mirror-home",
            "build": "build-a",
            "catalog_hash": catalog.hash,
            "protocol_min": 1,
            "protocol_max": 1,
            "packages": [{
                "id": "dialogue-en", "name": "Dialogue", "description": "", "kind": "dialogue",
                "version": "1", "target_build": "build-a", "catalog_hash": catalog.hash,
                "protocol_min": 1, "protocol_max": 1, "dependencies": [],
                "files": [{"path":"data/audio/a.rpf", "size":7, "sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "catalog_id":0, "dependencies":[]}],
                "total_size":7,"optional":true,"available":true,"locale":"en","created_at":null
            }]
        });
        (catalog, serde_json::to_vec(&json).unwrap())
    }

    #[test]
    fn transfer_control_pauses_resumes_and_cancels_waiters() {
        let control = std::sync::Arc::new(TransferControl::default());
        control.pause();
        let worker = control.clone();
        let waiting = std::thread::spawn(move || worker.wait_until_resumed());
        std::thread::sleep(Duration::from_millis(20));
        assert!(!waiting.is_finished());
        control.resume();
        assert!(waiting.join().unwrap());

        control.pause();
        let worker = control.clone();
        let waiting = std::thread::spawn(move || worker.wait_until_resumed());
        control.cancel();
        assert!(!waiting.join().unwrap());
        assert!(control.is_cancelled());
    }

    #[test]
    fn validates_manifest_and_catalog_ids_without_reordering() {
        let (catalog, bytes) = sample();
        let manifest = PackageManifest::parse(&bytes, &catalog, Some("mirror-home")).unwrap();
        assert_eq!(manifest.packages[0].files[0].catalog_id, 0);
        assert_eq!(manifest.packages[0].kind, "dialogue");
    }

    #[test]
    fn rejects_catalog_build_identity_path_hash_size_and_dependency_mismatches() {
        let (catalog, bytes) = sample();
        assert!(PackageManifest::parse(&bytes, &catalog, Some("other-server")).is_err());
        let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["packages"][0]["files"][0]["path"] = "../../outside".into();
        assert!(PackageManifest::parse(&serde_json::to_vec(&v).unwrap(), &catalog, None).is_err());
        let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["packages"][0]["files"][0]["size"] = 8.into();
        assert!(PackageManifest::parse(&serde_json::to_vec(&v).unwrap(), &catalog, None).is_err());
        let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["packages"][0]["dependencies"] = serde_json::json!(["missing"]);
        assert!(PackageManifest::parse(&serde_json::to_vec(&v).unwrap(), &catalog, None).is_err());
    }

    #[test]
    fn shared_files_and_dependencies_are_supported() {
        let (catalog, bytes) = sample();
        let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let mut second = v["packages"][0].clone();
        second["id"] = "shared-dependency".into();
        second["dependencies"] = serde_json::json!([]);
        let mut third = v["packages"][0].clone();
        third["id"] = "target-package".into();
        third["dependencies"] = serde_json::json!(["dialogue-en", "shared-dependency"]);
        v["packages"].as_array_mut().unwrap().extend([second, third]);
        let manifest = PackageManifest::parse(&serde_json::to_vec(&v).unwrap(), &catalog, None).unwrap();
        assert_eq!(manifest.packages.len(), 3);
        assert_eq!(manifest.packages[0].files[0].path, manifest.packages[1].files[0].path);
        let order = manifest.install_order("target-package").unwrap();
        assert_eq!(order.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["dialogue-en", "shared-dependency", "target-package"]);
        assert!(manifest.install_order("missing").is_err());
    }

    #[test]
    fn rejects_duplicate_or_uncatalogued_files_and_bad_hashes() {
        let (catalog, bytes) = sample();
        let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let file = v["packages"][0]["files"][0].clone();
        v["packages"][0]["files"].as_array_mut().unwrap().push(file.clone());
        v["packages"][0]["total_size"] = 14.into();
        assert!(PackageManifest::parse(&serde_json::to_vec(&v).unwrap(), &catalog, None).is_err());

        let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["packages"][0]["files"][0]["path"] = "data/audio/not-in-catalog.rpf".into();
        assert!(PackageManifest::parse(&serde_json::to_vec(&v).unwrap(), &catalog, None).is_err());

        let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["packages"][0]["files"][0]["sha256"] = "not-a-sha256".into();
        assert!(PackageManifest::parse(&serde_json::to_vec(&v).unwrap(), &catalog, None).is_err());
    }

    #[test]
    fn refuses_cycles_and_unsupported_schema_or_protocol() {
        let (catalog, bytes) = sample();
        let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["build"] = "other-build".into();
        assert!(PackageManifest::parse(&serde_json::to_vec(&v).unwrap(), &catalog, None).is_err());
        let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["catalog_hash"] = "stale-catalog".into();
        assert!(PackageManifest::parse(&serde_json::to_vec(&v).unwrap(), &catalog, None).is_err());
        let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["schema_version"] = 2.into();
        assert!(PackageManifest::parse(&serde_json::to_vec(&v).unwrap(), &catalog, None).is_err());
        let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["protocol_min"] = 9.into();
        assert!(PackageManifest::parse(&serde_json::to_vec(&v).unwrap(), &catalog, None).is_err());
        let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["packages"][0]["dependencies"] = serde_json::json!(["dialogue-en"]);
        assert!(PackageManifest::parse(&serde_json::to_vec(&v).unwrap(), &catalog, None).is_err());
    }
}
