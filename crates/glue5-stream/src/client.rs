//! The client: the game's files from a server, through the local [`Cache`].
//!
//! [`Client::start`] connects in the background and, before the game can
//! start, takes the catalog and the packs (every small file and the
//! shaders, ~10 MB in all); the boot set (the ranges every start reads,
//! ~550 MB) then comes at a low priority while the player is still on the
//! start page or the shaders compile (once the game runs, what follows the
//! parts it waits for comes first). A read the cache cannot answer asks
//! the server at the highest priority, or raises a request already on its
//! way, and waits for just its blocks. The engine's own announcements of
//! the reads it will make ([`Client::hint`]) and read-ahead of files played
//! over time (music, radio) are fetched in between.
//!
//! A lost connection is retried for ever; requests in flight are asked
//! again on the new one. Without a server, a copy that was complete enough
//! still plays (whatever is missing waits for the server).
//!
//! [`Client::stop`] returns at once; the client finishes in the background
//! (its threads end, its index is saved last). Until it has, nothing else
//! touches its folder: a client started after it waits, and so does work
//! on copies' folders ([`after_stopped`]).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{self, BufReader, Read};
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, PoisonError};
use std::thread::{self, JoinHandle, Thread};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use glue5_qos::{self as qos, Qos};
use sha2::{Digest, Sha256};

use crate::cache::{BLOCK, Cache};
use crate::catalog::Catalog;
use crate::pack::{self, Kind};
use crate::packages::{self, PackageManifest, TransferControl, TransferPhase, TransferProgress};
use crate::protocol::*;

pub struct Options {
    /// `host` or `host:port`.
    pub server: String,
    /// Where the copy is kept.
    pub root: PathBuf,
    /// The game build this app runs; the server must have the same.
    pub build: String,
    /// Which boot set to prefetch.
    pub low_memory: bool,
}

