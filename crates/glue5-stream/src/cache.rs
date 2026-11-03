//! The client's copy of the game: the server's files at the same paths
//! (`data/…`, `b/<build>/shaders/…`), sparse, with a bitmap per file of the
//! 4 KB blocks it holds.
//!
//! Blocks are written as they arrive and marked present right after; the
//! index of present blocks (`index`) is saved every few seconds, after the
//! data it describes is on disk, so a crash or a power cut loses at most
//! what came in since. Files are emptied only once an index without their
//! blocks is saved (on a volume too full for one, the index is deleted
//! until the next save: a crash meanwhile loses it all). The system may
//! delete any of it while the app is not running (Apple TV and iPhone
//! caches, a user): at every start a block only counts if its file is
//! still there with its full length, and the rest is fetched again when
//! needed.
//!
//! Reads of present blocks are plain positioned reads from these files, as
//! fast as a local game folder. When space runs out, files unused for the
//! longest time are emptied (a per-file lock keeps readers away meanwhile).

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};

use anyhow::Result;

use crate::catalog::Catalog;
use crate::sys;

pub const BLOCK: u64 = 4096;
const MAGIC: &[u8; 4] = b"G5SC";
/// Missing blocks are given back to the file system in runs of at least this.
#[cfg(target_vendor = "apple")]
const MIN_HOLE: u64 = 64 << 10;
const VERSION: u32 = 1;
/// In a partial copy: it is not a game folder for the local mode.
pub const MARKER: &str = ".glue5-stream";
/// In every copy (partial or complete): the server's list of files.
pub const CATALOG: &str = "catalog.json";
const PACKAGE_PINS: &str = "package-pins.json";

pub struct Cache {
    root: PathBuf,
    build: String,
    catalog_hash: String,
    total: u64,
    /// Files kept open, and how many may be: the rest are opened for each
    /// use (the open-file limit is 10240 on Apple systems, often 1024 on
    /// Linux, and the packs alone write 10,000 files).
    kept: AtomicUsize,
    keep_at_most: usize,
    slots: Vec<Slot>,
    session: u32,
    /// When the copy was opened (what `touched` counts from).
    opened: std::time::Instant,
    used: AtomicU64,
    /// Files written to since the last save (their data is synced first).
    dirty: Mutex<Vec<u32>>,
    changed: AtomicBool,
    saving: Mutex<()>,
    /// Held while the copy is open: another process on it (two instances
    /// on a desktop) would evict, punch and save over this one's index.
    _lock: Arc<File>,
}

/// The copy in `root` taken by this process, or an error if another
/// process has it. Shared by this process's copies of it (a stream started
/// again opens it while the stopped one still exists).
fn lock(root: &Path) -> io::Result<Arc<File>> {
    static LOCKS: Mutex<Option<HashMap<PathBuf, Weak<File>>>> = Mutex::new(None);
    let key = root.canonicalize()?;
    let mut locks = LOCKS.lock().unwrap();
    let locks = locks.get_or_insert_default();
    if let Some(f) = locks.get(&key).and_then(Weak::upgrade) {
        return Ok(f);
    }
    let f = OpenOptions::new().create(true).write(true).truncate(false).open(root.join(LOCK))?;
    match f.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            return Err(io::Error::other("the copy is in use by another Glue 5 (only one can use it at a time)"));
        }
        // A file system without locks (some network shares): unlocked.
        Err(std::fs::TryLockError::Error(e)) if e.kind() == io::ErrorKind::Unsupported || e.raw_os_error() == Some(libc_enolck()) => {
            log::warn!("stream cache: {} cannot be locked ({e}): other instances are not kept off it", root.display());
        }
        Err(std::fs::TryLockError::Error(e)) => return Err(e),
    }
    let f = Arc::new(f);
    locks.retain(|_, w| w.strong_count() > 0);
    locks.insert(key, Arc::downgrade(&f));
    Ok(f)
}

/// ENOLCK, where there is one (no locks on the file system).
fn libc_enolck() -> i32 {
    #[cfg(unix)]
    return libc::ENOLCK;
    #[cfg(not(unix))]
    return -1;
}

/// The lock file's name in a copy.
const LOCK: &str = ".glue5-lock";

struct Slot {
    path: String,
    size: u64,
    mtime: u64,
    present: Box<[AtomicU64]>,
    file: OnceLock<File>,
    /// Shared by reads and writes, exclusive while the file is emptied.
    guard: RwLock<()>,
    /// Held by a write and by the punching of a hole (not by reads).
    writing: Mutex<()>,
    last_used: AtomicU32,
    /// When this run last read or wrote the file: milliseconds since the
    /// copy was opened (0: not yet).
    touched: AtomicU32,
    /// One of a pack's files: every start needs it (without the server too).
    pack: bool,
    /// Holds a part of the boot set: every start reads it.
    boot: AtomicBool,
    /// A verified optional package member, protected from normal cache eviction.
    pinned: AtomicBool,
    dirty: AtomicBool,
}

fn load_pins(root: &Path, catalog: &Catalog) -> HashSet<String> {
    let Ok(bytes) = fs::read(root.join(PACKAGE_PINS)) else { return HashSet::new() };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else { return HashSet::new() };
    if value["version"].as_u64() != Some(1)
        || value["build"].as_str() != Some(catalog.build.as_str())
        || value["catalog_hash"].as_str() != Some(catalog.hash.as_str())
    {
        return HashSet::new();
    }
    value["files"].as_array().into_iter().flatten().filter_map(|v| {
        let path = v.as_str()?;
        if !crate::catalog::safe_path(path) || !path.starts_with("data/") {
            return None;
        }
        let id = catalog.find(path)? as usize;
        let entry = catalog.entries.get(id)?;
        (entry.path == path).then(|| path.to_owned())
    }).collect()
}

