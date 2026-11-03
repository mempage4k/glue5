//! A server and a client on this machine, over a small made-up game.

use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::cache::BLOCK;
use crate::catalog::Catalog;
use crate::client::{Client, Options, Phase};
use crate::protocol::{CHUNK, DATA, HEADER, HELLO, Header};
use crate::server::{self, Server};
use crate::packages::{TransferControl, TransferPhase};
use sha2::{Digest, Sha256};

const BUILD: &str = "testbuild";

fn noise(n: usize, seed: u64) -> Vec<u8> {
    let mut x = seed | 1;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("glue5-stream-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn game(dir: &Path) -> Vec<u8> {
    let big = noise(3 << 20 | 123, 7);
    let put = |path: &str, data: &[u8]| {
        let p = dir.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    };
    put("data/x64/big.rpf", &big);
    put("data/x64/later.rpf", &noise(1 << 20, 9));
    put("data/common/data/a.xml", &b"<a>hello</a>".repeat(1000));
    put("data/common/data/empty.meta", b"");
    put("data/manifest.json", b"{}");
    put("data/bootset.json", br#"[["x64/big.rpf", [[0, 5000], [1048576, 1100000]]]]"#);
    put(&format!("b/{BUILD}/shaders/index.json"), br#"{"00":{"ok":true}}"#);
    put(&format!("b/{BUILD}/shaders/00.wgsl"), b"fn main() {}");
    put(&format!("b/{BUILD}/shaders/pack0.x.bin"), b"skipped");
    big
}

fn serve(dir: &Path, cache: &Path, addr: SocketAddr) -> Server {
    let opts = server::Options { game: dir.to_owned(), build: BUILD.into(), cache: cache.to_owned(), rate: None };
    let prepared = server::prepare(opts, &|_| {}).unwrap();
    Server::start(&prepared, addr).unwrap()
}

/// Between a client and a server: passes on what each sends, or holds it
/// (the client's requests, the server's answers), or changes the headers
/// of the server's messages.
struct Proxy {
    addr: SocketAddr,
    up: Arc<AtomicBool>,
    down: Arc<AtomicBool>,
    accepted: Arc<AtomicUsize>,
    done: Arc<AtomicBool>,
    sockets: Arc<Mutex<Vec<TcpStream>>>,
}

type Tamper = Arc<dyn Fn(&mut Header) + Send + Sync>;

impl Proxy {
    fn start(server: SocketAddr, tamper: Option<Tamper>) -> Proxy {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        // Little room for requests the proxy holds: the client's writes stop.
        let _ = socket2::SockRef::from(&listener).set_recv_buffer_size(16 << 10);
        let p = Proxy {
            addr: listener.local_addr().unwrap(),
            up: Arc::new(AtomicBool::new(true)),
            down: Arc::new(AtomicBool::new(true)),
            accepted: Arc::new(AtomicUsize::new(0)),
            done: Arc::new(AtomicBool::new(false)),
            sockets: Arc::new(Mutex::new(Vec::new())),
        };
        let (up, down, accepted, done, sockets) = (p.up.clone(), p.down.clone(), p.accepted.clone(), p.done.clone(), p.sockets.clone());
        std::thread::spawn(move || {
            for client in listener.incoming() {
                if done.load(Ordering::Acquire) {
                    break;
                }
                let (Ok(client), Ok(server)) = (client, TcpStream::connect(server)) else { continue };
                accepted.fetch_add(1, Ordering::AcqRel);
                sockets.lock().unwrap().extend([client.try_clone().unwrap(), server.try_clone().unwrap()]);
                pump(client.try_clone().unwrap(), server.try_clone().unwrap(), up.clone(), done.clone(), None);
                pump(server, client, down.clone(), done.clone(), tamper.clone());
            }
        });
        p
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.addr);
        for s in self.sockets.lock().unwrap().drain(..) {
            let _ = s.shutdown(Shutdown::Both);
        }
    }
}

/// Passes `from`'s bytes on to `to` while `pass` (with `tamper`, a message
/// at a time).
fn pump(mut from: TcpStream, mut to: TcpStream, pass: Arc<AtomicBool>, done: Arc<AtomicBool>, tamper: Option<Tamper>) {
    std::thread::spawn(move || {
        let held = || {
            while !pass.load(Ordering::Acquire) {
                if done.load(Ordering::Acquire) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            false
        };
        let mut buf = vec![0u8; 1 << 20];
        loop {
            if held() {
                return;
            }
            let n = match &tamper {
                Some(tamper) => {
                    let Ok(mut h) = Header::read(&mut from) else { break };
                    let n = HEADER + if matches!(h.kind, HELLO | DATA) { h.len as usize } else { 0 };
                    if from.read_exact(&mut buf[HEADER..n]).is_err() {
                        break;
                    }
                    tamper(&mut h);
                    buf[..HEADER].copy_from_slice(&h.encode());
                    n
                }
                None => match from.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                },
            };
            // Also what came while waiting to read.
            if held() || to.write_all(&buf[..n]).is_err() {
                break;
            }
        }
        let _ = from.shutdown(Shutdown::Both);
        let _ = to.shutdown(Shutdown::Both);
    });
}

/// Whether `f` returns within a second.
fn promptly(f: impl FnOnce() + Send + 'static) -> bool {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        f();
        let _ = tx.send(());
    });
    rx.recv_timeout(Duration::from_secs(1)).is_ok()
}