/// Free space the copy leaves on its volume: below it, files read longest
/// ago are emptied (or, downloading the whole game, the download waits).
const FLOOR: u64 = 1 << 30;
/// What emptying frees above [`FLOOR`]: enough not to come back at once,
/// little enough that a full phone keeps most of its copy (it gave away a
/// gigabyte at a time, much of it the start's own files, fetched again).
const ROOM: u64 = 256 << 20;
/// Blocks per request: one chunk (64 KB). When the engine comes to wait for
/// a prefetched block, the request holding it moves ahead whole: small
/// requests keep that to one chunk (with 1 MB requests, a read waited 134 ms
/// behind a prefetch at 8 MB/s; with these, 10-14 ms).
const MAX_RUN: u64 = CHUNK as u64 / BLOCK;
/// Read-ahead of a file played over time (music, a radio station): four
/// times the read at first (64 KB at least), doubling up to 1 MB; of one
/// read in pieces of 64 KB or more one after the other: from 64 KB,
/// doubling up to 512 KB.
const AHEAD_FIRST: u64 = 64 << 10;
const AHEAD_MAX: u64 = 1 << 20;
const PIECES_MAX: u64 = 512 << 10;
/// A file read again this long at least after the engine had its previous
/// read is being played over time, not read in pieces.
const STREAM_GAP: Duration = Duration::from_millis(20);
/// Two boot ranges closer than this are fetched as one.
const BOOT_GAP: u64 = 64;
/// When the engine waits for a part of the boot set not here yet, this much
/// of the boot set after it comes next (it reads the boot set in an order of
/// its own, the stream brings it file after file).
const FOLLOW: u64 = 8 << 20;
/// Requests of the full download on their way at most (32 MB): spare
/// bandwidth is used, the queue of the server stays short.
const FILL_REQUESTS: usize = 512;
/// In the copy: the boot set it was given (for a start without the server).
const BOOT_SET: &str = "bootset.json";
/// While data keeps coming, the index is saved this often, or once this
/// much came since the last save (a save syncs every file written to, and
/// the game's frames come a little slower meanwhile; a crash loses at most
/// that much of the download); as soon as it stops, at once.
const SAVE_BUSY: Duration = Duration::from_secs(10);
const SAVE_BYTES: u64 = 256 << 20;
/// Requests in flight and no data for this long, counted from the last data
/// or from when they were asked after none was in flight (the connection
/// may have been quiet for long before): the connection is dead. So is one
/// that takes no request for this long.
#[cfg(not(test))]
pub(crate) const SILENCE: Duration = Duration::from_secs(20);
/// Tests wait for it.
#[cfg(test)]
pub(crate) const SILENCE: Duration = Duration::from_secs(4);
/// A read whose blocks were asked for less than this before the engine came
/// to wait for them was not announced in time (the engine often announces a
/// read as it makes it): it costs a round trip as if not announced at all.
const IN_TIME: Duration = Duration::from_millis(10);
/// What a server may send before the game: blobs (the packs are ~10 MB)
/// and the JSON in them, unpacked (the catalog is ~2 MB). A broken server
/// claiming more is refused, not obeyed.
const MAX_BLOB: u64 = 256 << 20;
const MAX_JSON: u64 = 64 << 20;

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Connecting,
    /// Taking what every start needs: `what`, `done` of `total` bytes.
    Syncing { what: &'static str, done: u64, total: u64 },
    Ready,
    /// The game cannot come from this server.
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct Status {
    pub phase: Phase,
    pub connected: bool,
    /// Why the last connection attempt failed or ended.
    pub error: Option<String>,
    pub cache_bytes: u64,
    /// Of the boot set: bytes here, of its total.
    pub boot: (u64, u64),
    /// Engine reads waiting for the server now.
    pub waiting: u32,
    /// The game's size: what a complete copy holds.
    pub total_bytes: u64,
    /// Downloading the whole game, and whether it is all here (the copy is
    /// then a game folder, which plays without the server).
    pub filling: bool,
    pub complete: bool,
    /// The whole game does not fit: the download waits for space.
    pub no_room: bool,
}

pub struct Client {
    opts: Options,
    phase: Mutex<Phase>,
    error: Mutex<Option<String>>,
    ready: OnceLock<Ready>,
    /// The connection, from when it is made (for `stop` to shut it).
    socket: Mutex<Option<TcpStream>>,
    /// The connection requests go on (locked briefly: writes hold `sending`).
    conn: Mutex<Option<Arc<TcpStream>>>,
    /// Whether there is one (the app asks every frame).
    connected: AtomicBool,
    /// Held while requests are written, one writer at a time; the socket's
    /// write timeout bounds it.
    sending: Mutex<()>,
    inflight: Mutex<Inflight>,
    waiters: Mutex<Vec<Waiter>>,
    stats: Stats,
    stop: AtomicBool,
    /// Wakes the client's threads from their pauses when it stops.
    woken: Mutex<()>,
    wake: Condvar,
    /// Set once a stopped client has finished.
    finished: Arc<Done>,
    /// The whole game is wanted on this device ([`Client::set_fill`]), and
    /// it is all here.
    fill: AtomicBool,
    complete: AtomicBool,
    no_room: AtomicBool,
    /// Emptying files made no room the last time: none is emptied until the
    /// volume has room again.
    no_gain: AtomicBool,
    /// A copy from an earlier run was tried without the server.
    tried_offline: AtomicBool,
    /// The boot set's progress, counted (a word of the bitmap at a time) at
    /// most twice a second: the UI asks every frame.
    boot: Mutex<Option<(Instant, (u64, u64))>>,
}

struct Ready {
    catalog: Catalog,
    cache: Cache,
    /// Optional logical packages; absent/invalid metadata never disables base streaming.
    packages: Option<PackageManifest>,
    package_server_id: Option<String>,
    package_hash: Option<String>,
    manifest: Vec<u8>,
    boot: Vec<(u32, u64, u64)>,
    /// Per file: its runs in `boot`.
    boot_runs: HashMap<u32, Vec<usize>>,
    /// Per file: the end of its last read, when the engine had it (µs after
    /// `epoch`), and the current read-ahead.
    seq_end: Vec<AtomicU64>,
    seq_done: Vec<AtomicU64>,
    ahead: Vec<AtomicU64>,
    epoch: Instant,
    radio: Vec<bool>,
}

impl Ready {
    /// The engine has its read of file `id`.
    fn read_done(&self, id: u32) {
        self.seq_done[id as usize].store(self.epoch.elapsed().as_micros() as u64, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct Inflight {
    next: u32,
    reqs: HashMap<u32, Req>,
    /// Per file: first block -> request (requests of a file never overlap).
    by_file: HashMap<u32, BTreeMap<u64, u32>>,
    /// Since when requests are in flight (none: none is).
    since: Option<Instant>,
}

#[derive(Clone, Copy)]
struct Req {
    file: u32,
    first: u64,
    last: u64,
    prio: u8,
    asked: Instant,
}

struct Waiter {
    file: u32,
    first: u64,
    last: u64,
    thread: Thread,
}

#[derive(Default)]
struct Stats {
    received: AtomicU64,
    waits: AtomicU64,
    wait_us: AtomicU64,
    cold: AtomicU64,
    hints: AtomicU64,
    waiting: AtomicU32,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").field("server", &self.opts.server).field("root", &self.opts.root).finish_non_exhaustive()
    }
}

impl Inflight {
    fn covering(&self, file: u32, block: u64) -> Option<u32> {
        let (_, &id) = self.by_file.get(&file)?.range(..=block).next_back()?;
        (self.reqs[&id].last >= block).then_some(id)
    }

    fn add(&mut self, req: Req) -> u32 {
        self.next = self.next.wrapping_add(1).max(1);
        let id = self.next;
        if self.reqs.is_empty() {
            self.since = Some(Instant::now());
        }
        self.reqs.insert(id, req);
        self.by_file.entry(req.file).or_default().insert(req.first, id);
        id
    }

    fn remove(&mut self, id: u32) -> Option<Req> {
        let req = self.reqs.remove(&id)?;
        if self.reqs.is_empty() {
            self.since = None;
        }
        if let Some(m) = self.by_file.get_mut(&req.file) {
            m.remove(&req.first);
            if m.is_empty() {
                self.by_file.remove(&req.file);
            }
        }
        Some(req)
    }
}

impl Client {
    pub fn start(opts: Options) -> Arc<Client> {
        let client = Arc::new(Client {
            opts,
            phase: Mutex::new(Phase::Connecting),
            error: Mutex::new(None),
            ready: OnceLock::new(),
            socket: Mutex::new(None),
            conn: Mutex::new(None),
            connected: AtomicBool::new(false),
            sending: Mutex::new(()),
            inflight: Mutex::new(Inflight::default()),
            waiters: Mutex::new(Vec::new()),
            stats: Stats::default(),
            stop: AtomicBool::new(false),
            woken: Mutex::new(()),
            wake: Condvar::new(),
            finished: Arc::new(Done::default()),
            fill: AtomicBool::new(false),
            complete: AtomicBool::new(false),
            no_room: AtomicBool::new(false),
            no_gain: AtomicBool::new(false),
            tried_offline: AtomicBool::new(false),
            boot: Mutex::new(None),
        });
        let before = pending();
        // In classes of their own, not their creator's (the app's main
        // thread): the engine's reads wait on the connection's, the saves
        // and the full download are background work.
        let c = client.clone();
        let keep = qos::spawn("stream sync", Qos::Utility, move || c.keep()).expect("stream sync thread");
        let c = client.clone();
        let fill = qos::spawn("stream fill", Qos::Utility, move || c.fill()).expect("stream fill thread");
        let c = client.clone();
        qos::spawn("stream", Qos::Initiated, move || c.run(before, [keep, fill])).expect("stream thread");
        client
    }

    pub fn server(&self) -> &str {
        &self.opts.server
    }

    /// The copy's folder: a game folder's layout (`data/`, `b/<build>/shaders/`).
    pub fn root(&self) -> &Path {
        &self.opts.root
    }

    /// Validated optional content announced by this server. `None` means the
    /// server is older, did not publish a manifest, or its manifest was invalid.
    pub fn packages(&self) -> Option<PackageManifest> {
        self.ready.get()?.packages.clone()
    }

    /// Whether a package has a matching durable receipt and all of its files
    /// are still complete in the cache. The receipt records the SHA-256 check
    /// performed at installation time.
    pub fn package_is_installed(&self, package_id: &str) -> bool {
        let Some(ready) = self.ready.get() else { return false };
        let Some(manifest) = ready.packages.as_ref() else { return false };
        let Some(package) = manifest.packages.iter().find(|p| p.id == package_id) else { return false };
        let path = self.opts.root.join("packages-state.json");
        let (state, identity) = load_package_state(&path, manifest);
        let fingerprint = package_fingerprint(package);
        identity
            && state["installed"][package_id]["fingerprint"].as_str() == Some(fingerprint.as_str())
            && package.files.iter().all(|file| ready.cache.complete(file.catalog_id) && ready.cache.size(file.catalog_id) == file.size)
    }

    /// Downloads one selected package and its dependencies using ordinary
    /// catalog range reads. Partial blocks remain in the cache for resumption;
    /// only verified package members are pinned against cache eviction.
    pub fn download_package(
        &self,
        package_id: &str,
        control: &TransferControl,
        mut progress: impl FnMut(TransferProgress),
    ) -> Result<()> {
        let ready = self.ready.get().context("stream is not ready")?;
        let manifest = ready.packages.as_ref().context("server has no valid optional package manifest")?;
        let order = manifest.install_order(package_id)?;
        let mut packages_to_install = Vec::with_capacity(order.len());
        let mut files = BTreeMap::new();
        for package in order {
            packages_to_install.push(package.clone());
            for file in &package.files {
                if let Some(old) = files.get(&file.path)
                    && (old.size != file.size || old.sha256 != file.sha256 || old.catalog_id != file.catalog_id)
                {
                    bail!("packages disagree about the content of {}", file.path);
                }
                files.insert(file.path.clone(), file.clone());
            }
        }
        let total: u64 = files.values().try_fold(0u64, |sum, file| sum.checked_add(file.size).context("package size overflow"))?;
        let mut verified = HashSet::new();
        let mut verified_bytes = 0u64;
        let mut missing = 0u64;
        for file in files.values() {
            if ready.cache.complete(file.catalog_id) {
                let matches = hash_file_matches(&ready.cache.path(file.catalog_id), file.size, &file.sha256, Some(control), |hashed| {
                    progress(TransferProgress {
                        package_id: package_id.to_owned(), phase: TransferPhase::Verifying,
                        file_path: Some(file.path.clone()), file_done: hashed, file_total: file.size,
                        bytes_done: verified_bytes.saturating_add(hashed), bytes_total: total,
                    });
                })?;
                let Some(matches) = matches else {
                    let _ = ready.cache.save();
                    progress(TransferProgress { package_id: package_id.to_owned(), phase: TransferPhase::Cancelled, file_path: Some(file.path.clone()), file_done: 0, file_total: file.size, bytes_done: verified_bytes, bytes_total: total });
                    bail!("package download cancelled");
                };
                if matches {
                    verified.insert(file.path.clone());
                    verified_bytes = verified_bytes.saturating_add(file.size);
                    continue;
                }
                ready.cache.invalidate(file.catalog_id)?;
            }
            let blocks = ready.cache.blocks(file.catalog_id);
            let have = if blocks == 0 { 0 } else { ready.cache.count(file.catalog_id, 0, blocks - 1) * BLOCK };
            missing = missing.saturating_add(file.size.saturating_sub(have.min(file.size)));
        }
        let required = if missing == 0 { 0 } else { missing.saturating_add(FLOOR).saturating_add(ROOM) };
        if required > 0 {
            let free = ready.cache.free_space().context("cannot determine free storage for package download")?;
            if free < required {
                bail!("not enough free storage: need about {required} bytes, have {free}");
            }
            if !self.connected.load(Ordering::Acquire) {
                bail!("server is offline; partial package download can resume when connected");
            }
        }
        let mut done = verified_bytes;
        let state_path = self.opts.root.join("packages-state.json");
        let (mut state, same_identity) = load_package_state(&state_path, manifest);
        if !same_identity {
            ready.cache.set_pinned(&[])?;
        }
        for package in packages_to_install {
            for file in &package.files {
                if verified.contains(&file.path) {
                    continue;
                }
                let id = file.catalog_id;
                let mut passed_hash = false;
                for attempt in 0..2 {
                    let mut offset = 0u64;
                    while offset < file.size {
                        if control.is_paused() {
                            progress(TransferProgress {
                                package_id: package.id.clone(), phase: TransferPhase::Paused,
                                file_path: Some(file.path.clone()), file_done: offset, file_total: file.size,
                                bytes_done: done + offset, bytes_total: total,
                            });
                        }
                        if !control.wait_until_resumed() {
                            let _ = ready.cache.save();
                            progress(TransferProgress { package_id: package.id.clone(), phase: TransferPhase::Cancelled, file_path: Some(file.path.clone()), file_done: offset, file_total: file.size, bytes_done: done + offset, bytes_total: total });
                            bail!("package download cancelled");
                        }
                        let amount = (file.size - offset).min(CHUNK as u64) as usize;
                        let mut buffer = vec![0; amount];
                        let Some(got) = self.read_inner(id, offset, &mut buffer, Some(control)) else {
                            let _ = ready.cache.save();
                            if control.is_cancelled() {
                                progress(TransferProgress { package_id: package.id.clone(), phase: TransferPhase::Cancelled, file_path: Some(file.path.clone()), file_done: offset, file_total: file.size, bytes_done: done + offset, bytes_total: total });
                                bail!("package download cancelled");
                            }
                            bail!("stream read failed for {}", file.path);
                        };
                        if got == 0 {
                            bail!("unexpected end of {}", file.path);
                        }
                        offset += got as u64;
                        progress(TransferProgress {
                            package_id: package.id.clone(), phase: TransferPhase::Downloading,
                            file_path: Some(file.path.clone()), file_done: offset, file_total: file.size,
                            bytes_done: done + offset, bytes_total: total,
                        });
                    }
                    if control.is_cancelled() {
                        let _ = ready.cache.save();
                        progress(TransferProgress { package_id: package.id.clone(), phase: TransferPhase::Cancelled, file_path: Some(file.path.clone()), file_done: offset, file_total: file.size, bytes_done: done + offset, bytes_total: total });
                        bail!("package download cancelled");
                    }
                    ready.cache.save()?;
                    progress(TransferProgress {
                        package_id: package.id.clone(), phase: TransferPhase::Verifying,
                        file_path: Some(file.path.clone()), file_done: 0, file_total: file.size,
                        bytes_done: done, bytes_total: total,
                    });
                    if ready.cache.complete(id) {
                        let matches = hash_file_matches(&ready.cache.path(id), file.size, &file.sha256, Some(control), |hashed| {
                            progress(TransferProgress {
                                package_id: package.id.clone(), phase: TransferPhase::Verifying,
                                file_path: Some(file.path.clone()), file_done: hashed, file_total: file.size,
                                bytes_done: done.saturating_add(hashed), bytes_total: total,
                            });
                        })?;
                        let Some(matches) = matches else {
                            let _ = ready.cache.save();
                            progress(TransferProgress { package_id: package.id.clone(), phase: TransferPhase::Cancelled, file_path: Some(file.path.clone()), file_done: 0, file_total: file.size, bytes_done: done, bytes_total: total });
                            bail!("package download cancelled");
                        };
                        if matches {
                            passed_hash = true;
                            break;
                        }
                    }
                    ready.cache.invalidate(id)?;
                    if attempt == 1 {
                        bail!("SHA-256 mismatch after retry for {}", file.path);
                    }
                }
                if !passed_hash {
                    bail!("could not verify {}", file.path);
                }
                verified.insert(file.path.clone());
                done = done.saturating_add(file.size);
            }
            state["installed"][package.id.as_str()] = serde_json::json!({
                "version": package.version,
                "fingerprint": package_fingerprint(&package),
                "target_build": package.target_build,
                "catalog_hash": package.catalog_hash,
                "files": package.files.iter().map(|f| serde_json::json!({
                    "path": f.path, "catalog_id": f.catalog_id, "size": f.size, "sha256": f.sha256,
                })).collect::<Vec<_>>(),
            });
            save_package_state(&state_path, &state)?;
            let pins = installed_pin_ids(manifest, &state);
            ready.cache.set_pinned(&pins)?;
            progress(TransferProgress {
                package_id: package.id, phase: TransferPhase::Installed, file_path: None,
                file_done: 0, file_total: 0, bytes_done: done, bytes_total: total,
            });
        }
        Ok(())
    }

    pub fn is_ready(&self) -> bool {
        self.ready.get().is_some()
    }

    /// Engine reads that waited for blocks not announced in time, so far.
    #[cfg(test)]
    pub(crate) fn not_in_time(&self) -> u64 {
        self.stats.cold.load(Ordering::Relaxed)
    }

    /// Whether the block holding `off` of file `id` is here.
    #[cfg(test)]
    pub(crate) fn has(&self, id: u32, off: u64) -> bool {
        self.ready.get().is_some_and(|r| r.cache.has_block(id, off / BLOCK))
    }

    pub fn status(&self) -> Status {
        let boot = {
            let mut cached = self.boot.lock().unwrap();
            match *cached {
                Some((at, boot)) if at.elapsed() < Duration::from_millis(500) => boot,
                _ => {
                    let boot = self.ready.get().map_or((0, 0), |r| {
                        let (mut have, mut total) = (0, 0);
                        for &(id, first, last) in &r.boot {
                            total += last - first + 1;
                            have += r.cache.count(id, first, last);
                        }
                        (have * BLOCK, total * BLOCK)
                    });
                    *cached = Some((Instant::now(), boot));
                    boot
                }
            }
        };
        Status {
            phase: self.phase.lock().unwrap().clone(),
            connected: self.connected.load(Ordering::Acquire),
            error: self.error.lock().unwrap().clone(),
            cache_bytes: self.ready.get().map_or(0, |r| r.cache.used()),
            boot,
            waiting: self.stats.waiting.load(Ordering::Relaxed),
            total_bytes: self.ready.get().map_or(0, |r| r.cache.total()),
            filling: self.fill.load(Ordering::Relaxed) && !self.complete.load(Ordering::Relaxed),
            complete: self.complete.load(Ordering::Relaxed),
            no_room: self.no_room.load(Ordering::Relaxed),
        }
    }

    /// The engine's manifest of its files (once ready).
    pub fn engine_manifest(&self) -> Option<&[u8]> {
        self.ready.get().map(|r| &r.manifest[..])
    }

    /// The id of the file at `path` (relative to the game folder).
    pub fn find(&self, path: &str) -> Option<u32> {
        self.ready.get()?.catalog.find(path)
    }

    /// Where the copy keeps file `id`.
    pub fn path(&self, id: u32) -> Option<PathBuf> {
        let r = self.ready.get()?;
        ((id as usize) < r.cache.len()).then(|| r.cache.path(id))
    }

    /// Downloads every file of the game, at the lowest priority (the game's
    /// own reads first), or stops doing so.
    pub fn set_fill(&self, on: bool) {
        self.fill.store(on, Ordering::Release);
    }

    /// Disconnects for good (another server is chosen), and returns at
    /// once: the client finishes in the background, its index saved last
    /// ([`Client::join`]). Reads still waiting stay so.
    pub fn stop(&self) {
        if self.stop.swap(true, Ordering::AcqRel) {
            return;
        }
        BEFORE.lock().unwrap_or_else(PoisonError::into_inner).push(self.finished.clone());
        // A sync, or a write stuck on it, fails at once.
        if let Some(s) = &*self.socket.lock().unwrap() {
            let _ = s.shutdown(Shutdown::Both);
        }
        self.connected.store(false, Ordering::Release);
        let _woken = self.woken.lock().unwrap();
        self.wake.notify_all();
    }

    /// Waits until the client, stopped, has finished.
    pub fn join(&self) {
        self.finished.wait();
    }

    #[cfg(test)]
    pub(crate) fn finished(&self) -> bool {
        self.finished.is_set()
    }

    /// Sleeps for `d`, or until the client stops.
    fn pause(&self, d: Duration) {
        let woken = self.woken.lock().unwrap();
        let _ = self.wake.wait_timeout_while(woken, d, |_| !self.stop.load(Ordering::Acquire));
    }

    /// Saves the copy's index (the app is closing).
    pub fn flush(&self) {
        if let Some(r) = self.ready.get()
            && let Err(e) = r.cache.save()
        {
            log::warn!("stream cache: saving the index: {e}");
        }
    }

    /// Reads `buf.len()` bytes of file `id` at `off` (fewer at its end),
    /// waiting for the server if they are not here. `None`: no such file.
    pub fn read(&self, id: u32, off: u64, buf: &mut [u8]) -> Option<usize> {
        self.read_inner(id, off, buf, None)
    }

    fn read_inner(
        &self,
        id: u32,
        off: u64,
        buf: &mut [u8],
        control: Option<&crate::packages::TransferControl>,
    ) -> Option<usize> {
        let r = self.ready.get()?;
        if id as usize >= r.cache.len() {
            return None;
        }
        let size = r.cache.size(id);
        let n = buf.len().min(size.saturating_sub(off) as usize);
        if n == 0 {
            return Some(0);
        }
        self.read_ahead(r, id, off, n as u64);
        match r.cache.read(id, off, &mut buf[..n]) {
            Ok(Some(got)) => {
                r.read_done(id);
                return Some(got);
            }
            Ok(None) => {}
            Err(e) => log::warn!("stream cache: reading {}: {e}", r.catalog.entries[id as usize].path),
        }
        let (first, last) = (off / BLOCK, (off + n as u64 - 1) / BLOCK);
        let started = Instant::now();
        if !self.in_time(r, id, first, last) {
            self.stats.cold.fetch_add(1, Ordering::Relaxed);
        }
        self.follow_boot(r, id, first);
        self.waiters.lock().unwrap().push(Waiter { file: id, first, last, thread: thread::current() });
        self.stats.waiting.fetch_add(1, Ordering::Relaxed);
        let mut asked: Option<Instant> = None;
        let mut logged = started;
        let got = loop {
            if control.is_some_and(|c| !c.wait_until_resumed()) {
                break None;
            }
            match r.cache.read(id, off, &mut buf[..n]) {
                Ok(Some(got)) => break Some(got),
                Ok(None) => {}
                Err(e) => {
                    log::warn!("stream cache: reading {}: {e}", r.catalog.entries[id as usize].path);
                    break None;
                }
            }
            // Asked again now and then: a new connection, an evicted block.
            if asked.is_none_or(|t| t.elapsed() >= Duration::from_secs(2)) {
                self.want(id, first, last, DEMAND);
                asked = Some(Instant::now());
            }
            if logged.elapsed() >= Duration::from_secs(5) {
                log::warn!(
                    "stream: waiting {:.0?} for {} at {off} ({n} bytes)",
                    started.elapsed(),
                    r.catalog.entries[id as usize].path
                );
                logged = Instant::now();
            }
            thread::park_timeout(Duration::from_millis(250));
        };
        self.waiters.lock().unwrap().retain(|w| w.thread.id() != thread::current().id() || w.file != id);
        self.stats.waiting.fetch_sub(1, Ordering::Relaxed);
        self.stats.waits.fetch_add(1, Ordering::Relaxed);
        self.stats.wait_us.fetch_add(started.elapsed().as_micros() as u64, Ordering::Relaxed);
        r.read_done(id);
        got
    }

    /// Whether the blocks `first..=last` of file `id` not here yet were all
    /// asked for [`IN_TIME`] before at least: the engine announced the read
    /// in time. (Its first block alone says little: it is often here, shared
    /// with the read before.)
    fn in_time(&self, r: &Ready, id: u32, first: u64, last: u64) -> bool {
        let inf = self.inflight.lock().unwrap();
        let mut b = first;
        while b <= last {
            if r.cache.has_block(id, b) {
                b += 1;
                continue;
            }
            match inf.covering(id, b).map(|q| inf.reqs[&q]) {
                Some(q) if q.asked.elapsed() >= IN_TIME => b = q.last + 1,
                _ => return false,
            }
        }
        true
    }

    /// The engine waits for a part of the boot set still at its place in the
    /// queue (`BOOT`): what follows it in the boot set comes next.
    fn follow_boot(&self, r: &Ready, id: u32, block: u64) {
        let Some(&k) = r.boot_runs.get(&id).and_then(|runs| runs.iter().find(|&&k| r.boot[k].1 <= block && block <= r.boot[k].2))
        else {
            return;
        };
        {
            let inf = self.inflight.lock().unwrap();
            if !inf.covering(id, block).is_some_and(|q| inf.reqs[&q].prio == BOOT) {
                return;
            }
        }
        self.ask(|out| {
            self.plan(id, block, r.boot[k].2, AHEAD, out, usize::MAX);
            let mut left = FOLLOW / BLOCK;
            for &(f, a, b) in &r.boot[k + 1..] {
                if left == 0 {
                    break;
                }
                self.plan(f, a, b, AHEAD, out, usize::MAX);
                left = left.saturating_sub(b - a + 1);
            }
        });
    }

    /// The engine will read this soon (`ahead`: may read it).
    pub fn hint(&self, id: u32, off: u64, len: u64, ahead: bool) {
        self.hints(&[(id, off, len, ahead)]);
    }

    /// [`Self::hint`] for several, asked for in one write.
    pub fn hints(&self, hints: &[(u32, u64, u64, bool)]) {
        let Some(r) = self.ready.get() else { return };
        let wanted: Vec<(u32, u64, u64, u8)> = hints
            .iter()
            .filter(|&&(id, off, len, _)| (id as usize) < r.cache.len() && len > 0 && off < r.cache.size(id))
            .map(|&(id, off, len, ahead)| {
                let last = (off + len - 1).min(r.cache.size(id) - 1) / BLOCK;
                (id, off / BLOCK, last, if ahead { AHEAD } else { HINT })
            })
            .collect();
        self.stats.hints.fetch_add(wanted.len() as u64, Ordering::Relaxed);
        // Those here already need no lock (most, once the copy fills).
        let missing: Vec<_> = wanted.into_iter().filter(|&(id, first, last, _)| !r.cache.has(id, first, last)).collect();
        if missing.is_empty() {
            return;
        }
        self.ask(|out| {
            for (id, first, last, prio) in missing {
                self.plan(id, first, last, prio, out, usize::MAX);
            }
        });
    }

    /// A read that continues the previous one of the file also fetches what
    /// follows, if the file is played over time (the engine had its previous
    /// read [`STREAM_GAP`] before at least: music is read 32 KB at a time
    /// every ~0.8 s, and waited a round trip for each piece; radio stations)
    /// or read in pieces of 64 KB or more (a smaller window). Smaller pieces
    /// read back to back get none: the next one is read before a read-ahead
    /// could come (read-ahead of every such run, up to 4 MB, fetched 520 MB
    /// in a cold run, 340 MB of it never read).
    fn read_ahead(&self, r: &Ready, id: u32, off: u64, n: u64) {
        let i = id as usize;
        let prev = r.seq_end[i].swap(off + n, Ordering::Relaxed);
        let since = (r.epoch.elapsed().as_micros() as u64).saturating_sub(r.seq_done[i].load(Ordering::Relaxed));
        // Radio stations skip a little between their ~33 KB blocks.
        let continues = prev != 0 && if r.radio[i] { off >= prev && off - prev <= 64 << 10 } else { off == prev };
        if !continues {
            r.ahead[i].store(0, Ordering::Relaxed);
            return;
        }
        let (first, max) = if since >= STREAM_GAP.as_micros() as u64 {
            ((4 * n).max(AHEAD_FIRST), AHEAD_MAX)
        } else if n >= 64 << 10 || r.radio[i] {
            (AHEAD_FIRST, PIECES_MAX)
        } else {
            return;
        };
        let ahead = match r.ahead[i].load(Ordering::Relaxed) {
            0 => first,
            a => 2 * a,
        }
        .min(max);
        r.ahead[i].store(ahead, Ordering::Relaxed);
        let (from, size) = (off + n, r.cache.size(id));
        if from < size {
            self.want(id, from / BLOCK, ((from + ahead).min(size) - 1) / BLOCK, AHEAD);
        }
    }

    /// Makes sure blocks `first..=last` of file `id` are here or on their
    /// way at `prio` at least.
    fn want(&self, id: u32, first: u64, last: u64, prio: u8) {
        // Here already (read-ahead of what is cached, most hints): no lock.
        if let Some(r) = self.ready.get()
            && first < r.cache.blocks(id)
            && r.cache.has(id, first, last.min(r.cache.blocks(id) - 1))
        {
            return;
        }
        self.ask(|out| {
            self.plan(id, first, last, prio, out, usize::MAX);
        });
    }

    /// Plans requests and writes them with `sending` held throughout: a
    /// request is on the wire before any PRIORITY another thread plans for
    /// it (the server ignores one for a request it has not seen, and the
    /// raise would be lost for good).
    fn ask(&self, plan: impl FnOnce(&mut Vec<Header>)) {
        let _one = self.sending.lock().unwrap();
        let mut out = Vec::new();
        plan(&mut out);
        if !out.is_empty() {
            self.write(&encode(&out));
        }
    }

    /// Adds to `out` the requests (at most `limit` new ones) that bring
    /// blocks `first..=last` of file `id`, or raise those on their way to
    /// `prio`. Returns the block it stopped at (past `last` when done).
    fn plan(&self, id: u32, first: u64, last: u64, prio: u8, out: &mut Vec<Header>, limit: usize) -> u64 {
        let Some(r) = self.ready.get() else { return u64::MAX };
        let blocks = r.cache.blocks(id);
        if blocks == 0 || first >= blocks {
            return u64::MAX;
        }
        let last = last.min(blocks - 1);
        let size = r.cache.size(id);
        let mut inf = self.inflight.lock().unwrap();
        let mut b = first;
        let mut added = 0;
        while b <= last {
            if added >= limit {
                return b;
            }
            if r.cache.has_block(id, b) {
                // A word of blocks at a time (a file being filled is scanned
                // from its start each pass).
                b = r.cache.first_missing(id, b, last);
                continue;
            }
            if let Some(req) = inf.covering(id, b) {
                let q = inf.reqs.get_mut(&req).unwrap();
                if prio < q.prio {
                    q.prio = prio;
                    out.push(Header { kind: PRIORITY, prio, req, ..Default::default() });
                }
                b = q.last + 1;
                continue;
            }
            let start = b;
            while b <= last && b - start < MAX_RUN && !r.cache.has_block(id, b) && inf.covering(id, b).is_none() {
                b += 1;
            }
            let req = inf.add(Req { file: id, first: start, last: b - 1, prio, asked: Instant::now() });
            let off = start * BLOCK;
            let len = (b * BLOCK).min(size) - off;
            out.push(Header { kind: READ, prio, req, off, len: len as u32, file: id, crc: 0 });
            added += 1;
        }
        b
    }

    /// Writes requests on the connection, if there is one (`sending` held).
    /// One that takes none for [`SILENCE`] (the socket's write timeout: the
    /// server stopped reading) is dead: shut, its session then ends.
    fn write(&self, buf: &[u8]) {
        let Some(s) = self.conn.lock().unwrap().clone() else { return };
        if io::Write::write_all(&mut &*s, buf).is_err() {
            let _ = s.shutdown(Shutdown::Both);
            *self.conn.lock().unwrap() = None;
            self.connected.store(false, Ordering::Release);
        }
    }

    fn set_phase(&self, p: Phase) {
        *self.phase.lock().unwrap() = p;
    }

    /// Waits for the clients stopped before it (its copy may be one of
    /// theirs), serves until stopped, then finishes: its other threads end,
    /// the index is saved one last time, and the copy's files and lock are
    /// closed (with the client, if this thread had the last of it: the app
    /// lets go of a client it stops) before it says it has finished. Work
    /// on the folder waits for that: Windows moves no folder with a file
    /// open in it.
    fn run(self: Arc<Self>, before: Vec<Arc<Done>>, threads: [JoinHandle<()>; 2]) {
        let over = Over(self.finished.clone());
        for done in before {
            done.wait();
        }
        self.sessions();
        for t in threads {
            let _ = t.join();
        }
        self.flush();
        drop(self);
        drop(over);
    }

    /// Connects, syncs, serves; again after every lost connection.
    fn sessions(&self) {
        let mut backoff = Duration::from_millis(500);
        while !self.stop.load(Ordering::Acquire) {
            let mut served = false;
            let result = self.session(&mut served);
            let failed = matches!(*self.phase.lock().unwrap(), Phase::Failed(_));
            *self.socket.lock().unwrap() = None;
            *self.conn.lock().unwrap() = None;
            self.connected.store(false, Ordering::Release);
            if self.stop.load(Ordering::Acquire) {
                return;
            }
            if let Err(e) = &result {
                let msg = format!("{e:#}");
                if self.error.lock().unwrap().as_deref() != Some(&msg) {
                    log::warn!("stream: {}: {msg}", self.opts.server);
                }
                *self.error.lock().unwrap() = Some(msg);
            }
            if failed {
                return;
            }
            if !self.is_ready() {
                self.set_phase(Phase::Connecting);
                // A copy from an earlier run can play while the server is away.
                if !self.tried_offline.swap(true, Ordering::AcqRel)
                    && let Err(e) = self.ready_offline()
                {
                    log::info!("stream: no copy to play from without the server: {e:#}");
                }
            }
            // A connection that served is made again at once; one that did
            // not, after a pause that grows.
            if served {
                backoff = Duration::from_millis(500);
            } else {
                self.pause(backoff);
                backoff = (backoff * 2).min(Duration::from_secs(5));
            }
        }
    }

    /// `served`: the copy was ready and requests went on the connection.
    fn session(&self, served: &mut bool) -> Result<()> {
        let (stream, addr) = connect(&self.opts.server)?;
        *self.socket.lock().unwrap() = Some(stream.try_clone()?);
        if self.stop.load(Ordering::Acquire) {
            bail!("stopped");
        }
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(SILENCE))?;
        let mut reader = BufReader::with_capacity(256 << 10, stream.try_clone()?);
        let hello = read_hello(&mut reader)?;
        if let Some(why) = unusable(&hello, &self.opts.build) {
            self.set_phase(Phase::Failed(why.clone()));
            bail!(why);
        }
        log::info!("stream: connected to {addr}");
        *self.error.lock().unwrap() = None;
        let mut sync = Sync { reader, stream: stream.try_clone()?, next: 1 << 31, blobs: HashMap::new() };
        match self.ready.get() {
            None => self.sync(&mut sync, &hello)?,
            // Unless the whole game is here, which needs nothing from it.
            Some(r) if hello["catalog"].as_str() != Some(&r.catalog.hash) && !self.complete.load(Ordering::Acquire) => {
                self.set_phase(Phase::Failed("the game's files changed on the server: restart the app".into()));
                bail!("the catalog changed");
            }
            Some(r)
                if hello["server_id"].as_str() != r.package_server_id.as_deref()
                    || hello["packages_hash"].as_str() != r.package_hash.as_deref() =>
            {
                self.set_phase(Phase::Failed("optional package metadata changed on the server: restart the app".into()));
                bail!("the optional package manifest changed");
            }
            Some(_) => {}
        }
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        {
            // What was in flight on the last connection goes first, in the
            // order it was asked (the server queues it so, and reads its
            // disk in that order). Taken once the connection is there, with
            // its writes held: a request planned meanwhile is either among
            // these or sent after them (maybe both: answered twice).
            let _one = self.sending.lock().unwrap();
            *self.conn.lock().unwrap() = Some(Arc::new(stream));
            self.connected.store(true, Ordering::Release);
            let mut again: Vec<Header> = {
                let inf = self.inflight.lock().unwrap();
                let r = self.ready.get().unwrap();
                inf.reqs
                    .iter()
                    .map(|(&req, q)| {
                        let off = q.first * BLOCK;
                        let len = ((q.last + 1) * BLOCK).min(r.cache.size(q.file)) - off;
                        Header { kind: READ, prio: q.prio, req, off, len: len as u32, file: q.file, crc: 0 }
                    })
                    .collect()
            };
            again.sort_by_key(|h| h.req);
            self.write(&encode(&again));
        }
        *served = true;
        // Then the boot set's rest.
        self.prefetch_boot();
        self.receive(sync.reader)
    }

    /// Takes the catalog and the packs, opens the copy.
    fn sync(&self, s: &mut Sync, hello: &serde_json::Value) -> Result<()> {
        let size = |name: &str| hello["blobs"][name].as_u64();
        // Asked together: one round trip, not one each.
        let boot_name = if self.opts.low_memory && size("bootset_low").is_some() { "bootset_low" } else { "bootset" };
        let mut requested = vec![("catalog", size("catalog").context("no catalog")?)];
        let boot_req_index = size(boot_name).map(|n| {
            requested.push((boot_name, n));
            requested.len() - 1
        });
        let package_req_index = size(packages::MANIFEST_BLOB).and_then(|n| {
            if n > packages::MAX_MANIFEST_BYTES as u64 {
                log::warn!("stream: ignoring oversized optional package manifest ({n} bytes)");
                None
            } else {
                requested.push((packages::MANIFEST_BLOB, n));
                Some(requested.len() - 1)
            }
        });
        let first = s.ask(&requested)?;
        let compressed = s.take(first[0], &|_, _| {})?;
        let json = unpack_json(&compressed, "the catalog")?;
        let catalog = Catalog::from_json(&json)?;
        if catalog.build != self.opts.build {
            bail!("the catalog is for build {}", catalog.build);
        }
        // A copy from an earlier run (a whole download too) follows the
        // server's list: files that still match are kept, changed ones come
        // again. The server is the authority on what the game is.
        fs::create_dir_all(&self.opts.root)?;
        let cache = Cache::open(&self.opts.root, &catalog)?;
        let mut packs = read_json(&self.opts.root.join("packs.json"));
        let mut needed = Vec::new();
        for kind in Kind::ALL {
            let hash = hello["packs"][kind.name()]["hash"].as_str().unwrap_or_default().to_string();
            let members = pack::members(&catalog, kind);
            if packs[kind.name()].as_str() != Some(&hash) || !members.iter().all(|&i| cache.complete(i)) {
                needed.push((kind, hash, size(kind.name()).context("no pack")?));
            }
        }
        let started = Instant::now();
        let asked = s.ask(&needed.iter().map(|(kind, _, total)| (kind.name(), *total)).collect::<Vec<_>>())?;
        for ((kind, hash, total), req) in needed.into_iter().zip(asked) {
            let what = kind.name();
            let progress = |done, total| self.set_phase(Phase::Syncing { what, done, total });
            let data = s.take(req, &progress)?;
            let mut files = 0;
            pack::unpack(&data[..], |id, bytes| {
                if (id as usize) < catalog.entries.len() && bytes.len() as u64 == catalog.entries[id as usize].size {
                    cache.write_whole(id, bytes)?;
                    files += 1;
                }
                Ok(())
            })?;
            log::info!("stream: {what} pack: {files} files, {:.1} MB in {:.1?}", total as f64 / 1e6, started.elapsed());
            packs[kind.name()] = hash.into();
            cache.save()?;
            fs::write(self.opts.root.join("packs.json"), serde_json::to_vec(&packs)?)?;
        }
        let boot = match boot_req_index.map(|index| first[index]) {
            Some(req) => {
                let boot = unpack_json(&s.take(req, &|_, _| {})?, "the boot set")?;
                fs::write(self.opts.root.join(BOOT_SET), &boot)?;
                boot_runs(&catalog, &boot)
            }
            None => Vec::new(),
        };
        let package_server_id = hello["server_id"].as_str().map(str::to_owned);
        let package_hash = hello["packages_hash"].as_str().map(str::to_owned);
        let package_manifest = if let Some(index) = package_req_index {
            let bytes = s.take(first[index], &|_, _| {})?;
            let actual_hash = crate::catalog::hex(&Sha256::digest(&bytes));
            let expected_server = package_server_id.as_deref();
            let hash_matches = package_hash.as_deref() == Some(actual_hash.as_str());
            match (expected_server, hash_matches) {
                (Some(server_id), true) => match PackageManifest::parse(&bytes, &catalog, Some(server_id)) {
                    Ok(manifest) => {
                        fs::write(self.opts.root.join(packages::MANIFEST_FILE), &bytes)?;
                        Some(manifest)
                    }
                    Err(e) => {
                        log::warn!("stream: ignoring invalid optional package manifest: {e:#}");
                        let _ = fs::remove_file(self.opts.root.join(packages::MANIFEST_FILE));
                        None
                    }
                },
                (Some(_), false) => {
                    log::warn!("stream: ignoring optional package manifest with mismatched SHA-256");
                    let _ = fs::remove_file(self.opts.root.join(packages::MANIFEST_FILE));
                    None
                }
                (None, _) => {
                    log::warn!("stream: ignoring package manifest without HELLO server_id");
                    let _ = fs::remove_file(self.opts.root.join(packages::MANIFEST_FILE));
                    None
                }
            }
        } else {
            let _ = fs::remove_file(self.opts.root.join(packages::MANIFEST_FILE));
            None
        };
        if let Some(manifest) = &package_manifest {
            reconcile_package_cache(&self.opts.root, &catalog, &cache, manifest)?;
        } else {
            cache.set_pinned(&[])?;
        }
        fs::write(self.opts.root.join(crate::cache::CATALOG), &json)?;
        self.make_ready(catalog, cache, boot, package_manifest, package_server_id, package_hash);
        Ok(())
    }

    /// Plays from the copy of an earlier run while the server is away (the
    /// packs must be complete; anything else missing waits for it).
    fn ready_offline(&self) -> Result<()> {
        let json = fs::read(self.opts.root.join(crate::cache::CATALOG))?;
        let catalog = Catalog::from_json(&json)?;
        if catalog.build != self.opts.build {
            bail!("the copy is of build {}", catalog.build);
        }
        let cache = Cache::open(&self.opts.root, &catalog)?;
        for kind in Kind::ALL {
            if !pack::members(&catalog, kind).iter().all(|&i| cache.complete(i)) {
                bail!("the copy lacks the {} pack", kind.name());
            }
        }
        log::info!("stream: playing from the copy until {} answers", self.opts.server);
        let boot = fs::read(self.opts.root.join(BOOT_SET)).map(|b| boot_runs(&catalog, &b)).unwrap_or_default();
        let (packages, package_server_id, package_hash) = match fs::read(self.opts.root.join(packages::MANIFEST_FILE)) {
            Ok(bytes) => {
                let value = serde_json::from_slice::<serde_json::Value>(&bytes).ok();
                let server_id = value.as_ref().and_then(|v| v["server_id"].as_str()).map(str::to_owned);
                let manifest = server_id.as_deref().and_then(|id| PackageManifest::parse(&bytes, &catalog, Some(id)).ok());
                let hash = manifest.as_ref().map(|_| crate::catalog::hex(&Sha256::digest(&bytes)));
                (manifest, server_id.filter(|_| hash.is_some()), hash)
            }
            Err(_) => (None, None, None),
        };
        if let Some(manifest) = &packages {
            reconcile_package_cache(&self.opts.root, &catalog, &cache, manifest)?;
        } else {
            cache.set_pinned(&[])?;
        }
        self.make_ready(catalog, cache, boot, packages, package_server_id, package_hash);
        Ok(())
    }

    fn make_ready(
        &self,
        catalog: Catalog,
        cache: Cache,
        boot: Vec<(u32, u64, u64)>,
        packages: Option<PackageManifest>,
        package_server_id: Option<String>,
        package_hash: Option<String>,
    ) {
        let n = catalog.entries.len();
        let radio = catalog.entries.iter().map(|e| e.path.to_ascii_lowercase().contains("/radio_")).collect();
        let manifest = catalog.engine_manifest();
        let mut boot_runs: HashMap<u32, Vec<usize>> = HashMap::new();
        for (k, &(id, _, _)) in boot.iter().enumerate() {
            boot_runs.entry(id).or_default().push(k);
            cache.keep_longer(id);
        }
        let ready = Ready {
            catalog,
            cache,
            packages,
            package_server_id,
            package_hash,
            manifest,
            boot,
            boot_runs,
            seq_end: (0..n).map(|_| AtomicU64::new(0)).collect(),
            seq_done: (0..n).map(|_| AtomicU64::new(0)).collect(),
            epoch: Instant::now(),
            ahead: (0..n).map(|_| AtomicU64::new(0)).collect(),
            radio,
        };
        // A copy complete from an earlier run (Cache::open finished it).
        let complete = ready.cache.all_here();
        if self.ready.set(ready).is_ok() {
            self.complete.store(complete, Ordering::Release);
            self.set_phase(Phase::Ready);
        }
    }

    fn prefetch_boot(&self) {
        let Some(r) = self.ready.get() else { return };
        self.ask(|out| {
            for &(id, first, last) in &r.boot {
                self.plan(id, first, last, BOOT, out, usize::MAX);
            }
            if !out.is_empty() {
                log::info!("stream: boot set: {} requests", out.len());
            }
        });
    }

    /// Takes the server's answers until the connection ends.
    fn receive(&self, mut reader: BufReader<TcpStream>) -> Result<()> {
        let r = self.ready.get().unwrap();
        let mut buf = vec![0u8; CHUNK as usize];
        let mut since_check = 0u64;
        // Requests with a chunk that came damaged.
        let mut corrupt: Vec<u32> = Vec::new();
        let mut last_data = Instant::now();
        loop {
            if self.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            let Some(h) = next_header(&mut reader).context("connection lost")? else {
                let since = self.inflight.lock().unwrap().since;
                if since.is_some_and(|t| t.max(last_data).elapsed() > SILENCE) {
                    bail!("the server stopped answering");
                }
                continue;
            };
            match h.kind {
                DATA => {
                    if h.len > CHUNK {
                        bail!("a chunk of {} bytes", h.len);
                    }
                    read_exact_retrying(&mut reader, &mut buf[..h.len as usize], SILENCE)?;
                    last_data = Instant::now();
                    self.stats.received.fetch_add(h.len as u64, Ordering::Relaxed);
                    // The header has no checksum of its own: a chunk is kept
                    // only within its request's file and range (one stored
                    // elsewhere would stay wrong in the copy). One of a request
                    // that ended was answered twice (asked again at a
                    // reconnect): not needed.
                    let fits = self.inflight.lock().unwrap().reqs.get(&h.req).map(|q| {
                        let end = ((q.last + 1) * BLOCK).min(r.cache.size(q.file));
                        q.file == h.file && h.off >= q.first * BLOCK && h.off.checked_add(h.len as u64).is_some_and(|e| e <= end)
                    });
                    if fits.is_none() {
                        continue;
                    }
                    if crc(&buf[..h.len as usize]) != h.crc || fits == Some(false) {
                        // Not kept: the request is asked again when it ends.
                        log::warn!("stream: a damaged chunk of request {} (offset {}): asked again", h.req, h.off);
                        if !corrupt.contains(&h.req) {
                            corrupt.push(h.req);
                        }
                        continue;
                    }
                    self.store(r, h.file, h.off, &buf[..h.len as usize]);
                    since_check += h.len as u64;
                    if since_check >= 64 << 20 {
                        since_check = 0;
                        self.make_room(r);
                    }
                }
                END => {
                    let req = self.inflight.lock().unwrap().remove(h.req);
                    if let Some(at) = corrupt.iter().position(|&c| c == h.req) {
                        corrupt.swap_remove(at);
                        if let Some(q) = req {
                            self.want(q.file, q.first, q.last, q.prio);
                        }
                        continue;
                    }
                    if h.len != DONE
                        && let Some(q) = req
                    {
                        log::warn!(
                            "stream: {} blocks {}..={}: the server answered {}",
                            r.catalog.entries[q.file as usize].path,
                            q.first,
                            q.last,
                            match h.len {
                                SHORT => "the file is shorter",
                                UNKNOWN => "no such file",
                                _ => "read error",
                            }
                        );
                    }
                }
                k => bail!("unexpected message {k}"),
            }
        }
    }

    fn store(&self, r: &Ready, id: u32, off: u64, data: &[u8]) {
        let mut result = r.cache.write(id, off, data);
        // Not for the whole game: it would empty what it downloaded.
        if let Err(e) = &result
            && is_full(e)
            && !self.fill.load(Ordering::Acquire)
            && !self.complete.load(Ordering::Acquire)
        {
            self.empty(r, FLOOR + ROOM);
            result = r.cache.write(id, off, data);
        }
        if let Err(e) = result {
            log::error!("stream cache: writing {}: {e}", r.catalog.entries[id as usize].path);
        }
        let (first, last) = (off / BLOCK, (off + data.len() as u64).div_ceil(BLOCK));
        for w in self.waiters.lock().unwrap().iter() {
            if w.file == id && w.first <= last && first <= w.last {
                w.thread.unpark();
            }
        }
    }

    /// The full download: keeps [`FILL_REQUESTS`] requests on their way,
    /// file after file, until every block is here.
    fn fill(self: Arc<Self>) {
        let mut cursor = 0u32;
        while !self.stop.load(Ordering::Acquire) {
            let connected = self.connected.load(Ordering::Acquire);
            let Some(r) = self.ready.get().filter(|_| connected && self.fill.load(Ordering::Acquire)) else {
                self.pause(Duration::from_millis(500));
                continue;
            };
            if self.complete.load(Ordering::Acquire) {
                self.pause(Duration::from_secs(1));
                continue;
            }
            let no_room = r.cache.free_space().is_some_and(|free| free < FLOOR);
            self.no_room.store(no_room, Ordering::Release);
            if no_room {
                self.pause(Duration::from_secs(2));
                continue;
            }
            let outstanding = self.inflight.lock().unwrap().reqs.values().filter(|q| q.prio == FILL).count();
            let mut budget = FILL_REQUESTS.saturating_sub(outstanding);
            let n = r.cache.len() as u32;
            let (mut visited, mut incomplete) = (0, 0);
            self.ask(|out| {
                while budget > 0 && visited < n {
                    visited += 1;
                    if !r.cache.complete(cursor) {
                        incomplete += 1;
                        let before = out.len();
                        let stopped = self.plan(cursor, 0, u64::MAX, FILL, out, budget);
                        budget -= out.len() - before;
                        if stopped < r.cache.blocks(cursor) {
                            // The budget ran out in this file: on with it next time.
                            break;
                        }
                    }
                    cursor = (cursor + 1) % n.max(1);
                }
            });
            if visited == n && incomplete == 0 && outstanding == 0 {
                match r.cache.finish(&r.catalog) {
                    Ok(()) => {
                        self.complete.store(true, Ordering::Release);
                        log::info!("stream: the whole game is on this device ({:.1} GB)", r.cache.total() as f64 / 1e9);
                    }
                    // Still a partial copy (its marker says so): tried again.
                    Err(e) => {
                        log::warn!("stream cache: finishing the whole game: {e}");
                        self.pause(Duration::from_secs(2));
                    }
                }
            }
            self.pause(Duration::from_millis(100));
        }
    }

    /// Keeps the volume above [`FLOOR`] (not while the whole game is
    /// wanted: then the download waits for room).
    fn make_room(&self, r: &Ready) {
        // Nor from a complete copy: a game folder, maybe the user's only one.
        if self.fill.load(Ordering::Acquire) || self.complete.load(Ordering::Acquire) {
            return;
        }
        match r.cache.free_space() {
            Some(free) if free < FLOOR => self.empty(r, FLOOR + ROOM - free),
            _ => self.no_gain.store(false, Ordering::Release),
        }
    }

    /// Empties files for `want` bytes, unless that made no room the last
    /// time: something else fills the volume, or the system keeps the space
    /// (snapshots); emptying on would give away the whole copy, the packs
    /// too, for nothing.
    fn empty(&self, r: &Ready, want: u64) {
        if self.no_gain.load(Ordering::Acquire) {
            return;
        }
        let before = r.cache.free_space();
        if r.cache.evict(want) > 0
            && let (Some(before), Some(after)) = (before, r.cache.free_space())
            && after <= before
        {
            log::warn!("stream cache: emptying files made no room: none emptied until the volume has room again");
            self.no_gain.store(true, Ordering::Release);
        }
    }

    /// Saves the index (see [`SAVE_BUSY`]); logs the traffic every ten seconds.
    fn keep(self: Arc<Self>) {
        let counters = |s: &Stats| {
            [&s.received, &s.waits, &s.wait_us, &s.cold, &s.hints].map(|c| c.load(Ordering::Relaxed))
        };
        let (mut last, mut at) = ([0u64; 5], Instant::now());
        // When the index was saved, the bytes received then, and two seconds ago.
        let (mut saved, mut saved_at, mut received) = (Instant::now(), 0, 0);
        loop {
            self.pause(Duration::from_secs(2));
            // The last save is the run thread's, once this one ended.
            if self.stop.load(Ordering::Acquire) {
                return;
            }
            let Some(r) = self.ready.get() else { continue };
            let now = self.stats.received.load(Ordering::Relaxed);
            let flowing = std::mem::replace(&mut received, now) != now;
            if !flowing || saved.elapsed() >= SAVE_BUSY || now.saturating_sub(saved_at) >= SAVE_BYTES {
                if let Err(e) = r.cache.save() {
                    log::warn!("stream cache: saving the index: {e}");
                }
                (saved, saved_at) = (Instant::now(), now);
            }
            if at.elapsed() < Duration::from_secs(10) {
                continue;
            }
            let now = counters(&self.stats);
            let [got, waits, wait_us, cold, hints] = std::array::from_fn(|i| now[i] - last[i]);
            if got > 0 || waits > 0 {
                log::info!(
                    "stream: {:.1} MB/s; {waits} engine reads waited, {:.0} ms each, {cold} of them not announced in time; \
                     {hints} announcements; {} requests in flight; copy {:.2} GB",
                    got as f64 / 1e6 / at.elapsed().as_secs_f64(),
                    if waits > 0 { wait_us as f64 / 1e3 / waits as f64 } else { 0.0 },
                    self.inflight.lock().unwrap().reqs.len(),
                    r.cache.used() as f64 / 1e9
                );
            }
            (last, at) = (now, Instant::now());
        }
    }
}

/// Clients stopped and not finished yet, and work on copies' folders not
/// done yet: what a client started now waits for.
static BEFORE: Mutex<Vec<Arc<Done>>> = Mutex::new(Vec::new());

fn pending() -> Vec<Arc<Done>> {
    let mut before = BEFORE.lock().unwrap_or_else(PoisonError::into_inner);
    before.retain(|d| !d.is_set());
    before.clone()
}

/// Runs `work` on copies' folders (a copy moved, or deleted) in the
/// background, once the clients stopped before have finished and the work
/// asked before is done; a client started after it waits for it.
pub fn after_stopped(work: impl FnOnce() + Send + 'static) {
    let done = Arc::new(Done::default());
    let before = {
        let mut before = BEFORE.lock().unwrap_or_else(PoisonError::into_inner);
        before.retain(|d| !d.is_set());
        let earlier = before.clone();
        before.push(done.clone());
        earlier
    };
    let over = Over(done);
    let spawned = qos::spawn("stream copy", Qos::Utility, move || {
        let _over = over;
        for done in before {
            done.wait();
        }
        work();
    });
    if let Err(e) = spawned {
        log::warn!("stream: no thread for the copy's folder: {e}");
    }
}

/// Set once something is over: a client finished, work on a copy done.
#[derive(Default)]
struct Done {
    over: Mutex<bool>,
    cv: Condvar,
}

impl Done {
    fn set(&self) {
        *self.over.lock().unwrap_or_else(PoisonError::into_inner) = true;
        self.cv.notify_all();
    }

    fn is_set(&self) -> bool {
        *self.over.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn wait(&self) {
        let mut over = self.over.lock().unwrap_or_else(PoisonError::into_inner);
        while !*over {
            over = self.cv.wait(over).unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// Sets its [`Done`] when dropped: however its thread ends.
struct Over(Arc<Done>);

impl Drop for Over {
    fn drop(&mut self) {
        self.0.set();
    }
}

fn file_hash_matches(path: &Path, size: u64, expected: &str) -> Result<bool> {
    Ok(hash_file_matches(path, size, expected, None, |_| {})?.unwrap_or(false))
}

fn hash_file_matches(
    path: &Path,
    size: u64,
    expected: &str,
    control: Option<&TransferControl>,
    mut progress: impl FnMut(u64),
) -> Result<Option<bool>> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Some(false)),
        Err(e) => return Err(e.into()),
    };
    if metadata.len() != size {
        return Ok(Some(false));
    }
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut hashed = 0u64;
    loop {
        if let Some(control) = control
            && !control.wait_until_resumed()
        {
            return Ok(None);
        }
        let n = file.read(&mut buffer)?;
        if n == 0 { break; }
        hash.update(&buffer[..n]);
        hashed += n as u64;
        progress(hashed);
    }
    Ok(Some(crate::catalog::hex(&hash.finalize()) == expected))
}

fn package_fingerprint(package: &crate::packages::GamePackage) -> String {
    let mut hash = Sha256::new();
    hash.update(package.id.as_bytes());
    hash.update([0]);
    hash.update(package.version.as_bytes());
    hash.update([0]);
    hash.update(package.target_build.as_bytes());
    for file in &package.files {
        hash.update(file.path.as_bytes());
        hash.update([0]);
        hash.update(file.size.to_le_bytes());
        hash.update(file.sha256.as_bytes());
    }
    crate::catalog::hex(&hash.finalize())
}

fn load_package_state(path: &Path, manifest: &PackageManifest) -> (serde_json::Value, bool) {
    let fresh = || serde_json::json!({
        "schema_version": 1,
        "server_id": manifest.server_id,
        "build": manifest.build,
        "catalog_hash": manifest.catalog_hash,
        "installed": {},
    });
    let Ok(bytes) = fs::read(path) else { return (fresh(), false) };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else { return (fresh(), false) };
    let matches = value["schema_version"].as_u64() == Some(1)
        && value["server_id"].as_str() == Some(manifest.server_id.as_str())
        && value["build"].as_str() == Some(manifest.build.as_str())
        && value["catalog_hash"].as_str() == Some(manifest.catalog_hash.as_str())
        && value["installed"].is_object();
    if matches { (value, true) } else { (fresh(), false) }
}

fn save_package_state(path: &Path, state: &serde_json::Value) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_vec(state)?)?;
    fs::File::open(&tmp)?.sync_all()?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn reconcile_package_cache(root: &Path, catalog: &Catalog, cache: &Cache, manifest: &PackageManifest) -> Result<()> {
    let state_path = root.join("packages-state.json");
    let old_state = fs::read(&state_path).ok().and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let same_identity = old_state.as_ref().is_some_and(|state| {
        state["schema_version"].as_u64() == Some(1)
            && state["server_id"].as_str() == Some(manifest.server_id.as_str())
            && state["build"].as_str() == Some(manifest.build.as_str())
            && state["catalog_hash"].as_str() == Some(manifest.catalog_hash.as_str())
            && state["installed"].is_object()
    });
    let mut new_hashes = HashMap::new();
    for package in &manifest.packages {
        for file in &package.files {
            new_hashes.insert(file.path.as_str(), file.sha256.as_str());
        }
    }
    if let Some(old_state) = &old_state
        && let Some(installed) = old_state["installed"].as_object()
    {
        for receipt in installed.values() {
            if let Some(files) = receipt["files"].as_array() {
                for file in files {
                    let Some(path) = file["path"].as_str() else { continue };
                    let Some(old_hash) = file["sha256"].as_str() else { continue };
                    if new_hashes.get(path).is_some_and(|&new_hash| new_hash != old_hash)
                        && let Some(id) = catalog.find(path)
                    {
                        cache.invalidate(id)?;
                    }
                }
            }
        }
    }
    if same_identity {
        let (state, _) = load_package_state(&state_path, manifest);
        cache.set_pinned(&installed_pin_ids(manifest, &state))?;
    } else {
        cache.set_pinned(&[])?;
    }
    Ok(())
}

fn installed_pin_ids(manifest: &PackageManifest, state: &serde_json::Value) -> Vec<u32> {
    let mut pins = HashSet::new();
    for package in &manifest.packages {
        let receipt = &state["installed"][package.id.as_str()];
        let fingerprint = package_fingerprint(package);
        if receipt["fingerprint"].as_str() == Some(fingerprint.as_str()) {
            for file in &package.files {
                pins.insert(file.catalog_id);
            }
        }
    }
    pins.into_iter().collect()
}

/// Blob requests of the sync, answered before anything else is asked.
struct Sync {
    reader: BufReader<TcpStream>,
    stream: TcpStream,
    next: u32,
    /// Blobs asked for: name, size, what came, whether all of it did.
    blobs: HashMap<u32, (String, u64, Vec<u8>, bool)>,
}

impl Sync {
    /// Asks for blobs (name, size), all in one write; their request numbers.
    fn ask(&mut self, blobs: &[(&str, u64)]) -> Result<Vec<u32>> {
        let mut msg = Vec::new();
        let mut reqs = Vec::new();
        for &(name, size) in blobs {
            if size > MAX_BLOB {
                bail!("{name} of {size} bytes");
            }
            self.next += 1;
            let req = self.next;
            msg.extend_from_slice(&Header { kind: BLOB, prio: HINT, req, len: name.len() as u32, ..Default::default() }.encode());
            msg.extend_from_slice(name.as_bytes());
            self.blobs.insert(req, (name.to_string(), size, Vec::with_capacity(size.min(16 << 20) as usize), false));
            reqs.push(req);
        }
        io::Write::write_all(&mut self.stream, &msg)?;
        Ok(reqs)
    }

    /// Blob `req`, once all of it is here (taking the others' chunks meanwhile).
    fn take(&mut self, req: u32, progress: &dyn Fn(u64, u64)) -> Result<Vec<u8>> {
        loop {
            match self.blobs.get(&req) {
                Some((_, _, _, true)) => return Ok(self.blobs.remove(&req).unwrap().2),
                Some(_) => {}
                None => bail!("blob {req} was never asked for"),
            }
            let h = Header::read(&mut self.reader).context("taking the game's lists and packs")?;
            let Some((name, size, data, done)) = self.blobs.get_mut(&h.req) else {
                bail!("unexpected message {} for request {}", h.kind, h.req);
            };
            match h.kind {
                DATA => {
                    if h.len > CHUNK {
                        bail!("a chunk of {} bytes", h.len);
                    }
                    let at = data.len();
                    if at as u64 + h.len as u64 > *size {
                        bail!("{name} came longer than its {size} bytes");
                    }
                    data.resize(at + h.len as usize, 0);
                    self.reader.read_exact(&mut data[at..])?;
                    if crc(&data[at..]) != h.crc {
                        bail!("{name} came damaged");
                    }
                    if h.req == req {
                        progress(data.len() as u64, *size);
                    }
                }
                END => {
                    if h.len != DONE || data.len() as u64 != *size {
                        bail!("the server could not send {name}");
                    }
                    *done = true;
                }
                k => bail!("unexpected message {k} while taking {name}"),
            }
        }
    }
}

/// What a server answers before anything streams from it: a server found on
/// the network, checked before it is chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check {
    /// It serves this game; connecting took `ms`. `at`: what its name stands
    /// for (to know it under another name, or its address).
    Ready { ms: u32, at: Vec<SocketAddr> },
    /// It answers but cannot serve this app: why.
    Unusable(String),
    /// No answer: why.
    Unreachable(String),
}

/// Asks `server` for its greeting: whether it serves `build`, and how far it is.
pub fn check(server: &str, build: &str) -> Check {
    let started = Instant::now();
    let at = resolve(server).unwrap_or_default();
    let stream = match connect(server) {
        Ok((stream, _)) => stream,
        Err(e) => return Check::Unreachable(format!("{e:#}")),
    };
    let ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    match read_hello(&mut BufReader::new(stream)) {
        Ok(hello) => unusable(&hello, build).map_or(Check::Ready { ms, at }, Check::Unusable),
        Err(e) if e.to_string() == NOT_A_SERVER => Check::Unusable(NOT_A_SERVER.into()),
        Err(e) => Check::Unreachable(format!("{e:#}")),
    }
}

const NOT_A_SERVER: &str = "not a Glue 5 server";

/// The server's greeting, the first message on a connection.
fn read_hello(reader: &mut impl Read) -> Result<serde_json::Value> {
    let h = Header::read(reader).context("waiting for the server")?;
    if h.kind != HELLO {
        bail!(NOT_A_SERVER);
    }
    Ok(serde_json::from_slice(&payload(reader, h.len, MAX_MESSAGE)?)?)
}

/// Why a server with this greeting cannot serve an app running `build`.
fn unusable(hello: &serde_json::Value, build: &str) -> Option<String> {
    if hello["protocol"].as_u64() != Some(PROTOCOL) {
        Some(format!("the server speaks protocol {}, this app {PROTOCOL}", hello["protocol"]))
    } else if hello["build"].as_str() != Some(build) {
        Some(format!("the server has game build {}, this app runs {build}", hello["build"].as_str().unwrap_or("?")))
    } else {
        None
    }
}

/// Connects to the first of the server's addresses that answers, IPv4 first:
/// a Mac's `.local` name (as Bonjour finds it) also stands for its IPv6
/// link-local address, where a server listening on IPv4 refuses.
fn connect(server: &str) -> Result<(TcpStream, SocketAddr)> {
    let mut addrs = resolve(server)?;
    addrs.sort_by_key(SocketAddr::is_ipv6);
    let mut last = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
            Ok(stream) => return Ok((stream, addr)),
            Err(e) => last = Some(anyhow::Error::new(e).context(format!("connecting to {addr}"))),
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("no address for {server}")))
}

