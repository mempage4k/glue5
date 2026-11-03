//! Serves a game folder to clients.
//!
//! [`prepare`] lists the files and builds (or finds, from an earlier run)
//! the packs; [`Server::start`] then accepts connections. Each connection has
//! a reader thread, which queues the client's requests, and a sender thread,
//! which always sends the next chunk of the most urgent request: the queues
//! are per priority and a request moves between them when the client asks.

use std::collections::{HashMap, VecDeque};
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::catalog::Catalog;
use crate::pack::{self, Kind};
use crate::packages::{self, PackageManifest};
use crate::protocol::*;
use crate::sys;

pub struct Options {
    /// The game folder: `data/` and `b/<build>/shaders/`.
    pub game: PathBuf,
    pub build: String,
    /// Where built packs are kept between runs.
    pub cache: PathBuf,
    /// Bytes per second at most, all clients together.
    pub rate: Option<u64>,
}

/// A game folder ready to serve.
pub struct Prepared {
    shared: Arc<Shared>,
}

struct Shared {
    game: PathBuf,
    catalog: Catalog,
    files: Vec<OnceLock<File>>,
    blobs: Vec<(String, Blob)>,
    hello: Vec<u8>,
    rate: Option<Mutex<Bucket>>,
    sent: AtomicU64,
}

enum Blob {
    Memory(Vec<u8>),
    File(File, u64),
}

impl Blob {
    fn len(&self) -> u64 {
        match self {
            Blob::Memory(v) => v.len() as u64,
            Blob::File(_, n) => *n,
        }
    }
}