impl Slot {
    fn blocks(&self) -> u64 {
        self.size.div_ceil(BLOCK)
    }

    fn bytes(&self) -> u64 {
        let n: u64 = self.present.iter().map(|w| w.load(Ordering::Relaxed).count_ones() as u64).sum();
        (n * BLOCK).min(self.size)
    }
}

impl Cache {
    /// The copy in `root` of the files of `catalog`.
    pub fn open(root: &Path, catalog: &Catalog) -> Result<Cache> {
        fs::create_dir_all(root)?;
        let lock = lock(root)?;
        sys::raise_open_file_limit();
        let index = load_index(&root.join("index"));
        // A partial copy from before (its marker is written below): what it
        // holds that its index does not vouch for is counted nowhere, so
        // eviction could never take it back.
        let (partial, lost) = (root.join(MARKER).exists(), index.is_none());
        let old = index.unwrap_or_default();
        let pinned = load_pins(root, catalog);
        let session = old.session.wrapping_add(1);
        let (mut kept, mut used) = (0usize, 0u64);
        // Blocks the index on disk claims that this copy does not (see below).
        let mut dropped = false;
        let mut slots = Vec::with_capacity(catalog.entries.len());
        for e in &catalog.entries {
            let words = e.size.div_ceil(BLOCK).div_ceil(64) as usize;
            let mut present: Box<[AtomicU64]> = (0..words).map(|_| AtomicU64::new(0)).collect();
            let mut last_used = 0;
            if let Some(o) = old.files.get(&e.path)
                && o.size == e.size
                && o.mtime == e.mtime
                && o.bits.len() == words
                && fs::metadata(root.join(&e.path)).is_ok_and(|m| m.len() == e.size)
            {
                present = o.bits.iter().map(|&w| AtomicU64::new(w)).collect();
                last_used = o.last_used;
                kept += 1;
            } else {
                dropped |= old.files.get(&e.path).is_some_and(|o| o.bits.iter().any(|&w| w != 0));
                if partial && (lost || old.files.contains_key(&e.path)) && crate::catalog::safe_path(&e.path) {
                    let _ = fs::remove_file(root.join(&e.path));
                }
            }
            let slot = Slot {
                path: e.path.clone(),
                size: e.size,
                mtime: e.mtime,
                present,
                file: OnceLock::new(),
                guard: RwLock::new(()),
                writing: Mutex::new(()),
                last_used: AtomicU32::new(last_used),
                touched: AtomicU32::new(0),
                pack: crate::pack::kind_of(catalog, slots.len()).is_some(),
                boot: AtomicBool::new(false),
                pinned: AtomicBool::new(pinned.contains(e.path.as_str())),
                dirty: AtomicBool::new(false),
            };
            used += slot.bytes();
            slots.push(slot);
        }
        // Files of an older catalog.
        for path in old.files.keys() {
            if catalog.find(path).is_none() && crate::catalog::safe_path(path) {
                let _ = fs::remove_file(root.join(path));
            }
        }
        log::info!(
            "stream cache: {} ({:.2} GB in {kept} of {} files, session {session})",
            root.display(),
            used as f64 / 1e9,
            slots.len()
        );
        let total = slots.iter().map(|s| s.size).sum();
        let cache = Cache {
            root: root.to_owned(),
            build: catalog.build.clone(),
            catalog_hash: catalog.hash.clone(),
            total,
            kept: AtomicUsize::new(0),
            keep_at_most: sys::open_file_budget(),
            slots,
            session,
            opened: std::time::Instant::now(),
            used: AtomicU64::new(used),
            dirty: Mutex::new(Vec::new()),
            changed: AtomicBool::new(true),
            saving: Mutex::new(()),
            _lock: lock,
        };
        let saved = if cache.all_here() {
            cache.finish(catalog)
        } else {
            fs::write(root.join(MARKER), b"A partial copy of a game folder, streamed by Glue 5.\n")?;
            cache.save()
        };
        // Not saved (a full volume): the copy is still usable, the next save
        // tries again. The index on disk must not claim blocks this copy
        // dropped meanwhile (a file the system deleted, written again later,
        // would count as here, its holes as data): then it goes until that
        // save, as when files are emptied.
        if let Err(e) = saved {
            log::warn!("stream cache: {}: the index not saved ({e}): again later", root.display());
            if dropped
                && let Err(e) = fs::remove_file(root.join("index"))
                && e.kind() != io::ErrorKind::NotFound
            {
                return Err(e.into());
            }
        }
        Ok(cache)
    }

    /// Every block of every file is here.
    pub fn all_here(&self) -> bool {
        (0..self.slots.len() as u32).all(|id| self.complete(id))
    }