/// `host`, `host:port`, `[v6]:port`.
fn resolve(server: &str) -> Result<Vec<SocketAddr>> {
    let server = server.trim();
    let with_port = if server.parse::<SocketAddr>().is_ok() || server.rsplit_once(':').is_some_and(|(h, p)| !h.contains(':') && p.parse::<u16>().is_ok()) {
        server.to_string()
    } else if server.contains(':') && !server.starts_with('[') {
        format!("[{server}]:{PORT}")
    } else {
        format!("{server}:{PORT}")
    };
    Ok(with_port.to_socket_addrs().with_context(|| format!("looking up {server}"))?.collect())
}

/// The boot set (whale5's `bootset.json`: `[[path, [[first, last], …]], …]`
/// in byte offsets) as block runs, close ranges merged.
fn boot_runs(catalog: &Catalog, json: &[u8]) -> Vec<(u32, u64, u64)> {
    let Ok(serde_json::Value::Array(files)) = serde_json::from_slice(json) else { return Vec::new() };
    let mut runs = Vec::new();
    for f in files {
        let (Some(path), Some(ranges)) = (f[0].as_str(), f[1].as_array()) else { continue };
        let Some(id) = catalog.find(&format!("data/{path}")) else { continue };
        let blocks = catalog.entries[id as usize].size.div_ceil(BLOCK);
        // An empty file has no blocks to fetch.
        if blocks == 0 {
            continue;
        }
        let mut cur: Option<(u64, u64)> = None;
        for r in ranges {
            let (Some(a), Some(b)) = (r[0].as_u64(), r[1].as_u64()) else { continue };
            let (a, b) = (a / BLOCK, (b / BLOCK).min(blocks - 1));
            if a > b {
                continue;
            }
            match &mut cur {
                Some(c) if a <= c.1 + BOOT_GAP => c.1 = c.1.max(b),
                _ => {
                    runs.extend(cur.take().map(|(x, y)| (id, x, y)));
                    cur = Some((a, b));
                }
            }
        }
        runs.extend(cur.map(|(x, y)| (id, x, y)));
    }
    runs
}