/// What [`prepare`] is doing, for a progress display.
#[derive(Clone, Debug)]
pub enum Step {
    Listing,
    /// Building a pack: bytes read so far, of the total.
    Packing(&'static str, u64, u64),
}

/// Lists the game folder and makes its packs (minutes the first time, with
/// all cores; read from `options.cache` after).
pub fn prepare(options: Options, progress: &dyn Fn(Step)) -> Result<Prepared> {
    progress(Step::Listing);
    sys::raise_open_file_limit();
    let catalog = Catalog::scan(&options.game, &options.build)?;
    // An optional sidecar in the game root avoids adding manager metadata to
    // the engine's `data/` manifest. Older clients ignore the extra blob.
    let packages_path = options.game.join(packages::MANIFEST_FILE);
    let package_manifest = match fs::read(&packages_path) {
        Ok(bytes) if bytes.len() > packages::MAX_MANIFEST_BYTES => {
            log::warn!("serve: ignoring oversized optional package manifest at {}", packages_path.display());
            None
        }
        Ok(bytes) => match PackageManifest::parse(&bytes, &catalog, None) {
            Ok(manifest) => Some((manifest, bytes)),
            Err(e) => {
                log::warn!("serve: ignoring invalid optional package manifest at {}: {e:#}", packages_path.display());
                None
            }
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e).with_context(|| format!("reading {}", packages_path.display())),
    };
    let bytes: u64 = catalog.entries.iter().map(|e| e.size).sum();
    log::info!(
        "serve: {} ({} files, {:.1} GB; {} of the engine), catalog {}",
        options.game.display(),
        catalog.entries.len(),
        bytes as f64 / 1e9,
        catalog.data_files,
        catalog.hash
    );
    fs::create_dir_all(&options.cache).with_context(|| format!("creating {}", options.cache.display()))?;
    let mut blobs = vec![("catalog".to_string(), Blob::Memory(zstd::encode_all(&catalog.to_json()[..], 19)?))];
    if let Some((manifest, bytes)) = &package_manifest {
        blobs.push((packages::MANIFEST_BLOB.to_string(), Blob::Memory(bytes.clone())));
        log::info!("serve: optional package manifest: {} packages ({})", manifest.packages.len(), packages_path.display());
    }
    let mut packs = serde_json::Map::new();
    // One preparation builds the packs at a time: another (the app sharing
    // again before the first was ready) waits, and finds them built.
    static PACKING: Mutex<()> = Mutex::new(());
    let _one = PACKING.lock().unwrap_or_else(|e| e.into_inner());
    for kind in Kind::ALL {
        let ids = pack::members(&catalog, kind);
        let hash = pack::hash(&catalog, &ids);
        let path = pack::cached_path(&options.cache, kind, &hash);
        if !path.is_file() {
            let started = Instant::now();
            let report = |done, total| progress(Step::Packing(kind.name(), done, total));
            pack::build(&options.game, &catalog, &ids, &path, &report)?;
            log::info!("serve: {} pack built in {:.0?}", kind.name(), started.elapsed());
        }
        pack::remove_others(&options.cache, kind, &path);
        let file = File::open(&path)?;
        let len = file.metadata()?.len();
        log::info!("serve: {} pack: {} files, {:.1} MB", kind.name(), ids.len(), len as f64 / 1e6);
        packs.insert(kind.name().into(), serde_json::json!({ "hash": hash, "files": ids.len() }));
        blobs.push((kind.name().to_string(), Blob::File(file, len)));
    }
    // The whale5 web build's boot read sets (the ranges every start reads), if the folder has them.
    for name in ["bootset", "bootset_low"] {
        if let Ok(json) = fs::read(options.game.join("data").join(format!("{name}.json"))) {
            blobs.push((name.to_string(), Blob::Memory(zstd::encode_all(&json[..], 19)?)));
        }
    }
    let sizes: serde_json::Map<_, _> = blobs.iter().map(|(n, b)| (n.clone(), b.len().into())).collect();
    let mut hello_json = serde_json::json!({
        "protocol": PROTOCOL, "build": options.build, "catalog": catalog.hash, "blobs": sizes, "packs": packs,
    });
    if let Some((manifest, bytes)) = &package_manifest {
        hello_json["server_id"] = manifest.server_id.clone().into();
        hello_json["packages_hash"] = crate::catalog::hex(&Sha256::digest(bytes)).into();
    }
    let hello = serde_json::to_vec(&hello_json)?;
    let files = (0..catalog.entries.len()).map(|_| OnceLock::new()).collect();
    let rate = options.rate.map(|r| Mutex::new(Bucket { rate: r.max(1), tokens: 0.0, at: Instant::now() }));
    let shared = Shared { game: options.game, catalog, files, blobs, hello, rate, sent: AtomicU64::new(0) };
    Ok(Prepared { shared: Arc::new(shared) })
}

impl Prepared {
    pub fn catalog(&self) -> &Catalog {
        &self.shared.catalog
    }
}

/// A running server; dropping it stops it.
pub struct Server {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    /// The connections open now (to close them when the server stops).
    clients: Arc<Mutex<HashMap<u64, TcpStream>>>,
    accept: Option<thread::JoinHandle<()>>,
}

impl Server {
    pub fn start(prepared: &Prepared, bind: SocketAddr) -> Result<Server> {
        let listener = TcpListener::bind(bind).with_context(|| format!("listening on {bind}"))?;
        let addr = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let clients = Arc::new(Mutex::new(HashMap::new()));
        let shared = prepared.shared.clone();
        let accept = {
            let (stop, clients) = (stop.clone(), clients.clone());
            thread::Builder::new().name("serve".into()).spawn(move || {
                for (id, stream) in (0u64..).zip(listener.incoming()) {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let stream = match stream {
                        Ok(stream) => stream,
                        // Out of files, say: a moment for some to close.
                        Err(_) => {
                            thread::sleep(Duration::from_millis(100));
                            continue;
                        }
                    };
                    // Two threads each: a few dozen at most (a device's
                    // stream and its Servers page take one or two).
                    if clients.lock().unwrap().len() >= MAX_CLIENTS {
                        continue;
                    }
                    if let Ok(clone) = stream.try_clone() {
                        clients.lock().unwrap().insert(id, clone);
                    }
                    let (shared, list) = (shared.clone(), clients.clone());
                    let spawned = thread::Builder::new().name("serve client".into()).spawn(move || {
                        // Its socket closes with the last handle: kept in the
                        // list, a closed connection stayed open (a phone's
                        // Servers page asks every 5 s).
                        let _leave = Leave(&list, id);
                        serve(shared, stream);
                    });
                    if spawned.is_err() {
                        clients.lock().unwrap().remove(&id);
                    }
                }
            })?
        };
        log::info!("serve: listening on {addr}");
        Ok(Server { addr, stop, clients, accept: Some(accept) })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The connections open now.
    #[cfg(test)]
    pub(crate) fn connections(&self) -> usize {
        self.clients.lock().unwrap().len()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Wakes the accept loop up.
        let wake = if self.addr.ip().is_unspecified() { SocketAddr::from(([127, 0, 0, 1], self.addr.port())) } else { self.addr };
        let _ = TcpStream::connect_timeout(&wake, Duration::from_millis(200));
        for (_, c) in self.clients.lock().unwrap().drain() {
            let _ = c.shutdown(Shutdown::Both);
        }
        if let Some(t) = self.accept.take() {
            let _ = t.join();
        }
    }
}

/// Takes a connection off the server's list when its thread ends, however
/// it ends.
struct Leave<'a>(&'a Mutex<HashMap<u64, TcpStream>>, u64);

impl Drop for Leave<'_> {
    fn drop(&mut self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.1);
    }
}