    /// The copy is complete: a game folder like any other (for the local
    /// mode too), saved for good. With the engine's manifest of `catalog`
    /// (the copy's) as the web build's server keeps it: played or shared
    /// as a folder, the copy gives the engine the dates streaming did (its
    /// files' own are when they came, see [`crate::catalog::game_dates`]).
    pub fn finish(&self, catalog: &Catalog) -> io::Result<()> {
        // Empty files have no blocks to bring them.
        for (id, s) in self.slots.iter().enumerate() {
            if s.file.get().is_none() && !fs::metadata(self.root.join(&s.path)).is_ok_and(|m| m.len() == s.size) {
                self.open_file(id as u32)?;
            }
        }
        self.save()?;
        let manifest = catalog.engine_manifest();
        let path = self.root.join("data/manifest.json");
        if fs::read(&path).ok().as_deref() != Some(&manifest[..]) {
            // (A name the catalog's listing skips, should it stay.)
            let tmp = self.root.join("data/.manifest.json.tmp");
            fs::create_dir_all(self.root.join("data"))?;
            fs::write(&tmp, &manifest)?;
            fs::rename(&tmp, &path)?;
        }
        match fs::remove_file(self.root.join(MARKER)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// The bytes of a complete copy.
    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn size(&self, id: u32) -> u64 {
        self.slots[id as usize].size
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Bytes held.
    pub fn used(&self) -> u64 {
        self.used.load(Ordering::Relaxed)
    }

    /// The blocks `first..=last` of file `id` are all here.
    pub fn has(&self, id: u32, first: u64, last: u64) -> bool {
        all_set(&self.slots[id as usize].present, first, last)
    }

    /// How many of the blocks `first..=last` of file `id` are here.
    pub fn count(&self, id: u32, first: u64, last: u64) -> u64 {
        count_set(&self.slots[id as usize].present, first, last)
    }

    /// The first block of `first..=last` of file `id` not here (`last + 1`:
    /// none), a word of the bitmap at a time.
    pub fn first_missing(&self, id: u32, first: u64, last: u64) -> u64 {
        let bits = &self.slots[id as usize].present;
        let mut b = first;
        while b <= last {
            let Some(word) = bits.get((b / 64) as usize) else { break };
            let ones = (word.load(Ordering::Acquire) >> (b % 64)).trailing_ones() as u64;
            if ones < 64 - b % 64 {
                return (b + ones).min(last + 1);
            }
            b = (b / 64 + 1) * 64;
        }
        last + 1
    }

    pub fn has_block(&self, id: u32, block: u64) -> bool {
        let w = self.slots[id as usize].present[(block / 64) as usize].load(Ordering::Acquire);
        w & (1 << (block % 64)) != 0
    }

    /// The whole file is here.
    pub fn complete(&self, id: u32) -> bool {
        let s = &self.slots[id as usize];
        s.size == 0 || all_set(&s.present, 0, s.blocks() - 1)
    }

    pub fn blocks(&self, id: u32) -> u64 {
        self.slots[id as usize].blocks()
    }

    /// Reads `buf.len()` bytes at `off` (fewer at the end of the file) if
    /// they are all here.
    pub fn read(&self, id: u32, off: u64, buf: &mut [u8]) -> io::Result<Option<usize>> {
        let s = &self.slots[id as usize];
        if off >= s.size || buf.is_empty() {
            return Ok(Some(0));
        }
        let n = buf.len().min((s.size - off) as usize);
        let _g = s.guard.read().unwrap();
        if !all_set(&s.present, off / BLOCK, (off + n as u64 - 1) / BLOCK) {
            return Ok(None);
        }
        let got = sys::read_full_at(&*self.handle(id)?, &mut buf[..n], off)?;
        self.touch(s);
        Ok(Some(got))
    }

    /// Stores `data` at `off` (a block boundary) of file `id` and marks the
    /// blocks it fills (the last one may end with the file).
    pub fn write(&self, id: u32, off: u64, data: &[u8]) -> io::Result<()> {
        let s = &self.slots[id as usize];
        if !off.is_multiple_of(BLOCK) || off.checked_add(data.len() as u64).is_none_or(|end| end > s.size) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "write outside the file's blocks"));
        }
        if data.is_empty() {
            // An empty file has no blocks: it is here once it exists.
            if s.size == 0 {
                self.open_file(id)?;
            }
            return Ok(());
        }
        // Not while the file is emptied or a hole punched: its blocks would
        // be marked over zeros.
        let _g = s.guard.read().unwrap();
        let _w = s.writing.lock().unwrap();
        sys::write_all_at(&*self.handle(id)?, data, off)?;
        let end = off + data.len() as u64;
        if end >= off + BLOCK || end == s.size {
            let last = if end == s.size { (end - 1) / BLOCK } else { end / BLOCK - 1 };
            let added = self.mark(id, off / BLOCK, last);
            self.used.fetch_add(added * BLOCK, Ordering::Relaxed);
        }
        self.touch(s);
        Ok(())
    }

    /// Marks blocks `first..=last` of file `id` present (its data written)
    /// and the file written to, both at once for [`Cache::save`]: a block
    /// it finds present has its file on the list it takes after, and synced
    /// before the index that says so. Returns how many were not present.
    fn mark(&self, id: u32, first: u64, last: u64) -> u64 {
        let s = &self.slots[id as usize];
        let mut dirty = self.dirty.lock().unwrap();
        let added = set_range(&s.present, first, last);
        if !s.dirty.swap(true, Ordering::AcqRel) {
            dirty.push(id);
        }
        self.changed.store(true, Ordering::Release);
        added
    }

