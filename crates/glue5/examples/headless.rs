//! The game without a window, for tests and measurements: the renderer
//! draws offscreen, input comes from a script.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::Parser;
use glue5_runtime::Game;
use glue5_runtime::audio::{CHANNELS, SAMPLE_RATE};
use glue5_runtime::input::{self, Gamepad, button};

/// The app's allocator (see src/main.rs), so measurements match it.
#[cfg(windows)]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// The game without a window, for tests and measurements.
///
/// The run ends after --for, when the game exits or asks to restart (its
/// menu), or, with status 1, when it stops working (an engine thread's
/// trap). Presses hold for 0.1 s (menus repeat a key held longer).
#[derive(Parser)]
struct Args {
    /// End after S seconds.
    #[arg(long = "for", value_name = "S")]
    run_for: Option<f64>,
    /// Press a pad button at S seconds: start, select, south or a, east or
    /// b, west or x, north or y, l1, r1, l2, r2, l3, r3, up, down, left,
    /// right.
    #[arg(long, num_args = 2, value_names = ["S", "BUTTON"])]
    press: Vec<String>,
    /// Press a key at S seconds: esc, enter, tab, backspace, space, grave,
    /// shift, up, down, left, right, a letter or a digit, or another key
    /// by the mods' name for it (f5, leftshift, keypad8).
    #[arg(long, num_args = 2, value_names = ["S", "KEY"])]
    key: Vec<String>,
    /// Also take commands from standard input, one per line: press
    /// <button>, key <key>, hold <key> <s>, text <text>, size <w>x<h> (the
    /// back buffer, as a window resize).
    #[arg(long)]
    stdin: bool,
    /// Every N-th presented frame to a PNG in DIR.
    #[arg(long, num_args = 2, value_names = ["N", "DIR"])]
    shots: Vec<String>,
    /// The back buffer's size.
    #[arg(long, value_name = "WxH", default_value = "1280x720", value_parser = parse_size)]
    size: (u32, u32),
    /// Present the frames as into a window of this size, scaled by
    /// --upscaler (with --shots, the presented frames too: outNNNNNN.png).
    #[arg(long, value_name = "WxH", value_parser = parse_size)]
    output: Option<(u32, u32)>,
    /// With --output: bicubic, metalfx, metalfx-temporal or fsr.
    #[arg(long, value_name = "NAME", default_value = "bicubic", value_parser = parse_upscaler)]
    upscaler: glue5_gpu::Upscaler,
    /// Frames per second at most, as the app's setting (the engine's
    /// limiter; 0: as fast as it can).
    #[arg(long, default_value_t = 0)]
    fps: u32,
    /// The low-memory profile (smaller pools, its graphics preset written
    /// into the game's settings).
    #[arg(long)]
    low: bool,
    /// A graphics setting written into the game's settings (settings.xml
    /// names, e.g. ShadowQuality=3), after --low.
    #[arg(long = "setting", value_name = "NAME=V", value_parser = parse_setting)]
    settings: Vec<(String, String)>,
    /// Start in the sandbox (the map, no story) instead of the story mode.
    #[arg(long)]
    sandbox: bool,
    /// GTA Online, as this player name.
    #[arg(long, value_name = "NAME")]
    online: Option<String>,
    /// With --online: the lobby to start in instead of everyone's (until
    /// the game hosts or looks for a session).
    #[arg(long, value_name = "NAME", requires = "online", default_value = "")]
    lobby: String,
    /// With --online: the story, GTA Online a choice of its pause menu away
    /// (as the app's story mode), instead of straight into freemode.
    #[arg(long, requires = "online")]
    story: bool,
    /// Play as the touch controls do (a phone's): the engine on its pad
    /// path, aim assist Full (the Targeting Mode touch play forces).
    #[arg(long)]
    touch: bool,
    /// The game's benchmark (a fly-through of the map; the run ends with
    /// it, results in the user directory).
    #[arg(long)]
    benchmark: bool,
    /// Visit the map: S seconds at each stop of FILE (lines `x y z hour
    /// weather [# name]`, z `ground`: on the ground there), the camera
    /// turning; the run ends after the last (use with --sandbox).
    #[arg(long, num_args = 2, value_names = ["FILE", "S"])]
    tour: Vec<String>,
    /// Run the mods of this folder (glue5-mods; those marked autostart
    /// start with the game, stdin's `mods start <name>` the others).
    #[arg(long, value_name = "DIR")]
    mods: Option<PathBuf>,
    /// With --mods: put the mods that come with GLUE 5 (Glue Trainer) in
    /// the folder first, as the app does.
    #[arg(long, requires = "mods")]
    bundled: bool,
    /// With --mods: serve GLUE 5 Studio and the MCP endpoint on this port,
    /// for this device only.
    #[arg(long, value_name = "PORT", requires = "mods")]
    studio: Option<u16>,
    /// With --studio: its pairing code (8 characters; default: a new one).
    #[arg(long, value_name = "CODE", requires = "studio")]
    studio_code: Option<String>,
    /// Saves, settings and the pipeline cache (default: glue5-headless in
    /// the temporary directory).
    #[arg(long, value_name = "DIR")]
    user_dir: Option<PathBuf>,
    /// Play the sound into a pretend device, as the app does (without one
    /// the engine mixes on its own clock).
    #[arg(long)]
    audio: bool,
    /// Compile pipelines when first drawn, not skipping the draws meanwhile
    /// (shots comparable between runs).
    #[arg(long)]
    sync_pipelines: bool,
    /// No rendering at all.
    #[arg(long)]
    no_gpu: bool,
    /// The game directory (not with --server).
    #[arg(long, value_name = "DIR", env = "GLUE5_GAME_DIR", required_unless_present = "server")]
    game: Option<PathBuf>,
    /// Stream the game from a server (host or host:port, see glue5-serve)
    /// instead of --game; the copy is kept in the user directory's
    /// `stream`, and is used again by the next run.
    #[arg(long, value_name = "ADDR")]
    server: Option<String>,
}