/// A request being answered.
struct Job {
    prio: u8,
    source: Source,
    next: u64,
    end: u64,
    /// [`END`] status once the range is sent.
    status: u32,
}

#[derive(Clone, Copy)]
enum Source {
    File(u32),
    Blob(usize),
}

/// Requests one connection may have waiting: far more than a client asks
/// (its boot set is about 9,000), far less than what would run the server
/// out of memory.
const MAX_QUEUED: usize = 1 << 18;
/// Connections served at once.
const MAX_CLIENTS: usize = 64;

#[derive(Default)]
struct Queue {
    /// Request numbers per priority, oldest first. An entry whose job has
    /// moved to another priority (or ended) is skipped when reached.
    levels: [VecDeque<u32>; LEVELS],
    jobs: HashMap<u32, Job>,
    closed: bool,
}

/// One chunk to send.
struct Chunk {
    req: u32,
    source: Source,
    off: u64,
    len: u32,
    /// The request's last chunk: its status follows.
    end: Option<u32>,
}

impl Queue {
    fn push(&mut self, req: u32, job: Job) {
        self.levels[job.prio as usize].push_back(req);
        self.jobs.insert(req, job);
    }

    fn next(&mut self) -> Option<Chunk> {
        for (level, queue) in self.levels.iter_mut().enumerate() {
            while let Some(&req) = queue.front() {
                let Some(job) = self.jobs.get_mut(&req).filter(|j| j.prio as usize == level) else {
                    queue.pop_front();
                    continue;
                };
                let off = job.next;
                let len = (job.end - off).min(CHUNK as u64) as u32;
                job.next += len as u64;
                let end = (job.next == job.end).then_some(job.status);
                let source = job.source;
                if end.is_some() {
                    queue.pop_front();
                    self.jobs.remove(&req);
                }
                return Some(Chunk { req, source, off, len, end });
            }
        }
        None
    }
}

struct Conn {
    queue: Mutex<Queue>,
    ready: Condvar,
}

/// Closes a connection's queue when the reader ends, however it ends: its
/// sender then ends too.
struct Close<'a>(&'a Conn);

impl Drop for Close<'_> {
    fn drop(&mut self) {
        self.0.queue.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
        self.0.ready.notify_all();
    }
}