    /// Invalidates all cached blocks of file `id`, for a package file whose
    /// content SHA-256 did not match its manifest.
    pub fn invalidate(&self, id: u32) -> io::Result<()> {
        let s = self.slots.get(id as usize).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "unknown file id"))?;
        let _g = s.guard.write().unwrap();
        let _w = s.writing.lock().unwrap();
        let freed = s.bytes();
        for word in &s.present {
            word.store(0, Ordering::Release);
        }
        self.used.fetch_sub(freed.min(self.used()), Ordering::Relaxed);
        {
            let mut dirty = self.dirty.lock().unwrap();
            if !s.dirty.swap(true, Ordering::AcqRel) {
                dirty.push(id);
            }
        }
        self.changed.store(true, Ordering::Release);
        // Persist the empty bitmap before truncating: a crash cannot make
        // zero-filled bytes count as previously verified game data.
        self.save()?;
        if let Some(file) = s.file.get() {
            file.set_len(0)?;
            file.set_len(s.size)?;
        } else if self.path(id).is_file() {
            let file = OpenOptions::new().write(true).open(self.path(id))?;
            file.set_len(0)?;
            file.set_len(s.size)?;
        }
        Ok(())
    }

    /// Stores the whole file `id` (from a pack), without keeping it open.
    pub fn write_whole(&self, id: u32, data: &[u8]) -> io::Result<()> {
        let s = &self.slots[id as usize];
        if data.len() as u64 != s.size {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not the file's size"));
        }
        let _g = s.guard.read().unwrap();
        let _w = s.writing.lock().unwrap();
        let f = match s.file.get() {
            Some(f) => Handle::Kept(f),
            None => Handle::Once(self.open_file(id)?),
        };
        sys::write_all_at(&f, data, 0)?;
        if s.size > 0 {
            let added = self.mark(id, 0, s.blocks() - 1);
            self.used.fetch_add((added * BLOCK).min(s.size), Ordering::Relaxed);
        }
        self.touch(s);
        Ok(())
    }

    /// Where file `id` is kept.
    pub fn path(&self, id: u32) -> PathBuf {
        self.root.join(&self.slots[id as usize].path)
    }

    /// File `id`, kept open if there is room for one more.
    fn handle(&self, id: u32) -> io::Result<Handle<'_>> {
        let s = &self.slots[id as usize];
        if let Some(f) = s.file.get() {
            return Ok(Handle::Kept(f));
        }
        let f = self.open_file(id)?;
        // A pack's member is read whole, once: no room kept for it (the
        // budget is for the archives read thousands of times).
        if s.pack {
            return Ok(Handle::Once(f));
        }
        if self.kept.fetch_add(1, Ordering::Relaxed) < self.keep_at_most {
            Ok(Handle::Kept(s.file.get_or_init(|| f)))
        } else {
            self.kept.fetch_sub(1, Ordering::Relaxed);
            Ok(Handle::Once(f))
        }
    }

    /// Opens (creates, sparse, at its full length) file `id`.
    fn open_file(&self, id: u32) -> io::Result<File> {
        let s = &self.slots[id as usize];
        let path = self.root.join(&s.path);
        let open = || OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&path);
        // Its folder made only when missing (two system calls less for each
        // of the thousands of files opened).
        let f = match open() {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if let Some(dir) = path.parent() {
                    fs::create_dir_all(dir)?;
                }
                open()?
            }
            f => f?,
        };
        if f.metadata()?.len() != s.size {
            sys::make_sparse(&f);
            f.set_len(s.size)?;
        }
        Ok(f)
    }

    /// Saves the index if anything changed (the data first).
    pub fn save(&self) -> io::Result<()> {
        let _one = self.saving.lock().unwrap();
        if !self.changed.swap(false, Ordering::AcqRel) {
            return Ok(());
        }
        // The bits, then the files written to (see `mark`): what a write
        // marks after the bits were read waits for the next save.
        let index = self.index();
        let dirty = {
            let mut list = self.dirty.lock().unwrap();
            for &id in list.iter() {
                self.slots[id as usize].dirty.store(false, Ordering::Release);
            }
            std::mem::take(&mut *list)
        };
        let result = self.write_index(&dirty, &index);
        if result.is_err() {
            // Synced or not, they are synced again before the next index.
            let mut list = self.dirty.lock().unwrap();
            for &id in &dirty {
                if !self.slots[id as usize].dirty.swap(true, Ordering::AcqRel) {
                    list.push(id);
                }
            }
            self.changed.store(true, Ordering::Release);
        }
        result
    }

    /// Syncs the files written to, then writes `index` (their data first).
    fn write_index(&self, dirty: &[u32], index: &[u8]) -> io::Result<()> {
        // A file not kept open is opened again just to be synced (a sync
        // flushes the file, whichever handle asks), for writing: Windows
        // flushes no file opened only to be read (access denied). One gone
        // meanwhile (emptied) has nothing to sync.
        let sync = |id: &u32| match self.slots[*id as usize].file.get() {
            Some(f) => sys::sync_data(f),
            None => match OpenOptions::new().read(true).write(true).open(self.path(*id)) {
                Ok(f) => sys::sync_data(&f),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e),
            },
        };
        // A pack writes thousands of files: synced side by side, each only to
        // the drive (one full flush of the drive's cache, the index's, below
        // covers them all: 14 s -> well under one for the text pack on a Mac).
        let workers = dirty.len().div_ceil(256).clamp(1, 8);
        std::thread::scope(|scope| {
            let jobs: Vec<_> = dirty
                .chunks(dirty.len().div_ceil(workers).max(1))
                .map(|chunk| scope.spawn(move || chunk.iter().try_for_each(sync)))
                .collect();
            jobs.into_iter().try_for_each(|j| j.join().unwrap_or(Ok(())))
        })?;
        #[cfg(target_vendor = "apple")]
        for &id in dirty {
            // Not one being emptied (evict holds it from clearing its bits
            // to truncating it): its bits went after `index` was taken,
            // which still claims them, and its holes would be read as data
            // if this index were the last one written.
            let Ok(_g) = self.slots[id as usize].guard.try_read() else { continue };
            if let Err(e) = self.keep_sparse(id) {
                log::debug!("stream cache: {}: {e}", self.slots[id as usize].path);
            }
        }
        let tmp = self.root.join("index.tmp");
        let mut f = File::create(&tmp)?;
        f.write_all(index)?;
        // The data before the index that says it is there.
        sys::sync_full(&f)?;
        fs::rename(&tmp, self.root.join("index"))
    }

    /// The index as saved: every file's bitmap.
    fn index(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + self.slots.len() * 64);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&self.session.to_le_bytes());
        out.extend_from_slice(&(self.slots.len() as u32).to_le_bytes());
        for s in &self.slots {
            out.extend_from_slice(&(s.path.len() as u16).to_le_bytes());
            out.extend_from_slice(s.path.as_bytes());
            out.extend_from_slice(&s.size.to_le_bytes());
            out.extend_from_slice(&s.mtime.to_le_bytes());
            out.extend_from_slice(&s.last_used.load(Ordering::Relaxed).to_le_bytes());
            out.extend_from_slice(&(s.present.len() as u32).to_le_bytes());
            for w in s.present.iter() {
                out.extend_from_slice(&w.load(Ordering::Acquire).to_le_bytes());
            }
        }
        out
    }

    /// Frees what the file system allocated for blocks that are not here.
    /// APFS, when it writes a file out, may fill the holes between its
    /// written parts with zeros (whether it does depends on its state: once
    /// 4.2 GB were taken for 3.7 GB of blocks); a just-synced file gives them
    /// back here (other file systems never fill holes). Each hole is punched
    /// with the file's writes held off (a block written meanwhile would be
    /// marked over zeros), its reads not (they only read blocks that are here).
    #[cfg(target_vendor = "apple")]
    fn keep_sparse(&self, id: u32) -> io::Result<()> {
        let s = &self.slots[id as usize];
        if self.complete(id) {
            return Ok(());
        }
        let f = match s.file.get() {
            Some(f) => Handle::Kept(f),
            None => Handle::Once(OpenOptions::new().read(true).write(true).open(self.root.join(&s.path))?),
        };
        let blocks = s.blocks();
        let set = |b: u64| s.present[(b / 64) as usize].load(Ordering::Acquire) & (1 << (b % 64)) != 0;
        for (start, end) in sys::data_extents(&f)? {
            let (mut b, last) = (start.div_ceil(BLOCK), (end / BLOCK).min(blocks));
            while b < last {
                if set(b) {
                    b += 1;
                    continue;
                }
                let hole = b;
                while b < last && !set(b) {
                    b += 1;
                }
                let (off, len) = (hole * BLOCK, ((b * BLOCK).min(s.size)) - hole * BLOCK);
                if len < MIN_HOLE {
                    continue;
                }
                let _writes = s.writing.lock().unwrap();
                // Written meanwhile: next time.
                if (hole..b).all(|k| !set(k)) {
                    sys::punch_hole(&f, off, len)?;
                }
            }
        }
        Ok(())
    }

    /// A file was read or written: used by this run, now.
    fn touch(&self, s: &Slot) {
        s.last_used.store(self.session, Ordering::Relaxed);
        let ms = self.opened.elapsed().as_millis().min(u32::MAX as u128 - 1) as u32;
        s.touched.store(ms + 1, Ordering::Relaxed);
    }

    /// File `id` holds a part of the boot set: it is emptied after the
    /// others (see [`Cache::evict`]).
    pub fn keep_longer(&self, id: u32) {
        self.slots[id as usize].boot.store(true, Ordering::Relaxed);
    }

    /// Persist the set of verified package files which normal cache pressure
    /// must not evict. Pins are bound to this exact build and catalog hash.
    pub fn set_pinned(&self, ids: &[u32]) -> io::Result<()> {
        let mut selected = vec![false; self.slots.len()];
        for &id in ids {
            let Some(value) = selected.get_mut(id as usize) else {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "package pin has an unknown catalog id"));
            };
            *value = true;
        }
        let files: Vec<_> = self.slots.iter().zip(&selected).filter_map(|(slot, pin)| pin.then_some(slot.path.as_str())).collect();
        let bytes = serde_json::to_vec(&serde_json::json!({
            "version": 1, "build": self.build, "catalog_hash": self.catalog_hash, "files": files,
        })).map_err(io::Error::other)?;
        let tmp = self.root.join(format!("{PACKAGE_PINS}.tmp"));
        let path = self.root.join(PACKAGE_PINS);
        fs::write(&tmp, &bytes)?;
        File::open(&tmp)?.sync_all()?;
        fs::rename(&tmp, &path)?;
        for (slot, pin) in self.slots.iter().zip(selected) {
            slot.pinned.store(pin, Ordering::Release);
        }
        Ok(())
    }

    pub fn is_pinned(&self, id: u32) -> bool {
        self.slots.get(id as usize).is_some_and(|slot| slot.pinned.load(Ordering::Acquire))
    }

    pub fn pinned_ids(&self) -> Vec<u32> {
        self.slots.iter().enumerate().filter_map(|(id, slot)| slot.pinned.load(Ordering::Acquire).then_some(id as u32)).collect()
    }

    /// Empties the files used longest ago until `want` bytes are freed:
    /// those of earlier runs first, then this run's, least recently used
    /// first; the boot set's files after them (every start reads them, and
    /// they are read first: least recently used, they went first), the
    /// packs' files last (every start needs them, and a start without the
    /// server cannot have them again). Returns the bytes freed.
    pub fn evict(&self, want: u64) -> u64 {
        let mut order: Vec<u32> = (0..self.slots.len() as u32)
            .filter(|&i| {
                let slot = &self.slots[i as usize];
                slot.bytes() > 0 && !slot.pinned.load(Ordering::Acquire)
            })
            .collect();
        order.sort_by_key(|&i| {
            let s = &self.slots[i as usize];
            let tier = if s.pack { 2 } else { s.boot.load(Ordering::Relaxed) as u8 };
            (tier, s.last_used.load(Ordering::Relaxed), s.touched.load(Ordering::Relaxed))
        });
        // Their blocks go first, readers and writers kept away until the
        // files are emptied.
        let (mut freed, mut emptied) = (0, Vec::new());
        for id in order {
            if freed >= want {
                break;
            }
            let s = &self.slots[id as usize];
            let g = s.guard.write().unwrap();
            let bytes = s.bytes();
            for w in s.present.iter() {
                w.store(0, Ordering::Release);
            }
            self.used.fetch_sub(bytes.min(self.used()), Ordering::Relaxed);
            freed += bytes;
            emptied.push((s, g));
        }
        if emptied.is_empty() {
            return 0;
        }
        // Then the index on disk, which must not claim them once their data
        // is gone (after a crash, a full-length file of zeros would count):
        // saved, or, if it cannot be (a full volume), deleted until the next
        // save writes it again.
        self.changed.store(true, Ordering::Release);
        if let Err(e) = self.save() {
            log::warn!("stream cache: saving the index before emptying files: {e}");
            if let Err(e) = fs::remove_file(self.root.join("index"))
                && e.kind() != io::ErrorKind::NotFound
            {
                // Kept: the index still claims it (fetched again when needed).
                log::warn!("stream cache: nothing emptied: {e}");
                return 0;
            }
        }
        for (s, _g) in &emptied {
            if let Some(f) = s.file.get() {
                let _ = f.set_len(0).and_then(|()| f.set_len(s.size));
            } else {
                let _ = fs::remove_file(self.root.join(&s.path));
            }
        }
        log::info!("stream cache: {:.2} GB freed", freed as f64 / 1e9);
        freed
    }

    /// Bytes free on the cache's volume.
    pub fn free_space(&self) -> Option<u64> {
        sys::free_space(&self.root)
    }
}