/// Ends the run: the stream's index saved first (the next run starts from
/// what this one received).
fn quit(stream: Option<&glue5_stream::Client>, code: i32) -> ! {
    // The last shots are still being encoded.
    if !glue5_gpu::screenshots_written(std::time::Duration::from_secs(60)) {
        log::warn!("screenshots: some were not written before the end");
    }
    if let Some(client) = stream {
        client.flush();
    }
    // The profile, if the game was built instrumented: the process ends
    // without the exit handlers.
    glue5::write_profile();
    glue5_runtime::exit::process(code)
}

fn parse_size(size: &str) -> Result<(u32, u32)> {
    let (w, h) = size.split_once('x').context("<width>x<height>")?;
    Ok((w.parse()?, h.parse()?))
}

fn parse_upscaler(name: &str) -> Result<glue5_gpu::Upscaler> {
    Ok(match name {
        "bicubic" => glue5_gpu::Upscaler::Bicubic,
        "metalfx" => glue5_gpu::Upscaler::MetalFx,
        "metalfx-temporal" => glue5_gpu::Upscaler::MetalFxTemporal,
        "fsr" => glue5_gpu::Upscaler::Fsr,
        _ => bail!("unknown upscaler {name}"),
    })
}

fn parse_setting(setting: &str) -> Result<(String, String)> {
    let (name, value) = setting.split_once('=').context("<name>=<value>")?;
    Ok((name.into(), value.into()))
}

/// How long a press holds its button or key, in seconds.
const HOLD: f64 = 0.1;