fn serve(shared: Arc<Shared>, stream: TcpStream) {
    let peer = stream.peer_addr().map_or_else(|_| "?".to_string(), |a| a.to_string());
    sys::tune_server_socket(&stream);
    let started = Instant::now();
    let mut writer = match stream.try_clone() {
        Ok(w) => w,
        Err(_) => return,
    };
    let hello = Header { kind: HELLO, len: shared.hello.len() as u32, ..Default::default() };
    if hello.send(&mut writer, &shared.hello).is_err() {
        return;
    }
    let conn = Arc::new(Conn { queue: Mutex::new(Queue::default()), ready: Condvar::new() });
    let close = Close(&conn);
    let sender = {
        let (shared, conn) = (shared.clone(), conn.clone());
        thread::Builder::new().name("serve send".into()).spawn(move || send(&shared, &conn, writer))
    };
    let mut reader = BufReader::with_capacity(64 * 1024, stream.try_clone().expect("socket clone"));
    let (mut reads, mut asked) = (0u64, 0u64);
    // Whether it asked anything: a client that only reads the greeting (an
    // app checking whether this server serves its game) is not a session.
    let mut asking = false;
    let result: io::Result<()> = (|| {
        loop {
            let h = Header::read(&mut reader)?;
            if !asking {
                asking = true;
                log::info!("serve: {peer} connected");
            }
            let queue_job = |job: Job| -> io::Result<()> {
                let mut q = conn.queue.lock().unwrap();
                // Asked twice: answered once.
                if q.jobs.contains_key(&h.req) {
                    return Ok(());
                }
                if q.jobs.len() >= MAX_QUEUED {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "too many requests"));
                }
                q.push(h.req, job);
                conn.ready.notify_one();
                Ok(())
            };
            let prio = h.prio.min(LEVELS as u8 - 1);
            match h.kind {
                READ => {
                    reads += 1;
                    asked += h.len as u64;
                    let Some(entry) = shared.catalog.entries.get(h.file as usize).filter(|_| h.len <= MAX_READ) else {
                        queue_job(Job { prio, source: Source::File(0), next: 0, end: 0, status: UNKNOWN })?;
                        continue;
                    };
                    let want = h.off.saturating_add(h.len as u64);
                    let end = want.min(entry.size);
                    let start = h.off.min(end);
                    let status = if end < want { SHORT } else { DONE };
                    if end > start
                        && let Ok(f) = shared.file(h.file)
                    {
                        sys::advise(f, start, end - start);
                    }
                    queue_job(Job { prio, source: Source::File(h.file), next: start, end, status })?;
                }
                BLOB => {
                    let name = payload(&mut reader, h.len, MAX_MESSAGE)?;
                    let name = String::from_utf8_lossy(&name);
                    match shared.blobs.iter().position(|(n, _)| *n == name) {
                        Some(i) => {
                            let end = shared.blobs[i].1.len();
                            queue_job(Job { prio, source: Source::Blob(i), next: 0, end, status: DONE })?
                        }
                        None => queue_job(Job { prio, source: Source::Blob(0), next: 0, end: 0, status: UNKNOWN })?,
                    }
                }
                PRIORITY => {
                    let mut q = conn.queue.lock().unwrap();
                    // Only a raise (all a client sends): a job moves to
                    // another level a few times at most.
                    if let Some(job) = q.jobs.get_mut(&h.req)
                        && prio < job.prio
                    {
                        job.prio = prio;
                        q.levels[prio as usize].push_back(h.req);
                    }
                }
                _ => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("message kind {}", h.kind))),
            }
        }
    })();
    drop(close);
    let _ = stream.shutdown(Shutdown::Both);
    if let Ok(t) = sender {
        let _ = t.join();
    }
    // The client closing the connection is how it ends.
    let why = match result {
        Err(e) if e.kind() != io::ErrorKind::UnexpectedEof => format!(" ({e})"),
        _ => String::new(),
    };
    if !asking {
        log::debug!("serve: {peer} read the greeting and left{why}");
        return;
    }
    log::info!(
        "serve: {peer} left after {:.0?}{why}: {reads} reads, {:.0} MB asked; {:.0} MB sent in all",
        started.elapsed(),
        asked as f64 / 1e6,
        shared.sent.load(Ordering::Relaxed) as f64 / 1e6
    );
}