/// Messages as they go on the wire.
fn encode(msgs: &[Header]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(msgs.len() * HEADER);
    for m in msgs {
        buf.extend_from_slice(&m.encode());
    }
    buf
}

/// A blob of zstd-packed JSON, unpacked (at most [`MAX_JSON`]).
fn unpack_json(packed: &[u8], what: &str) -> Result<Vec<u8>> {
    let mut json = Vec::new();
    zstd::stream::Decoder::new(packed)?.take(MAX_JSON + 1).read_to_end(&mut json).with_context(|| format!("unpacking {what}"))?;
    if json.len() as u64 > MAX_JSON {
        bail!("{what} unpacks to more than {MAX_JSON} bytes");
    }
    Ok(json)
}

fn read_json(path: &Path) -> serde_json::Value {
    fs::read(path).ok().and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_else(|| serde_json::json!({}))
}

fn is_full(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::StorageFull || e.raw_os_error() == Some(28)
}

/// The next message's header, or `None` if nothing came within the
/// socket's read timeout (a header is never left half read).
fn next_header(r: &mut impl Read) -> io::Result<Option<Header>> {
    let mut b = [0u8; HEADER];
    match r.read(&mut b[..1]) {
        Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
        Ok(_) => {}
        Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted) => {
            return Ok(None);
        }
        Err(e) => return Err(e),
    }
    read_exact_retrying(r, &mut b[1..], SILENCE)?;
    Ok(Some(Header::decode(&b)))
}