/// A file of the copy: kept open, or open for this use only.
enum Handle<'a> {
    Kept(&'a File),
    Once(File),
}

impl std::ops::Deref for Handle<'_> {
    type Target = File;
    fn deref(&self) -> &File {
        match self {
            Handle::Kept(f) => f,
            Handle::Once(f) => f,
        }
    }
}

/// Deletes the copy in `root`.
pub fn clear(root: &Path) -> io::Result<()> {
    // Not a copy another process uses.
    if root.is_dir() {
        let _taken = lock(root)?;
    }
    match fs::remove_dir_all(root) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[derive(Default)]
struct Index {
    session: u32,
    files: HashMap<String, Saved>,
}

struct Saved {
    size: u64,
    mtime: u64,
    last_used: u32,
    bits: Vec<u64>,
}

fn load_index(path: &Path) -> Option<Index> {
    let mut data = Vec::new();
    File::open(path).ok()?.read_to_end(&mut data).ok()?;
    let mut r = Bytes(&data);
    if r.take(4)? != MAGIC || r.u32()? != VERSION {
        return None;
    }
    let session = r.u32()?;
    let count = r.u32()?;
    // Not more than the data can hold (26 bytes a file at least): a damaged
    // count would ask for gigabytes.
    let mut files = HashMap::with_capacity((count as usize).min(data.len() / 26));
    for _ in 0..count {
        let len = u16::from_le_bytes(r.take(2)?.try_into().ok()?) as usize;
        let path = String::from_utf8(r.take(len)?.to_vec()).ok()?;
        let (size, mtime, last_used, words) = (r.u64()?, r.u64()?, r.u32()?, r.u32()?);
        let bits = (0..words).map(|_| r.u64()).collect::<Option<Vec<u64>>>()?;
        files.insert(path, Saved { size, mtime, last_used, bits });
    }
    Some(Index { session, files })
}

struct Bytes<'a>(&'a [u8]);