fn send(shared: &Shared, conn: &Conn, mut w: TcpStream) {
    // A chunk with its header, and the request's end when it is the last
    // (one write, one segment for both).
    let mut buf = vec![0u8; HEADER + CHUNK as usize + HEADER];
    loop {
        let chunk = {
            let mut q = conn.queue.lock().unwrap();
            loop {
                if q.closed {
                    return;
                }
                if let Some(c) = q.next() {
                    break c;
                }
                q = conn.ready.wait(q).unwrap();
            }
        };
        let mut status = chunk.end;
        let mut n = chunk.len as usize;
        if n > 0 {
            let data = &mut buf[HEADER..HEADER + n];
            let got = match chunk.source {
                Source::File(id) => shared.file(id).and_then(|f| sys::read_full_at(f, data, chunk.off)),
                Source::Blob(i) => shared.blob(i, data, chunk.off),
            };
            match got {
                Ok(got) if got == n => {}
                // The file is shorter than when it was listed: what there is, and the end.
                Ok(got) => {
                    n = got;
                    status = Some(SHORT);
                    conn.queue.lock().unwrap().jobs.remove(&chunk.req);
                }
                Err(e) => {
                    log::warn!("serve: reading {}: {e}", shared.name(chunk.source));
                    n = 0;
                    status = Some(FAILED);
                    conn.queue.lock().unwrap().jobs.remove(&chunk.req);
                }
            }
        }
        // The data (with its header in front), then the end header after it.
        let (start, mut len) = if n > 0 { (0, HEADER + n) } else { (HEADER, HEADER) };
        if n > 0 {
            if let Some(rate) = &shared.rate {
                Bucket::take(rate, n as u64);
            }
            let file = match chunk.source {
                Source::File(id) => id,
                Source::Blob(_) => u32::MAX,
            };
            let crc = crc(&buf[HEADER..HEADER + n]);
            let header = Header { kind: DATA, req: chunk.req, off: chunk.off, len: n as u32, file, prio: 0, crc };
            buf[..HEADER].copy_from_slice(&header.encode());
        }
        if let Some(status) = status {
            let end = Header { kind: END, req: chunk.req, off: chunk.off + n as u64, len: status, ..Default::default() };
            buf[len..len + HEADER].copy_from_slice(&end.encode());
            len += HEADER;
        }
        if len > start {
            if n > 0 {
                sys::wait_writable(&w);
            }
            if io::Write::write_all(&mut w, &buf[start..len]).is_err() {
                return;
            }
            shared.sent.fetch_add(n as u64, Ordering::Relaxed);
        }
    }
}

impl Shared {
    fn file(&self, id: u32) -> io::Result<&File> {
        let slot = &self.files[id as usize];
        if let Some(f) = slot.get() {
            return Ok(f);
        }
        let f = File::open(self.game.join(&self.catalog.entries[id as usize].path))?;
        Ok(slot.get_or_init(|| f))
    }

    fn blob(&self, i: usize, buf: &mut [u8], off: u64) -> io::Result<usize> {
        match &self.blobs[i].1 {
            Blob::Memory(v) => {
                let start = (off as usize).min(v.len());
                let n = buf.len().min(v.len() - start);
                buf[..n].copy_from_slice(&v[start..start + n]);
                Ok(n)
            }
            Blob::File(f, _) => sys::read_full_at(f, buf, off),
        }
    }

    fn name(&self, source: Source) -> String {
        match source {
            Source::File(id) => self.catalog.entries[id as usize].path.clone(),
            Source::Blob(i) => self.blobs[i].0.clone(),
        }
    }
}

/// A token bucket: the rate limit of all connections together.
struct Bucket {
    rate: u64,
    tokens: f64,
    at: Instant,
}

impl Bucket {
    fn take(bucket: &Mutex<Bucket>, n: u64) {
        let wait = {
            let mut b = bucket.lock().unwrap();
            let now = Instant::now();
            // At most a tenth of a second of burst.
            b.tokens = (b.tokens + now.duration_since(b.at).as_secs_f64() * b.rate as f64).min(b.rate as f64 / 10.0);
            b.at = now;
            b.tokens -= n as f64;
            (b.tokens < 0.0).then(|| Duration::from_secs_f64(-b.tokens / b.rate as f64))
        };
        if let Some(wait) = wait {
            thread::sleep(wait);
        }
    }
}