/// `read_exact` through the reader's timeouts (a chunk may arrive slowly):
/// fails after `silence` without a byte.
fn read_exact_retrying(r: &mut impl Read, mut buf: &mut [u8], silence: Duration) -> io::Result<()> {
    let mut last = Instant::now();
    while !buf.is_empty() {
        match r.read(buf) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => {
                buf = &mut buf[n..];
                last = Instant::now();
            }
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted) => {
                if last.elapsed() > silence {
                    return Err(e);
                }
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// A chunk that comes slowly, but comes, is taken whole (a link of a few
/// KB/s takes longer than the silence for 64 KB); one that stops is not.
#[test]
fn takes_a_slow_chunk() {
    // A byte every 30 ms, timeouts in between.
    struct Slow(Instant);
    impl Read for Slow {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            thread::sleep(Duration::from_millis(10));
            if self.0.elapsed() < Duration::from_millis(30) {
                return Err(io::ErrorKind::TimedOut.into());
            }
            self.0 = Instant::now();
            buf[0] = 1;
            Ok(1)
        }
    }
    let mut buf = [0u8; 10];
    read_exact_retrying(&mut Slow(Instant::now()), &mut buf, Duration::from_millis(100)).unwrap();
    assert_eq!(buf, [1; 10]);
    let mut slower = Slow(Instant::now() + Duration::from_millis(200));
    assert!(read_exact_retrying(&mut slower, &mut buf, Duration::from_millis(100)).is_err());
}

/// The boot set's ranges as runs of blocks, an empty file's left out (it
/// has none: its bitmap is empty).
#[test]
fn boot_runs_of_files_with_blocks() {
    use crate::catalog::Entry;
    let entries = vec![
        Entry { path: "data/a.rpf".into(), size: 10 * BLOCK, mtime: 1 },
        Entry { path: "data/empty.rpf".into(), size: 0, mtime: 1 },
    ];
    let catalog = Catalog::new("x".into(), entries, 2);
    let json = br#"[["a.rpf", [[0, 100], [8192, 9000], [1000000, 2000000]]], ["empty.rpf", [[0, 10]]]]"#;
    assert_eq!(boot_runs(&catalog, json), vec![(0, 0, 2)]);
}

#[test]
fn connects_to_the_address_that_answers() {
    // "localhost" stands for ::1 too, where nothing listens.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (_, addr) = connect(&format!("localhost:{port}")).unwrap();
    assert_eq!(addr, listener.local_addr().unwrap());
}