impl<'a> Bytes<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let (a, b) = (self.0.get(..n)?, self.0.get(n..)?);
        self.0 = b;
        Some(a)
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
}

/// The bits `lo..=hi` of a word.
fn mask(lo: u64, hi: u64) -> u64 {
    let width = hi - lo + 1;
    (if width == 64 { u64::MAX } else { (1u64 << width) - 1 }) << lo
}

fn all_set(bits: &[AtomicU64], first: u64, last: u64) -> bool {
    let mut b = first;
    while b <= last {
        let word = b / 64;
        let m = mask(b % 64, (last - word * 64).min(63));
        if bits[word as usize].load(Ordering::Acquire) & m != m {
            return false;
        }
        b = (word + 1) * 64;
    }
    true
}

/// How many of the bits `first..=last` are set.
fn count_set(bits: &[AtomicU64], first: u64, last: u64) -> u64 {
    let (mut b, mut n) = (first, 0);
    while b <= last {
        let word = b / 64;
        let m = mask(b % 64, (last - word * 64).min(63));
        n += (bits[word as usize].load(Ordering::Acquire) & m).count_ones() as u64;
        b = (word + 1) * 64;
    }
    n
}

/// Sets the bits `first..=last`; returns how many were not set.
fn set_range(bits: &[AtomicU64], first: u64, last: u64) -> u64 {
    let (mut b, mut added) = (first, 0);
    while b <= last {
        let word = b / 64;
        let m = mask(b % 64, (last - word * 64).min(63));
        let old = bits[word as usize].fetch_or(m, Ordering::AcqRel);
        added += (m & !old).count_ones() as u64;
        b = (word + 1) * 64;
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Entry;

    #[test]
    fn bits() {
        let bits: Vec<AtomicU64> = (0..3).map(|_| AtomicU64::new(0)).collect();
        assert_eq!(set_range(&bits, 3, 130), 128);
        assert!(all_set(&bits, 3, 130) && !all_set(&bits, 2, 130) && !all_set(&bits, 3, 131));
        assert_eq!((count_set(&bits, 0, 191), count_set(&bits, 60, 70), count_set(&bits, 130, 131)), (128, 11, 1));
        assert_eq!(set_range(&bits, 0, 191), 64);
        assert!(all_set(&bits, 0, 191));
    }

    #[test]
    fn keeps_what_is_still_there() {
        let root = std::env::temp_dir().join(format!("glue5-cache-test-{}", std::process::id()));
        let _ = clear(&root);
        let entries = vec![
            Entry { path: "data/a.rpf".into(), size: 3 * BLOCK + 10, mtime: 1 },
            Entry { path: "data/b.xml".into(), size: 5, mtime: 1 },
        ];
        let catalog = Catalog::new("x".into(), entries.clone(), 2);
        {
            let c = Cache::open(&root, &catalog).unwrap();
            let block = vec![7u8; BLOCK as usize];
            c.write(0, 0, &block).unwrap();
            c.write(0, 3 * BLOCK, &[9u8; 10]).unwrap();
            c.write(1, 0, b"hello").unwrap();
            assert!(c.has(0, 0, 0) && !c.has(0, 1, 1) && c.has(0, 3, 3) && c.complete(1));
            let mut buf = [0u8; 12];
            assert_eq!(c.read(0, 3 * BLOCK - 2, &mut buf).unwrap(), None);
            assert_eq!(c.read(0, 3 * BLOCK, &mut buf).unwrap(), Some(10));
            c.save().unwrap();
        }
        // The system deleted one file while the app was away.
        fs::remove_file(root.join("data/b.xml")).unwrap();
        let c = Cache::open(&root, &catalog).unwrap();
        assert!(c.has(0, 0, 0) && c.has(0, 3, 3) && !c.complete(1));
        let mut buf = [0u8; 4];
        assert_eq!(c.read(0, 0, &mut buf).unwrap(), Some(4));
        assert_eq!(buf, [7; 4]);
        assert!(c.evict(1) > 0 && !c.has(0, 0, 0));
        let _ = clear(&root);
    }

    #[test]
    fn verified_package_files_are_pinned_across_cache_reopens() {
        let root = std::env::temp_dir().join(format!("glue5-cache-pins-{}", std::process::id()));
        let _ = clear(&root);
        let entries = vec![
            Entry { path: "data/audio/voice.rpf".into(), size: 2 * BLOCK, mtime: 1 },
            Entry { path: "data/audio/other.rpf".into(), size: 2 * BLOCK, mtime: 1 },
        ];
        let catalog = Catalog::new("x".into(), entries, 2);
        {
            let cache = Cache::open(&root, &catalog).unwrap();
            cache.write_whole(0, &vec![1; (2 * BLOCK) as usize]).unwrap();
            cache.write_whole(1, &vec![2; (2 * BLOCK) as usize]).unwrap();
            cache.set_pinned(&[0]).unwrap();
            assert!(cache.is_pinned(0));
            assert_eq!(cache.evict(4 * BLOCK), 2 * BLOCK);
            assert!(cache.complete(0));
            assert!(!cache.complete(1));
        }
        let cache = Cache::open(&root, &catalog).unwrap();
        assert!(cache.is_pinned(0));
        assert!(!cache.is_pinned(1));
        assert_eq!(cache.evict(4 * BLOCK), 0);
        let _ = clear(&root);
    }

    /// Emptying takes the files this run used least recently first, the
    /// packs' files last.
    #[test]
    fn evicts_the_least_recent_and_the_packs_last() {
        let root = std::env::temp_dir().join(format!("glue5-cache-evict-{}", std::process::id()));
        let _ = clear(&root);
        let entries = vec![
            Entry { path: "data/x64/a.rpf".into(), size: 2 * BLOCK, mtime: 1 },
            Entry { path: "data/x64/b.rpf".into(), size: 2 * BLOCK, mtime: 1 },
            Entry { path: "data/common/data/c.xml".into(), size: 10, mtime: 1 },
        ];
        let c = Cache::open(&root, &Catalog::new("x".into(), entries, 3)).unwrap();
        let pause = || std::thread::sleep(std::time::Duration::from_millis(5));
        c.write_whole(2, b"<c>hi</c>!").unwrap();
        c.write(0, 0, &vec![1; 2 * BLOCK as usize]).unwrap();
        pause();
        c.write(1, 0, &vec![2; 2 * BLOCK as usize]).unwrap();
        pause();
        // a is read again: b is now the least recent.
        c.read(0, 0, &mut [0; 8]).unwrap();
        assert_eq!(c.evict(1), 2 * BLOCK);
        assert!(c.has(0, 0, 1) && !c.has(1, 0, 0) && c.complete(2));
        // Then a, and only then the pack's file, though used before both.
        assert_eq!(c.evict(1), 2 * BLOCK);
        assert!(!c.has(0, 0, 0) && c.complete(2));
        assert_eq!(c.evict(1), 10);
        assert!(!c.complete(2));
        let _ = clear(&root);
    }

    /// The boot set's files go after the run's others, though read first.
    #[test]
    fn evicts_the_boot_set_after_the_others() {
        let root = std::env::temp_dir().join(format!("glue5-cache-boot-{}", std::process::id()));
        let _ = clear(&root);
        let entries = vec![
            Entry { path: "data/x64/boot.rpf".into(), size: 2 * BLOCK, mtime: 1 },
            Entry { path: "data/x64/other.rpf".into(), size: 2 * BLOCK, mtime: 1 },
            Entry { path: "data/common/data/c.xml".into(), size: 10, mtime: 1 },
        ];
        let c = Cache::open(&root, &Catalog::new("x".into(), entries, 3)).unwrap();
        c.keep_longer(0);
        let pause = || std::thread::sleep(std::time::Duration::from_millis(5));
        c.write_whole(2, b"<c>hi</c>!").unwrap();
        c.write(0, 0, &vec![1; 2 * BLOCK as usize]).unwrap();
        pause();
        c.write(1, 0, &vec![2; 2 * BLOCK as usize]).unwrap();
        assert_eq!(c.evict(1), 2 * BLOCK);
        assert!(c.has(0, 0, 1) && !c.has(1, 0, 0) && c.complete(2));
        assert_eq!(c.evict(1), 2 * BLOCK);
        assert!(!c.has(0, 0, 0) && c.complete(2));
        let _ = clear(&root);
    }

    /// Blocks emptied stay gone after a crash right after (no save): the
    /// index on disk no longer claims them, though the file kept open is
    /// still there at its full length (of zeros).
    #[test]
    fn emptied_blocks_stay_gone_after_a_crash() {
        let root = std::env::temp_dir().join(format!("glue5-cache-emptied-{}", std::process::id()));
        let _ = clear(&root);
        let catalog = Catalog::new("x".into(), vec![Entry { path: "data/a.rpf".into(), size: 2 * BLOCK, mtime: 1 }], 1);
        {
            let c = Cache::open(&root, &catalog).unwrap();
            c.write(0, 0, &vec![5u8; 2 * BLOCK as usize]).unwrap();
            c.save().unwrap();
            assert_eq!(c.evict(1), 2 * BLOCK);
        }
        assert_eq!(fs::metadata(root.join("data/a.rpf")).unwrap().len(), 2 * BLOCK);
        let c = Cache::open(&root, &catalog).unwrap();
        assert!(!c.has(0, 0, 0) && !c.has(0, 1, 1));
        let _ = clear(&root);
    }

    /// A save that fails leaves its work to the next one.
    #[cfg(unix)]
    #[test]
    fn saves_again_after_a_failed_save() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("glue5-cache-failed-{}", std::process::id()));
        let _ = clear(&root);
        let catalog = Catalog::new("x".into(), vec![Entry { path: "data/a.rpf".into(), size: 2 * BLOCK, mtime: 1 }], 1);
        {
            let c = Cache::open(&root, &catalog).unwrap();
            c.write(0, 0, &vec![3u8; BLOCK as usize]).unwrap();
            // No index can be written meanwhile.
            fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).unwrap();
            assert!(c.save().is_err());
            fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
            c.save().unwrap();
        }
        let c = Cache::open(&root, &catalog).unwrap();
        assert!(c.has(0, 0, 0));
        let _ = clear(&root);
    }

    /// A copy whose index cannot be saved (a full volume) still opens, as
    /// it was.
    #[cfg(unix)]
    #[test]
    fn opens_when_the_index_cannot_be_saved() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("glue5-cache-unsaved-{}", std::process::id()));
        let _ = clear(&root);
        let entries = vec![
            Entry { path: "data/a.rpf".into(), size: 2 * BLOCK, mtime: 1 },
            Entry { path: "data/b.rpf".into(), size: BLOCK, mtime: 1 },
        ];
        let catalog = Catalog::new("x".into(), entries, 2);
        {
            let c = Cache::open(&root, &catalog).unwrap();
            c.write(0, 0, &vec![3u8; BLOCK as usize]).unwrap();
            c.save().unwrap();
        }
        // Nothing beside the files can be written.
        fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).unwrap();
        let reopened = Cache::open(&root, &catalog).map(|c| c.has(0, 0, 0));
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(reopened.unwrap());
        // Complete, the same.
        {
            let c = Cache::open(&root, &catalog).unwrap();
            c.write(0, BLOCK, &vec![4u8; BLOCK as usize]).unwrap();
            c.write(1, 0, &vec![5u8; BLOCK as usize]).unwrap();
            c.finish(&catalog).unwrap();
        }
        fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).unwrap();
        let reopened = Cache::open(&root, &catalog).map(|c| c.all_here());
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(reopened.unwrap());
        let _ = clear(&root);
    }

    /// A damaged index is no index, whatever it says.
    #[test]
    fn takes_a_damaged_index_for_none() {
        let root = std::env::temp_dir().join(format!("glue5-cache-damaged-{}", std::process::id()));
        let _ = clear(&root);
        fs::create_dir_all(&root).unwrap();
        let mut index = MAGIC.to_vec();
        for word in [VERSION, 1, u32::MAX] {
            index.extend_from_slice(&word.to_le_bytes());
        }
        fs::write(root.join("index"), &index).unwrap();
        assert!(load_index(&root.join("index")).is_none());
        let _ = clear(&root);
    }

    /// What the copy takes on disk is what it holds (APFS may fill holes when
    /// it writes a file out; the save gives them back).
    #[cfg(unix)]
    #[test]
    fn takes_only_its_blocks() {
        use std::os::unix::fs::MetadataExt;
        let root = std::env::temp_dir().join(format!("glue5-cache-sparse-{}", std::process::id()));
        let _ = clear(&root);
        let size = 37_007_360;
        let catalog = Catalog::new("x".into(), vec![Entry { path: "data/r.rpf".into(), size, mtime: 1 }], 1);
        let c = Cache::open(&root, &catalog).unwrap();
        let chunk = vec![1u8; 64 << 10];
        for off in [0, 15_794_176, 23_494_656, 36_700_160] {
            c.write(0, off, &chunk).unwrap();
        }
        c.save().unwrap();
        let taken = fs::metadata(root.join("data/r.rpf")).unwrap().blocks() * 512;
        assert!(taken <= 4 * (64 << 10) + (256 << 10), "{taken} bytes taken for 256 KB");
        let mut buf = vec![0u8; 64 << 10];
        assert_eq!(c.read(0, 23_494_656, &mut buf).unwrap(), Some(64 << 10));
        assert_eq!(buf, chunk);
        let _ = clear(&root);
    }
}