/// A server of a test's own making: its greeting, then the blobs asked for.
fn fake_server(hello: serde_json::Value, blobs: Vec<(&'static str, Vec<u8>)>) -> SocketAddr {
    use crate::protocol::*;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let hello = serde_json::to_vec(&hello).unwrap();
    std::thread::spawn(move || {
        for s in listener.incoming() {
            let Ok(mut s) = s else { continue };
            let _ = Header { kind: HELLO, len: hello.len() as u32, ..Default::default() }.send(&mut s, &hello);
            while let Ok(h) = Header::read(&mut s) {
                let Ok(name) = payload(&mut s, h.len, MAX_MESSAGE) else { break };
                let data = blobs.iter().find(|(n, _)| n.as_bytes() == name).map_or(&[][..], |(_, d)| &d[..]);
                for (k, chunk) in data.chunks(CHUNK as usize).enumerate() {
                    let off = (k * CHUNK as usize) as u64;
                    let data = Header { kind: DATA, req: h.req, off, len: chunk.len() as u32, file: u32::MAX, prio: 0, crc: crc(chunk) };
                    let _ = data.send(&mut s, chunk);
                }
                let _ = s.write_all(&Header { kind: END, req: h.req, len: DONE, ..Default::default() }.encode());
            }
        }
    });
    addr
}

/// Waits for the client's error to say `what`.
fn wait_error(c: &Client, what: &str) {
    let started = Instant::now();
    while !c.status().error.is_some_and(|e| e.contains(what)) {
        assert!(started.elapsed() < Duration::from_secs(5), "{:?}", c.status());
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_ready(c: &Client) {
    let started = Instant::now();
    while !c.is_ready() {
        assert!(started.elapsed() < Duration::from_secs(30), "not ready: {:?}", c.status());
        if let Phase::Failed(e) = c.status().phase {
            panic!("{e}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn optional_package_manifest_is_negotiated_without_reordering_catalog_ids() {
    let dir = temp("packages-game");
    game(&dir);
    let file_path = "data/x64/audio/voice.rpf";
    let file_bytes = vec![0x3a; (CHUNK as usize) * 2 + 3];
    fs::write(dir.join(file_path), &file_bytes).unwrap();
    let catalog = Catalog::scan(&dir, BUILD).unwrap();
    let file_id = catalog.find(file_path).unwrap();
    let manifest = serde_json::json!({
        "schema_version": 1,
        "server_id": "test-server",
        "build": BUILD,
        "catalog_hash": catalog.hash,
        "protocol_min": 1,
        "protocol_max": 1,
        "packages": [{
            "id": "voice-en", "name": "Voice", "description": "synthetic fixture",
            "kind": "dialogue", "version": "1", "target_build": BUILD,
            "catalog_hash": catalog.hash, "protocol_min": 1, "protocol_max": 1,
            "dependencies": [],
            "files": [{"path": file_path, "size": file_bytes.len(),
                "sha256": crate::catalog::hex(&Sha256::digest(&file_bytes)),
                "catalog_id": file_id, "dependencies": []}],
            "total_size": file_bytes.len(), "optional": true, "available": true,
            "locale": "en", "created_at": null
        }]
    });
    fs::write(dir.join(crate::packages::MANIFEST_FILE), serde_json::to_vec(&manifest).unwrap()).unwrap();
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    let address = server.addr().to_string();
    let root = temp("packages-copy");
    let client = Client::start(Options {
        server: address.clone(), root: root.clone(), build: BUILD.into(), low_memory: false,
    });
    wait_ready(&client);
    let received = client.packages().expect("new server advertises package metadata");
    assert_eq!(received.server_id, "test-server");
    assert_eq!(received.packages[0].files[0].catalog_id, file_id);
    assert_eq!(client.find(file_path), Some(file_id));
    let cancel = TransferControl::default();
    let stopped_once = AtomicBool::new(false);
    let interrupted = client.download_package("voice-en", &cancel, |p| {
        if p.phase == TransferPhase::Downloading && p.file_done > 0 && !stopped_once.swap(true, Ordering::AcqRel) {
            cancel.cancel();
        }
    });
    assert!(interrupted.is_err());
    assert!(!client.package_is_installed("voice-en"));
    assert!(!root.join("packages-state.json").exists());
    let mut damaged = fs::OpenOptions::new().write(true).open(root.join(file_path)).unwrap();
    damaged.write_all(b"corrupt fixture bytes").unwrap();
    damaged.sync_all().unwrap();

    let retry = TransferControl::default();
    let phases = Mutex::new(Vec::new());
    client.download_package("voice-en", &retry, |p| phases.lock().unwrap().push(p.phase)).unwrap();
    assert!(phases.lock().unwrap().contains(&TransferPhase::Verifying));
    assert!(phases.lock().unwrap().contains(&TransferPhase::Installed));
    assert!(client.package_is_installed("voice-en"));
    let bytes = fs::read(root.join(file_path)).unwrap();
    assert_eq!(bytes, file_bytes);
    let state: serde_json::Value = serde_json::from_slice(&fs::read(root.join("packages-state.json")).unwrap()).unwrap();
    assert!(state["installed"]["voice-en"].is_object());
    assert!(String::from_utf8_lossy(&fs::read(root.join("package-pins.json")).unwrap()).contains(file_path));
    client.stop();
    client.join();
    drop(server);
    let offline = Client::start(Options {
        server: address, root: root.clone(), build: BUILD.into(), low_memory: false,
    });
    wait_ready(&offline);
    assert!(offline.packages().is_some());
    assert!(offline.package_is_installed("voice-en"));
    offline.stop();
    offline.join();
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn invalid_optional_package_manifest_does_not_prevent_base_streaming() {
    let dir = temp("bad-packages-game");
    game(&dir);
    fs::write(dir.join(crate::packages::MANIFEST_FILE), b"not-json").unwrap();
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    let root = temp("bad-packages-copy");
    let client = Client::start(Options {
        server: server.addr().to_string(), root: root.clone(), build: BUILD.into(), low_memory: false,
    });
    wait_ready(&client);
    assert!(client.packages().is_none());
    client.stop();
    client.join();
    drop(server);
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn end_to_end() {
    let dir = temp("game");
    let big = game(&dir);
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    let addr = server.addr();
    let root = temp("copy");
    let opts = || Options { server: addr.to_string(), root: root.clone(), build: BUILD.into(), low_memory: false };
    let client = Client::start(opts());
    wait_ready(&client);

    // The packs brought the small files and the shaders, whole.
    let xml = client.find("data/common/data/a.xml").unwrap();
    assert!(client.path(xml).unwrap().is_file());
    assert_eq!(fs::read(root.join(format!("b/{BUILD}/shaders/00.wgsl"))).unwrap(), b"fn main() {}");
    assert!(!root.join(format!("b/{BUILD}/shaders/pack0.x.bin")).exists());
    let manifest: serde_json::Value = serde_json::from_slice(client.engine_manifest().unwrap()).unwrap();
    assert_eq!(manifest["mount"], "/game/");
    assert!(manifest["files"].as_array().unwrap().iter().all(|f| !f[0].as_str().unwrap().starts_with("b/")));
    assert!(!manifest.to_string().contains("bootset"));

    // Reads from several threads, cached or not, byte for byte.
    let id = client.find("data/x64/big.rpf").unwrap();
    let big = Arc::new(big);
    let threads: Vec<_> = (0..8u64)
        .map(|t| {
            let (client, big) = (client.clone(), big.clone());
            std::thread::spawn(move || {
                for i in 0..40u64 {
                    let off = (t * 7919 + i * 104729 * 13) % big.len() as u64;
                    let len = 1 + ((t + 1) * (i + 3) * 4099) as usize % 200_000;
                    let mut buf = vec![0u8; len];
                    let n = client.read(id, off, &mut buf).unwrap();
                    let end = (off as usize + len).min(big.len());
                    assert_eq!(n, end - off as usize);
                    assert_eq!(&buf[..n], &big[off as usize..end]);
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let mut tail = [0u8; 300];
    assert_eq!(client.read(id, big.len() as u64 - 100, &mut tail), Some(100));
    assert_eq!(&tail[..100], &big[big.len() - 100..]);

    // The boot set arrives by itself.
    let started = Instant::now();
    while client.status().boot.0 < client.status().boot.1 {
        assert!(started.elapsed() < Duration::from_secs(10), "boot set: {:?}", client.status().boot);
        std::thread::sleep(Duration::from_millis(20));
    }

    // The server goes away and comes back on the same port.
    client.hint(id, 2 << 20, BLOCK, true);
    drop(server);
    let mut buf = vec![0u8; 4096];
    assert_eq!(client.read(id, 0, &mut buf), Some(4096));
    let back = serve(&dir, &dir.join("packs"), addr);
    let later = client.find("data/x64/later.rpf").unwrap();
    let mut buf = vec![0u8; 8192];
    assert_eq!(client.read(later, 300_000, &mut buf), Some(8192));
    assert_eq!(&buf[..], &noise(1 << 20, 9)[300_000..300_000 + 8192]);
    client.stop();
    client.join();
    drop(back);

    // Without a server, the copy plays.
    let offline = Client::start(Options { server: "127.0.0.1:1".into(), ..opts() });
    wait_ready(&offline);
    let mut buf = vec![0u8; 5000];
    assert_eq!(offline.read(id, 0, &mut buf), Some(5000));
    assert_eq!(&buf[..], &big[..5000]);
    offline.stop();
    offline.join();
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn demand_overtakes_prefetch() {
    let dir = temp("prio");
    let data = noise(32 << 20, 3);
    fs::create_dir_all(dir.join("data")).unwrap();
    fs::write(dir.join("data/huge.rpf"), &data).unwrap();
    fs::create_dir_all(dir.join(format!("b/{BUILD}/shaders"))).unwrap();
    fs::write(dir.join(format!("b/{BUILD}/shaders/index.json")), b"{}").unwrap();
    // 8 MB/s: the whole file takes 4 s.
    let opts = server::Options { game: dir.clone(), build: BUILD.into(), cache: dir.join("packs"), rate: Some(8_000_000) };
    let prepared = server::prepare(opts, &|_| {}).unwrap();
    let server = Server::start(&prepared, "127.0.0.1:0".parse().unwrap()).unwrap();
    let root = temp("prio-copy");
    let client = Client::start(Options {
        server: server.addr().to_string(),
        root: root.clone(),
        build: BUILD.into(),
        low_memory: false,
    });
    wait_ready(&client);
    let id = client.find("data/huge.rpf").unwrap();
    client.hint(id, 0, data.len() as u64, true);
    std::thread::sleep(Duration::from_millis(300));
    let started = Instant::now();
    let mut buf = vec![0u8; 4096];
    let off = data.len() as u64 - 4096;
    assert_eq!(client.read(id, off, &mut buf), Some(4096));
    assert_eq!(&buf[..], &data[off as usize..]);
    let waited = started.elapsed();
    assert!(waited < Duration::from_millis(100), "the demand read waited {waited:?} behind the prefetch");
    client.stop();
    client.join();
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn counts_reads_not_announced_in_time() {
    let dir = temp("late");
    let data = noise(16 << 20, 5);
    fs::create_dir_all(dir.join("data")).unwrap();
    fs::write(dir.join("data/a.rpf"), &data).unwrap();
    fs::write(dir.join("data/b.rpf"), &data[..1 << 20]).unwrap();
    fs::create_dir_all(dir.join(format!("b/{BUILD}/shaders"))).unwrap();
    fs::write(dir.join(format!("b/{BUILD}/shaders/index.json")), b"{}").unwrap();
    // 8 MB/s: what is announced takes two seconds to come.
    let opts = server::Options { game: dir.clone(), build: BUILD.into(), cache: dir.join("packs"), rate: Some(8_000_000) };
    let prepared = server::prepare(opts, &|_| {}).unwrap();
    let server = Server::start(&prepared, "127.0.0.1:0".parse().unwrap()).unwrap();
    let root = temp("late-copy");
    let client = Client::start(Options {
        server: server.addr().to_string(),
        root: root.clone(),
        build: BUILD.into(),
        low_memory: false,
    });
    wait_ready(&client);
    let (a, b) = (client.find("data/a.rpf").unwrap(), client.find("data/b.rpf").unwrap());
    client.hint(a, 0, data.len() as u64, true);
    std::thread::sleep(Duration::from_millis(300));
    // Announced in time, on its way: waits, but counts not.
    let at = 12 << 20;
    let mut buf = vec![0u8; 256 << 10];
    assert_eq!(client.read(a, at, &mut buf[..4096]), Some(4096));
    // Its first block is here now (with the rest of its request), the
    // others on their way: announced in time too.
    assert_eq!(client.read(a, at + 4000, &mut buf), Some(buf.len()));
    assert_eq!(&buf[..], &data[at as usize + 4000..][..buf.len()]);
    assert_eq!(client.not_in_time(), 0);
    // Never announced.
    assert_eq!(client.read(b, 5000, &mut buf[..100]), Some(100));
    assert_eq!(client.not_in_time(), 1);
    client.stop();
    client.join();
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn reads_ahead_only_what_is_played() {
    let dir = temp("ahead");
    let data = noise(4 << 20, 11);
    fs::create_dir_all(dir.join("data")).unwrap();
    fs::write(dir.join("data/music.rpf"), &data).unwrap();
    fs::write(dir.join("data/model.rpf"), &data).unwrap();
    fs::create_dir_all(dir.join(format!("b/{BUILD}/shaders"))).unwrap();
    fs::write(dir.join(format!("b/{BUILD}/shaders/index.json")), b"{}").unwrap();
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    let root = temp("ahead-copy");
    let client = Client::start(Options {
        server: server.addr().to_string(),
        root: root.clone(),
        build: BUILD.into(),
        low_memory: false,
    });
    wait_ready(&client);
    let (music, model) = (client.find("data/music.rpf").unwrap(), client.find("data/model.rpf").unwrap());
    // Played: 32 KB every 50 ms. What follows comes before it is read.
    let mut buf = vec![0u8; 32 << 10];
    for k in 0..3 {
        assert_eq!(client.read(music, k * (32 << 10), &mut buf), Some(buf.len()));
        std::thread::sleep(Duration::from_millis(50));
    }
    let started = Instant::now();
    while !client.has(music, 320 << 10) {
        assert!(started.elapsed() < Duration::from_secs(2), "nothing read ahead of the music");
        std::thread::sleep(Duration::from_millis(10));
    }
    // Small pieces of one resource read back to back: nothing past them.
    let mut piece = vec![0u8; 16 << 10];
    for k in 0..3 {
        assert_eq!(client.read(model, k * (16 << 10), &mut piece), Some(piece.len()));
        assert_eq!(&piece[..], &data[(k * (16 << 10)) as usize..][..piece.len()]);
    }
    std::thread::sleep(Duration::from_millis(300));
    assert!(!client.has(model, 48 << 10), "read ahead of small pieces read back to back");
    client.stop();
    client.join();
    drop(server);
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn boot_set_follows_the_engine() {
    let dir = temp("follow");
    let (a, b) = (noise(8 << 20, 11), noise(8 << 20, 13));
    fs::create_dir_all(dir.join("data/x64")).unwrap();
    fs::write(dir.join("data/x64/a.rpf"), &a).unwrap();
    fs::write(dir.join("data/x64/b.rpf"), &b).unwrap();
    fs::write(dir.join("data/bootset.json"), br#"[["x64/a.rpf", [[0, 8388607]]], ["x64/b.rpf", [[0, 8388607]]]]"#).unwrap();
    fs::create_dir_all(dir.join(format!("b/{BUILD}/shaders"))).unwrap();
    fs::write(dir.join(format!("b/{BUILD}/shaders/index.json")), b"{}").unwrap();
    // 8 MB/s: the boot set takes two seconds, a.rpf first.
    let opts = server::Options { game: dir.clone(), build: BUILD.into(), cache: dir.join("packs"), rate: Some(8_000_000) };
    let prepared = server::prepare(opts, &|_| {}).unwrap();
    let server = Server::start(&prepared, "127.0.0.1:0".parse().unwrap()).unwrap();
    let root = temp("follow-copy");
    let client = Client::start(Options {
        server: server.addr().to_string(),
        root: root.clone(),
        build: BUILD.into(),
        low_memory: false,
    });
    wait_ready(&client);
    let (ia, ib) = (client.find("data/x64/a.rpf").unwrap(), client.find("data/x64/b.rpf").unwrap());
    // The engine starts with b.rpf: the rest of it comes before a.rpf's.
    let mut buf = [0u8; 4096];
    assert_eq!(client.read(ib, 0, &mut buf), Some(4096));
    assert_eq!(&buf[..], &b[..4096]);
    let started = Instant::now();
    while !client.has(ib, 6 << 20) {
        assert!(started.elapsed() < Duration::from_secs(10), "b.rpf does not come");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!client.has(ia, 7 << 20), "a.rpf came first");
    client.stop();
    client.join();
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn downloads_the_whole_game() {
    let dir = temp("whole");
    game(&dir);
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    let root = temp("whole-copy");
    let client = Client::start(Options {
        server: server.addr().to_string(),
        root: root.clone(),
        build: BUILD.into(),
        low_memory: false,
    });
    wait_ready(&client);
    assert!(root.join(crate::cache::MARKER).exists() && !client.status().complete);
    client.set_fill(true);
    let started = Instant::now();
    while !client.status().complete {
        assert!(started.elapsed() < Duration::from_secs(20), "{:?}", client.status());
        std::thread::sleep(Duration::from_millis(20));
    }
    // A game folder like any other: every file whole, no marker.
    assert!(!root.join(crate::cache::MARKER).exists());
    for f in ["data/x64/big.rpf", "data/x64/later.rpf", "data/common/data/a.xml", "data/common/data/empty.meta"] {
        assert_eq!(fs::read(root.join(f)).unwrap(), fs::read(dir.join(f)).unwrap(), "{f}");
    }
    client.stop();
    client.join();
    drop(server);
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

/// The whole game is complete once the copy says so on disk (no marker):
/// until then, a partial copy.
#[cfg(unix)]
#[test]
fn completes_only_once_finished() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp("unfinished");
    game(&dir);
    // The web build's manifest, with the dates the engine is told.
    fs::write(dir.join("data/manifest.json"), br#"{"files":[["x64/big.rpf",3145851,1790644442]]}"#).unwrap();
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    let root = temp("unfinished-copy");
    let client = Client::start(Options {
        server: server.addr().to_string(),
        root: root.clone(),
        build: BUILD.into(),
        low_memory: false,
    });
    wait_ready(&client);
    // The files can be written, nothing beside them (the index, the marker).
    fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).unwrap();
    client.set_fill(true);
    let started = Instant::now();
    while client.status().cache_bytes < client.status().total_bytes {
        assert!(started.elapsed() < Duration::from_secs(20), "{:?}", client.status());
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(500));
    assert!(!client.status().complete && root.join(crate::cache::MARKER).exists());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    while !client.status().complete {
        assert!(started.elapsed() < Duration::from_secs(20), "{:?}", client.status());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!root.join(crate::cache::MARKER).exists());
    // A game folder with the dates streaming told the engine, played or
    // shared as one (its manifest no file of the game).
    let dates = crate::catalog::game_dates(&root.join("data"));
    assert_eq!(crate::catalog::date(&dates, "x64/big.rpf", 3145851), Some(1790644442));
    let shared = Catalog::scan(&root, BUILD).unwrap();
    let big = shared.find("data/x64/big.rpf").unwrap() as usize;
    assert_eq!(shared.dates[big], 1790644442);
    assert!(shared.find("data/manifest.json").is_none());
    client.stop();
    client.join();
    drop(server);
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

/// A server that takes no more requests holds up neither the app's status
/// (asked every frame) nor stopping the client.
#[test]
fn a_stuck_server_holds_nothing_up() {
    let dir = temp("stuck");
    fs::create_dir_all(dir.join("data")).unwrap();
    // 8 GB, sparse: 4 MB of requests.
    fs::File::create(dir.join("data/huge.rpf")).unwrap().set_len(8 << 30).unwrap();
    fs::create_dir_all(dir.join(format!("b/{BUILD}/shaders"))).unwrap();
    fs::write(dir.join(format!("b/{BUILD}/shaders/index.json")), b"{}").unwrap();
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    let proxy = Proxy::start(server.addr(), None);
    let root = temp("stuck-copy");
    let client = Client::start(Options { server: proxy.addr.to_string(), root: root.clone(), build: BUILD.into(), low_memory: false });
    wait_ready(&client);
    let id = client.find("data/huge.rpf").unwrap();
    proxy.up.store(false, Ordering::Release);
    let c = client.clone();
    std::thread::spawn(move || {
        for k in 0..64 {
            c.hint(id, k << 27, 1 << 27, true);
        }
    });
    std::thread::sleep(Duration::from_secs(1));
    let c = client.clone();
    assert!(promptly(move || assert!(c.status().connected)), "status() waits for a stuck write");
    let c = client.clone();
    assert!(promptly(move || c.stop()), "stop() waits for a stuck write");
    client.join();
    drop(proxy);
    drop(server);
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

/// A connection quiet for long, then slow to answer, is not taken for
/// dead: the silence counts from the request.
#[test]
fn a_quiet_connection_is_not_taken_for_dead() {
    let dir = temp("quiet");
    let data = noise(1 << 20, 19);
    fs::create_dir_all(dir.join("data")).unwrap();
    fs::write(dir.join("data/a.rpf"), &data).unwrap();
    fs::create_dir_all(dir.join(format!("b/{BUILD}/shaders"))).unwrap();
    fs::write(dir.join(format!("b/{BUILD}/shaders/index.json")), b"{}").unwrap();
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    let proxy = Proxy::start(server.addr(), None);
    let root = temp("quiet-copy");
    let client = Client::start(Options { server: proxy.addr.to_string(), root: root.clone(), build: BUILD.into(), low_memory: false });
    wait_ready(&client);
    let id = client.find("data/a.rpf").unwrap();
    std::thread::sleep(crate::client::SILENCE + Duration::from_millis(500));
    // The answer takes longer than the client's read timeout (2 s).
    proxy.down.store(false, Ordering::Release);
    let c = client.clone();
    let read = std::thread::spawn(move || {
        let mut buf = vec![0u8; 4096];
        c.read(id, 0, &mut buf).map(|n| buf[..n].to_vec())
    });
    std::thread::sleep(Duration::from_millis(2500));
    proxy.down.store(true, Ordering::Release);
    assert_eq!(read.join().unwrap().as_deref(), Some(&data[..4096]));
    assert_eq!(proxy.accepted.load(Ordering::Acquire), 1, "the connection was dropped");
    client.stop();
    client.join();
    drop(proxy);
    drop(server);
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

/// A chunk whose header was damaged on the way (it has no checksum of its
/// own) is not stored where the header says, outside its request.
#[test]
fn keeps_no_chunk_outside_its_request() {
    let dir = temp("misplaced");
    let data = noise(1 << 20, 23);
    fs::create_dir_all(dir.join("data")).unwrap();
    fs::write(dir.join("data/a.rpf"), &data).unwrap();
    fs::create_dir_all(dir.join(format!("b/{BUILD}/shaders"))).unwrap();
    fs::write(dir.join(format!("b/{BUILD}/shaders/index.json")), b"{}").unwrap();
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    // The first chunk of a.rpf comes with another offset.
    let once = AtomicBool::new(false);
    let tamper: Tamper = Arc::new(move |h: &mut Header| {
        if h.kind == DATA && h.file == 0 && h.off == 0 && !once.swap(true, Ordering::AcqRel) {
            h.off = 128 << 10;
        }
    });
    let proxy = Proxy::start(server.addr(), Some(tamper));
    let root = temp("misplaced-copy");
    let client = Client::start(Options { server: proxy.addr.to_string(), root: root.clone(), build: BUILD.into(), low_memory: false });
    wait_ready(&client);
    assert_eq!(client.find("data/a.rpf"), Some(0));
    let mut buf = vec![0u8; 4096];
    assert_eq!(client.read(0, 0, &mut buf), Some(4096));
    assert_eq!(&buf[..], &data[..4096]);
    assert_eq!(client.read(0, 128 << 10, &mut buf), Some(4096));
    assert_eq!(&buf[..], &data[128 << 10..][..4096]);
    client.stop();
    client.join();
    drop(proxy);
    drop(server);
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

/// A broken server (sizes no blob has, a catalog that unpacks to more than
/// any game's) is refused, not obeyed.
#[test]
fn refuses_what_no_server_sends() {
    let hello = |blobs: serde_json::Value| {
        serde_json::json!({ "protocol": crate::protocol::PROTOCOL, "build": BUILD, "catalog": "x", "blobs": blobs, "packs": {} })
    };
    let root = temp("broken-copy");
    let opts = |addr: SocketAddr| Options { server: addr.to_string(), root: root.clone(), build: BUILD.into(), low_memory: false };
    let client = Client::start(opts(fake_server(hello(serde_json::json!({ "catalog": u64::MAX / 2 })), vec![])));
    wait_error(&client, "bytes");
    client.stop();
    client.join();
    let bomb = zstd::encode_all(&vec![b' '; 100 << 20][..], 3).unwrap();
    let addr = fake_server(hello(serde_json::json!({ "catalog": bomb.len() })), vec![("catalog", bomb)]);
    let client = Client::start(opts(addr));
    wait_error(&client, "more than");
    client.stop();
    client.join();
    let _ = fs::remove_dir_all(&root);
}

/// A stopped client let go of has closed its copy (files, lock) by the time
/// work on its folder starts.
#[test]
fn a_stopped_client_closes_its_copy_before_it_has_finished() {
    let dir = temp("closes");
    game(&dir);
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    let root = temp("closes-copy");
    let client = Client::start(Options { server: server.addr().to_string(), root: root.clone(), build: BUILD.into(), low_memory: false });
    wait_ready(&client);
    let mut buf = vec![0u8; 100];
    assert_eq!(client.read(client.find("data/x64/big.rpf").unwrap(), 0, &mut buf), Some(100));
    client.stop();
    drop(client);
    // Another open of the lock file can take it only once the copy's is closed.
    let (tx, rx) = std::sync::mpsc::channel();
    let lock = root.join(".glue5-lock");
    crate::after_stopped(move || {
        let taken = fs::File::open(&lock).and_then(|f| f.try_lock().map_err(std::io::Error::from));
        let _ = tx.send(taken.map_err(|e| e.to_string()));
    });
    assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), Ok(()));
    drop(server);
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

/// A stopped client finishes in the background, a sync it was in too; what
/// touches its folder after it (work on it, the next client) waits for it.
#[test]
fn a_stopped_client_finishes_before_its_folder_is_touched() {
    let dir = temp("handover");
    game(&dir);
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    // The catalog's first chunk waits: the client is in its sync.
    let hold = Arc::new(AtomicBool::new(true));
    let held = hold.clone();
    let tamper: Tamper = Arc::new(move |h: &mut Header| {
        while h.kind == DATA && h.file == u32::MAX && held.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let proxy = Proxy::start(server.addr(), Some(tamper));
    let root = temp("handover-copy");
    let opts = |server: SocketAddr| Options { server: server.to_string(), root: root.clone(), build: BUILD.into(), low_memory: false };
    let a = Client::start(opts(proxy.addr));
    std::thread::sleep(Duration::from_millis(300));
    a.stop();
    let c = a.clone();
    assert!(promptly(move || c.join()), "a stopped client goes on with its sync");
    hold.store(false, Ordering::Release);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (c, s) = (a.clone(), seen.clone());
    crate::after_stopped(move || s.lock().unwrap().push(c.finished()));
    let b = Client::start(opts(server.addr()));
    wait_ready(&b);
    assert_eq!(*seen.lock().unwrap(), [true]);
    assert!(!a.is_ready());
    b.stop();
    b.join();
    drop(proxy);
    drop(server);
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

/// The files under `root`'s `data` and `b`, with their sizes.
fn files(root: &Path) -> Vec<(PathBuf, u64)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, u64)>) {
        for e in fs::read_dir(dir).into_iter().flatten().flatten() {
            let m = e.metadata().unwrap();
            if m.is_dir() {
                walk(&e.path(), out);
            } else {
                out.push((e.path(), m.len()));
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join("data"), &mut out);
    walk(&root.join("b"), &mut out);
    out.sort();
    out
}

/// The whole game, downloaded, stays as it is when the server's files
/// change (one of another size, one gone): same files, same sizes, no
/// marker, complete, played with its own catalog.
#[test]
fn keeps_the_whole_game_as_it_is() {
    let dir = temp("kept");
    game(&dir);
    let root = temp("kept-copy");
    let opts = |server: SocketAddr| Options { server: server.to_string(), root: root.clone(), build: BUILD.into(), low_memory: false };
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    let client = Client::start(opts(server.addr()));
    wait_ready(&client);
    client.set_fill(true);
    let started = Instant::now();
    while !client.status().complete {
        assert!(started.elapsed() < Duration::from_secs(20), "{:?}", client.status());
        std::thread::sleep(Duration::from_millis(20));
    }
    client.stop();
    client.join();
    drop(server);
    let whole = files(&root);
    fs::write(dir.join("data/x64/later.rpf"), noise(1 << 19, 5)).unwrap();
    fs::remove_file(dir.join("data/common/data/a.xml")).unwrap();
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    let client = Client::start(opts(server.addr()));
    wait_ready(&client);
    assert!(client.status().complete && !root.join(crate::cache::MARKER).exists());
    assert_eq!(files(&root), whole);
    let id = client.find("data/x64/later.rpf").unwrap();
    let mut buf = vec![0u8; 4096];
    assert_eq!(client.read(id, 0, &mut buf), Some(4096));
    assert_eq!(&buf[..], &noise(1 << 20, 9)[..4096]);
    client.stop();
    client.join();
    drop(server);
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn checks_a_server_before_it_is_chosen() {
    let dir = temp("check");
    game(&dir);
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    let addr = server.addr().to_string();
    assert!(matches!(crate::check(&addr, BUILD), crate::Check::Ready { .. }));
    assert!(matches!(crate::check(&addr, "otherbuild"), crate::Check::Unusable(why) if why.contains("otherbuild")));
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().to_string();
    assert!(matches!(crate::check(&closed, BUILD), crate::Check::Unreachable(_)));
    drop(server);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn prepares_side_by_side() {
    let dir = temp("prepare");
    game(&dir);
    let cache = dir.join("packs");
    // The app sharing again before its first preparation was done.
    let jobs: Vec<_> = (0..2)
        .map(|_| {
            let opts = server::Options { game: dir.clone(), build: BUILD.into(), cache: cache.clone(), rate: None };
            std::thread::spawn(move || server::prepare(opts, &|_| {}).map(|_| ()))
        })
        .collect();
    for j in jobs {
        j.join().unwrap().unwrap();
    }
    let mut names: Vec<String> = fs::read_dir(&cache).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into()).collect();
    names.sort();
    assert_eq!(names.len(), 2, "{names:?}");
    for name in names {
        assert!(name.ends_with(".zst"), "{name}");
        crate::pack::unpack(fs::File::open(cache.join(&name)).unwrap(), |_, _| Ok(())).unwrap();
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn leaves_the_files_of_a_build_going_on() {
    let dir = temp("leftovers");
    game(&dir);
    let cache = dir.join("packs");
    fs::create_dir_all(&cache).unwrap();
    let put = |name: &str, age: Duration| {
        let f = fs::File::create(cache.join(name)).unwrap();
        f.set_modified(std::time::SystemTime::now() - age).unwrap();
    };
    put("text-0123456789abcdef.4242-0.tmp", Duration::ZERO);
    put("shaders-0123456789abcdef.4242-1.tmp", Duration::from_secs(7200));
    put("text-fedcba9876543210.zst", Duration::from_secs(60));
    let opts = server::Options { game: dir.clone(), build: BUILD.into(), cache: cache.clone(), rate: None };
    server::prepare(opts, &|_| {}).unwrap();
    // Another server's build, going on: left alone; one that died, and an
    // older pack: gone.
    assert!(cache.join("text-0123456789abcdef.4242-0.tmp").exists());
    assert!(!cache.join("shaders-0123456789abcdef.4242-1.tmp").exists());
    assert!(!cache.join("text-fedcba9876543210.zst").exists());
    let _ = fs::remove_dir_all(&dir);
}

/// A request past a file's end (even past what 64 bits hold) gets what
/// there is of it, and its end.
#[test]
fn answers_a_read_past_the_end() {
    use crate::protocol::*;
    use std::io::Write;
    let dir = temp("past");
    game(&dir);
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    let mut s = std::net::TcpStream::connect(server.addr()).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let hello = Header::read(&mut s).unwrap();
    payload(&mut s, hello.len, MAX_MESSAGE).unwrap();
    let read = Header { kind: READ, prio: DEMAND, req: 1, off: u64::MAX - 100, len: 4096, file: 0, crc: 0 };
    s.write_all(&read.encode()).unwrap();
    let end = Header::read(&mut s).unwrap();
    assert_eq!((end.kind, end.req, end.len), (END, 1, SHORT));
    drop(s);
    let started = Instant::now();
    while server.connections() > 0 {
        assert!(started.elapsed() < Duration::from_secs(5), "the connection stays open");
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(server);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn keeps_no_connection_its_client_closed() {
    let dir = temp("closed");
    game(&dir);
    let server = serve(&dir, &dir.join("packs"), "127.0.0.1:0".parse().unwrap());
    let addr = server.addr().to_string();
    // A phone's Servers page checks every few seconds.
    for _ in 0..20 {
        assert!(matches!(crate::check(&addr, BUILD), crate::Check::Ready { .. }));
    }
    let started = Instant::now();
    while server.connections() > 0 {
        assert!(started.elapsed() < Duration::from_secs(5), "{} connections still open", server.connections());
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(server);
    let _ = fs::remove_dir_all(&dir);
}