/// A key press: when (s), the key, whether it is down now, for how long (s).
type Key = (f64, u8, bool, f64);

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();
    let mut presses = Vec::new();
    for p in args.press.chunks(2) {
        presses.push((p[0].parse::<f64>()?, parse_button(&p[1])?));
    }
    let mut keys = Vec::new();
    for k in args.key.chunks(2) {
        keys.push((k[0].parse::<f64>()?, parse_key(&k[1])?, false, HOLD));
    }
    let shots = match &args.shots[..] {
        [every, dir] => Some((every.parse::<u32>()?, PathBuf::from(dir))),
        _ => None,
    };
    let tour = match &args.tour[..] {
        [path, stay] => {
            let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
            Some((glue5_runtime::tour::parse(&text)?, Duration::from_secs_f64(stay.parse()?)))
        }
        _ => None,
    };
    let user_dir = args.user_dir.unwrap_or_else(|| std::env::temp_dir().join("glue5-headless"));
    let (mut game_dir, low) = (args.game.unwrap_or_default(), args.low);
    let stream = match args.server {
        Some(server) => {
            let client = glue5_stream::Client::start(glue5_stream::Options {
                server,
                root: user_dir.join("stream"),
                build: glue5_runtime::GAME_BUILD.into(),
                low_memory: low,
            });
            let started = Instant::now();
            while !client.is_ready() {
                let status = client.status();
                if let glue5_stream::Phase::Failed(why) = status.phase {
                    bail!("{why}");
                }
                log::info!("stream: {:?}{}", status.phase, status.error.map(|e| format!(" ({e})")).unwrap_or_default());
                std::thread::sleep(Duration::from_secs(1));
            }
            log::info!("stream: ready in {:.1?}", started.elapsed());
            game_dir = client.root().to_owned();
            Some(client)
        }
        None => None,
    };
    if low {
        glue5_runtime::game_settings::apply_low_preset(&user_dir)?;
    }
    if !args.settings.is_empty() {
        glue5_runtime::game_settings::apply(&user_dir, &args.settings)?;
    }
    let gpu = if args.no_gpu {
        glue5_runtime::gpu::null()
    } else {
        let mut settings = glue5_gpu::Settings::for_game(&game_dir);
        settings.cache_dir = Some(user_dir.join("cache"));
        settings.sync_pipelines = args.sync_pipelines;
        if let Some((every, dir)) = &shots {
            std::fs::create_dir_all(dir)?;
            settings.shot_every = *every;
            settings.shot_dir = Some(dir.clone());
        }
        let screen = match args.output {
            Some(size) => {
                let presentation = glue5_gpu::Presentation::new(size, false, args.upscaler);
                Some(glue5_gpu::Screen::offscreen(presentation, shots.clone())?)
            }
            None => None,
        };
        glue5_gpu::backend(settings, screen)
    };
    // The camera turns all along the tour, as fast as the stick turns it.
    let spin = if tour.is_some() { [1.0, 0.0] } else { [0.0; 2] };
    if let Some((stops, stay)) = tour {
        log::info!("tour: {} stops, {} s each", stops.len(), stay.as_secs_f64());
        glue5_runtime::tour::start(stops, stay);
    }
    if let Some(name) = &args.online {
        glue5_runtime::friends::start(&user_dir, name);
    }
    if let Some(dir) = &args.mods {
        if args.bundled {
            glue5_mods::bundled::install(dir);
        }
        glue5_mods::install(dir);
    }
    let _studio = match args.studio {
        Some(port) => {
            let studio = glue5_mods::studio::Studio::start(port, false, args.studio_code.as_deref().unwrap_or(""), &glue5::device_name())?;
            let code = studio.pairing_code();
            log::info!("studio: {}?pair={code} (code {code}; control page {})", studio.url("127.0.0.1"), studio.control_url("127.0.0.1"));
            Some(studio)
        }
        None => None,
    };
    if args.touch {
        glue5_runtime::input::set_touch_play(true);
        glue5_runtime::input::set_touch_targeting(glue5_runtime::input::Targeting::Full);
    }
    let game = glue5_runtime::start(
        glue5_runtime::Config {
            game_dir,
            stream: stream.clone(),
            user_dir,
            width: args.size.0,
            height: args.size.1,
            start_mode: if args.sandbox { 2 } else { 1 },
            low_memory: low,
            max_fps: args.fps,
            benchmark: args.benchmark,
            online: args.online.map(|name| glue5_runtime::Online { name, lobby: args.lobby, freemode: !args.story }),
            language: None,
        },
        gpu,
    )?;
    if args.audio {
        let out = game.audio();
        std::thread::spawn(move || {
            // The app's sound device: 256 frames at a time.
            const PERIOD: usize = 256;
            let mut buf = [0f32; PERIOD * CHANNELS];
            let (start, mut frames) = (Instant::now(), 0);
            loop {
                frames += PERIOD;
                let due = start + Duration::from_secs_f64(frames as f64 / SAMPLE_RATE as f64);
                std::thread::sleep(due.saturating_duration_since(Instant::now()));
                out.read(&mut buf);
            }
        });
    }
    let (live_tx, live) = std::sync::mpsc::channel::<String>();
    if args.stdin {
        std::thread::spawn(move || {
            for line in std::io::stdin().lines().map_while(Result::ok) {
                let _ = live_tx.send(line);
            }
        });
    }
    let t0 = Instant::now();
    let mut focused = false;
    loop {
        let t = t0.elapsed().as_secs_f64();
        for line in live.try_iter() {
            if let Err(e) = command(&line, t, &game, &mut presses, &mut keys) {
                log::warn!("{line}: {e}");
            }
        }
        if !focused && let Some(input) = game.input() {
            input.focus(true); // without focus the engine ignores input
            focused = true;
        }
        for notice in glue5_runtime::friends::take_notices() {
            log::info!("friends: notice {notice:?}");
        }
        if let Some(input) = game.input() {
            for (at, vk, down, hold) in &mut keys {
                let held = (*at..*at + *hold).contains(&t);
                if held != *down {
                    input.key(*vk, held);
                    // As the app reports them (no-op without mods).
                    glue5_mods::input::key(*vk, held);
                    *down = held;
                }
            }
        }
        let buttons = presses.iter().filter(|(at, _)| (*at..at + HOLD).contains(&t)).fold(0, |b, (_, bits)| b | bits);
        input::set_gamepad(0, Gamepad { connected: true, buttons, right: spin, ..Default::default() });
        if args.run_for.is_some_and(|limit| t >= limit) {
            quit(stream.as_deref(), 0);
        }
        if let Some(exit) = glue5_runtime::exit::requested() {
            log::info!("the engine asked for {exit:?} at {t:.1} s");
            quit(stream.as_deref(), 0);
        }
        if glue5_runtime::tour::finished() {
            log::info!("tour: done at {t:.1} s");
            quit(stream.as_deref(), 0);
        }
        if glue5_runtime::exit::failed().is_some() {
            quit(stream.as_deref(), 1);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A line from standard input, at `t` seconds.
fn command(line: &str, t: f64, game: &Game, presses: &mut Vec<(f64, u16)>, keys: &mut Vec<Key>) -> Result<()> {
    match line.split_whitespace().collect::<Vec<_>>()[..] {
        ["press", name] => presses.push((t, parse_button(name)?)),
        ["key", name] => keys.push((t, parse_key(name)?, false, HOLD)),
        ["hold", name, secs] => keys.push((t, parse_key(name)?, false, secs.parse()?)),
        ["text", ..] => game.input().context("no input yet")?.text(line.trim_start()["text".len()..].trim()),
        ["size", size] => {
            let (w, h) = size.split_once('x').context("size <width>x<height>")?;
            game.input().context("no input yet")?.resize(w.parse()?, h.parse()?);
        }
        // The window's focus, as the app gives it (with the game's "pause on
        // focus loss" the engine pauses without it).
        ["focus", on @ ("on" | "off")] => game.input().context("no input yet")?.focus(on == "on"),
        // The system's low-memory event, as the app gets it.
        ["low-memory"] => glue5_gpu::trim(),
        ["friends", ..] => friends(&line.split_whitespace().collect::<Vec<_>>()[1..])?,
        ["mods", "list"] => log::info!("mods: {:?}", glue5_mods::mods()),
        ["mods", "start", name] => glue5_mods::start(name),
        ["mods", "stop", name] => glue5_mods::stop(name),
        ["mods", "menus"] => log::info!("mods: menus {:?}", glue5_mods::menus()),
        ["mods", "input", module, item, what, ref rest @ ..] => {
            use glue5_mods::MenuInput;
            let input = match (what, rest) {
                ("press", []) => MenuInput::Press,
                ("toggle", [v]) => MenuInput::Toggle(*v == "on"),
                ("value", [v]) => MenuInput::Number(v.parse()?),
                ("choose", [v]) => MenuInput::Choose(v.parse()?),
                ("text", [v]) => MenuInput::Text(v.to_string()),
                _ => bail!("mods input <mod> <item> press|toggle on/off|value <n>|choose <i>|text <t>"),
            };
            glue5_mods::menu_input(module, item.parse()?, input).map_err(anyhow::Error::msg)?;
        }
        _ => bail!(
            "expected `press <button>`, `key <key>`, `hold <key> <s>`, `text <text>`, `size <w>x<h>` or \
             `friends code|ask <code>|accept|list|join|invite|play|add <name>`, `focus on|off`, `low-memory` or \
             `mods list|start|stop|menus|input`"
        ),
    }
    Ok(())
}

/// The friends, as the application's page drives them.
fn friends(words: &[&str]) -> Result<()> {
    use glue5_runtime::friends;
    match words {
        ["code"] => log::info!("friends: code {}", friends::my_code().context("no friends")?),
        ["ask", code] => friends::ask(code).map_err(|e| anyhow::anyhow!("{e:?}"))?,
        ["accept"] => friends::snapshot().requests.iter().for_each(|(key, _)| friends::answer(key, true)),
        ["list"] => log::info!("friends: {:?}", friends::snapshot()),
        ["join", name] => {
            let f = friends::snapshot().friends.into_iter().find(|f| f.name == *name).context("no such friend")?;
            friends::follow(&f.key);
        }
        ["invite", name] => {
            let f = friends::snapshot().friends.into_iter().find(|f| f.name == *name).context("no such friend")?;
            friends::invite(&f.key);
        }
        // A friend's invite accepted, or its session joined, from the game.
        ["play", name] => {
            let f = friends::snapshot().friends.into_iter().find(|f| f.name == *name).context("no such friend")?;
            friends::join(&f.key);
        }
        // A player met in a session asked.
        ["add", name] => {
            let r = friends::snapshot().recent.into_iter().find(|r| r.name == *name).context("no such player")?;
            friends::ask_player(&r.key);
        }
        _ => bail!("friends code|ask <code>|accept|list|join <name>|invite <name>|play <name>|add <name>"),
    }
    Ok(())
}

fn parse_button(name: &str) -> Result<u16> {
    Ok(match name {
        "start" => button::START,
        "select" => button::SELECT,
        "south" | "a" | "cross" => button::SOUTH,
        "east" | "b" | "circle" => button::EAST,
        "west" | "x" | "square" => button::WEST,
        "north" | "y" | "triangle" => button::NORTH,
        "l1" => button::L1,
        "r1" => button::R1,
        "l2" => button::L2,
        "r2" => button::R2,
        "l3" => button::L3,
        "r3" => button::R3,
        "up" => button::UP,
        "down" => button::DOWN,
        "left" => button::LEFT,
        "right" => button::RIGHT,
        other => bail!("unknown button {other}"),
    })
}

/// Windows virtual-key codes.
fn parse_key(name: &str) -> Result<u8> {
    Ok(match name {
        "esc" => 0x1B,
        "enter" => 0x0D,
        "tab" => 0x09,
        "backspace" => 0x08,
        "space" => 0x20,
        "grave" => 0xC0,
        "shift" => 0x10,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => s.as_bytes()[0].to_ascii_uppercase(),
        // The mods' names for the others (f5, leftshift, keypad8).
        other => glue5_mods::input::vk(other).with_context(|| format!("unknown key {other}"))?,
    })
}
