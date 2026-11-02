//! The start page's scenes: one per mode, layers of PNGs (a background, the
//! characters with their transparency) from the player's own folder, drifting
//! at different speeds. Nothing is embedded: without the folder the page
//! keeps its drawn backdrop. The images are decoded off the UI thread, at
//! most three scenes are held, and all are dropped when the game starts.
use std::collections::HashMap;
use std::f64::consts::TAU;
use std::time::{Duration, Instant};
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::{Color32, ColorImage, Context, Painter, Rect, TextureHandle, TextureOptions, Vec2, pos2, vec2};
use serde::Deserialize;

use crate::paths;

/// The folders a scene may be in. They are not tied to the game's mode: the
/// start page turns from one to the next on its own clock.
pub(super) const KEYS: [&str; 4] = ["story", "online", "sandbox", "benchmark"];

/// How long a scene stays before the next, unless `scenes.json` says otherwise.
const ROTATE_SECS: f64 = 9.0;

/// Keep user-provided artwork within the resource bounds used by the former
/// scene loader. At most three held scenes each have a 32 MiB decoded budget.
const MAX_SIDE: usize = 2048;
const MAX_PIXELS: usize = 16_000_000;
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_SCENE_BYTES: usize = 32 * 1024 * 1024;
const MAX_LAYERS: usize = 6;
/// A scene fades over the one before, and in from nothing.
const FADE_SECS: f32 = 0.30;
const APPEAR_SECS: f32 = 0.45;
/// Stronger default drift makes the separated scene layers read as parallax;
/// a scenes.json manifest can still tune or disable it.
const DEFAULT_MOTION_AMP: [f32; 2] = [32.0, 9.0];
const DEFAULT_MOTION_PERIOD: [f32; 2] = [16.0, 23.0];

// --- What the folder says --------------------------------------------------

/// `scenes.json`, all of it optional: without it, the layers are the PNGs of
/// each scene's folder in the order of their names.
#[derive(Deserialize, Default, Debug)]
struct Manifest {
    /// How far the nearest layer drifts, in points, and how long a swing takes.
    #[serde(default)]
    amp: Option<[f32; 2]>,
    #[serde(default)]
    period: Option<[f32; 2]>,
    #[serde(default)]
    rotate: Option<Rotate>,
    #[serde(default)]
    scenes: HashMap<String, SceneSpec>,
}

/// How the scenes take turns: every `seconds`, in `order` (by folder name).
/// Without it, every nine seconds, in the order of `KEYS`.
#[derive(Deserialize, Default, Debug)]
struct Rotate {
    #[serde(default)]
    seconds: Option<f32>,
    #[serde(default)]
    order: Option<Vec<String>>,
}

#[derive(Deserialize, Default, Debug)]
struct SceneSpec {
    #[serde(default)]
    layers: Vec<LayerSpec>,
    /// Which side the text panel goes ("left" or "right", the default).
    #[serde(default)]
    panel_side: Option<String>,
    /// The point of a background that is kept when the screen is narrower
    /// (0..1 across and down).
    #[serde(default)]
    focus: Option<[f32; 2]>,
    /// A colour grade: each channel multiplies the scene's.
    #[serde(default)]
    tint: Option<[f32; 3]>,
}

/// A layer without `height` covers the screen; with it, it is a figure:
/// `height` of the screen's, centred at `x` of its width, its foot at
/// `bottom` of its height.
#[derive(Deserialize, Debug)]
struct LayerSpec {
    file: String,
    #[serde(default)]
    depth: Option<f32>,
    #[serde(default)]
    overscan: Option<f32>,
    #[serde(default)]
    height: Option<f32>,
    #[serde(default)]
    x: Option<f32>,
    #[serde(default)]
    bottom: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Place {
    Cover { overscan: f32 },
    Sprite { height: f32, x: f32, bottom: f32 },
}

#[derive(Clone, Debug)]
struct LayerPlan {
    path: PathBuf,
    depth: f32,
    place: Place,
}

#[derive(Clone, Debug)]
struct Plan {
    layers: Vec<LayerPlan>,
    panel_right: bool,
    focus: Vec2,
    tint: [f32; 3],
}

/// Nearer layers move more: the first (the background) the least.
fn default_depth(i: usize, n: usize) -> f32 {
    if n <= 1 { 0.3 } else { 0.2 + 0.8 * i as f32 / (n - 1) as f32 }
}

/// What to load for a scene: from the manifest, else from its folder.
fn plan(dir: &Path, key: &str, manifest: &Manifest) -> Option<Plan> {
    let spec = manifest.scenes.get(key).filter(|s| !s.layers.is_empty());
    if spec.is_some_and(|scene| scene.layers.len() > MAX_LAYERS) {
        log::warn!("scenes: {key} has more than {MAX_LAYERS} layers; leaving it out");
        return None;
    }
    let panel_right = !matches!(
        spec.and_then(|s| s.panel_side.as_deref()),
        Some(side) if side.eq_ignore_ascii_case("left")
    );
    // The shell's panel occupies the left 62%. Figures stay in the free
    // right-hand area even if an older manifest asks for the other side.
    let figure_x = 0.74;
    let layers: Vec<LayerPlan> = match spec {
        Some(spec) => {
            let n = spec.layers.len();
            spec.layers
                .iter()
                .enumerate()
                .filter_map(|(i, l)| {
                    let relative = Path::new(&l.file);
                    // Only names inside the folder; canonicalization below
                    // also rejects a symlink that points outside it.
                    if !relative.components().all(|c| matches!(c, Component::Normal(_)))
                        || !relative.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
                    {
                        log::warn!("scenes: {} is not a relative PNG inside the folder", l.file);
                        return None;
                    }
                    let place = match l.height {
                        Some(height) => Place::Sprite {
                            height: height.clamp(0.1, 1.5),
                            x: l.x.unwrap_or(figure_x).clamp(-0.5, 1.5),
                            bottom: l.bottom.unwrap_or(1.04).clamp(0.2, 1.6),
                        },
                        None => Place::Cover { overscan: l.overscan.unwrap_or(0.05).clamp(0.0, 0.3) },
                    };
                    Some(LayerPlan {
                        path: dir.join(relative),
                        depth: l.depth.unwrap_or_else(|| default_depth(i, n)).clamp(0.0, 1.5),
                        place,
                    })
                })
                .collect()
        }
        None => {
            let mut files: Vec<PathBuf> = std::fs::read_dir(dir.join(key))
                .ok()?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("png")))
                .take(MAX_LAYERS + 1)
                .collect();
            files.sort();
            let n = files.len();
            files
                .into_iter()
                .enumerate()
                .map(|(i, path)| LayerPlan {
                    path,
                    depth: default_depth(i, n),
                    place: if i == 0 {
                        Place::Cover { overscan: 0.06 }
                    } else {
                        Place::Sprite { height: 0.95, x: figure_x, bottom: 1.04 }
                    },
                })
                .collect()
        }
    };
    if layers.is_empty() {
        return None;
    }
    if layers.len() > MAX_LAYERS {
        log::warn!("scenes: {key} has more than {MAX_LAYERS} layers; leaving it out");
        return None;
    }
    let focus = spec.and_then(|s| s.focus).map_or(vec2(0.5, 0.5), |[x, y]| vec2(x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)));
    let tint = spec.and_then(|s| s.tint).unwrap_or([1.0; 3]);
    Some(Plan { layers, panel_right, focus, tint })
}

/// Where the scenes may be, best first: in the game's folder, beside it, one
/// up, then the app's own.
fn candidates(game_dir: Option<&Path>, user_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(game) = game_dir {
        dirs.push(game.join(paths::SCENES_FOLDER));
        if let Some(parent) = game.parent() {
            dirs.push(parent.join(paths::SCENES_FOLDER));
            if let Some(grand) = parent.parent() {
                dirs.push(grand.join(paths::SCENES_FOLDER));
            }
        }
    }
    if let Some(user) = user_dir {
        dirs.push(paths::scenes_dir(user));
    }
    dirs
}

/// A folder is the player's if it has a manifest or a scene's folder.
fn has_scenes(dir: &Path) -> bool {
    dir.join("scenes.json").is_file() || KEYS.iter().any(|k| dir.join(k).is_dir())
}

fn read_manifest(dir: &Path) -> Manifest {
    let path = dir.join("scenes.json");
    if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() > MAX_MANIFEST_BYTES) {
        log::warn!("scenes: {} exceeds the 1 MiB manifest limit", path.display());
        return Manifest::default();
    }
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            log::warn!("scenes: scenes.json: {e}");
            Manifest::default()
        }),
        Err(_) => Manifest::default(),
    }
}

/// A PNG as the UI draws it. Images stay under strict file, dimension and
/// decoded-memory bounds; paths are confined to the chosen scene directory.
fn decode(path: &Path, root: &Path) -> Option<ColorImage> {
    let root = std::fs::canonicalize(root).ok()?;
    let full = std::fs::canonicalize(path).ok()?;
    if !full.starts_with(&root) {
        log::warn!("scenes: {} resolves outside its scene folder", path.display());
        return None;
    }
    let size = std::fs::metadata(&full).ok()?.len();
    if size > MAX_FILE_BYTES {
        log::warn!("scenes: {} exceeds the 32 MiB file limit", path.display());
        return None;
    }
    let file = std::fs::File::open(&full).ok()?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let (w, h) = {
        let info = reader.info();
        (info.width as usize, info.height as usize)
    };
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE || w.saturating_mul(h) > MAX_PIXELS {
        log::warn!("scenes: {} is {w}x{h}: left out (at most {MAX_SIDE} a side)", full.display());
        return None;
    }
    let buffer_size = reader.output_buffer_size().unwrap_or(0);
    if buffer_size == 0 || buffer_size > MAX_SCENE_BYTES {
        log::warn!("scenes: {} is too large after decoding", full.display());
        return None;
    }
    let mut pixels = vec![0; buffer_size];
    let info = reader.next_frame(&mut pixels).ok()?;
    let data = &pixels[..info.buffer_size()];
    let size = [info.width as usize, info.height as usize];
    Some(match info.color_type {
        png::ColorType::Rgba => ColorImage::from_rgba_unmultiplied(size, data),
        png::ColorType::Rgb => ColorImage::from_rgb(size, data),
        png::ColorType::GrayscaleAlpha => {
            let rgba: Vec<u8> = data.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect();
            ColorImage::from_rgba_unmultiplied(size, &rgba)
        }
        png::ColorType::Grayscale => {
            let rgb: Vec<u8> = data.iter().flat_map(|g| [*g, *g, *g]).collect();
            ColorImage::from_rgb(size, &rgb)
        }
        png::ColorType::Indexed => return None,
    })
}


/// Prepare a softened copy of a scene layer once, off the UI thread. The
/// original is kept for the unblurred side. Downsampling bounds work and GPU
/// memory; premultiplied color avoids dark fringes around transparent layers.
fn blur_background(image: &ColorImage) -> ColorImage {
    // Kept weak on purpose: the pause menu keeps the world readable behind
    // its translucent blocks — a light defocus, not a frosted pane.
    const REDUCTION: usize = 2;
    const RADIUS: usize = 1;
    let [width, height] = image.size;
    let small_w = width.div_ceil(REDUCTION);
    let small_h = height.div_ceil(REDUCTION);
    let mut small = Vec::with_capacity(small_w * small_h);
    for y in 0..small_h {
        for x in 0..small_w {
            let mut sum = [0u32; 4];
            let mut count = 0u32;
            for sy in y * REDUCTION..((y + 1) * REDUCTION).min(height) {
                for sx in x * REDUCTION..((x + 1) * REDUCTION).min(width) {
                    for (total, channel) in sum.iter_mut().zip(image.pixels[sy * width + sx].to_array()) {
                        *total += u32::from(channel);
                    }
                    count += 1;
                }
            }
            small.push(Color32::from_rgba_premultiplied(
                (sum[0] / count) as u8, (sum[1] / count) as u8,
                (sum[2] / count) as u8, (sum[3] / count) as u8,
            ));
        }
    }
    let mut blurred = Vec::with_capacity(small.len());
    for y in 0..small_h {
        for x in 0..small_w {
            let mut sum = [0u32; 4];
            let mut count = 0u32;
            for sy in y.saturating_sub(RADIUS)..=(y + RADIUS).min(small_h - 1) {
                for sx in x.saturating_sub(RADIUS)..=(x + RADIUS).min(small_w - 1) {
                    for (total, channel) in sum.iter_mut().zip(small[sy * small_w + sx].to_array()) {
                        *total += u32::from(channel);
                    }
                    count += 1;
                }
            }
            blurred.push(Color32::from_rgba_premultiplied(
                (sum[0] / count) as u8, (sum[1] / count) as u8,
                (sum[2] / count) as u8, (sum[3] / count) as u8,
            ));
        }
    }
    ColorImage::new([small_w, small_h], blurred)
}

// --- What is held ----------------------------------------------------------

struct Layer {
    texture: TextureHandle,
    blurred: TextureHandle,
    depth: f32,
    place: Place,
}

struct Scene {
    layers: Vec<Layer>,
    panel_right: bool,
    focus: Vec2,
    tint: [f32; 3],
    /// 0 to 1: how far the scene has faded in.
    appear: f32,
}

enum State {
    Loading,
    /// Nothing to show (no folder, no files, none that decode).
    Missing,
    Ready(Scene),
}

struct DecodedLayer {
    original: ColorImage,
    blurred: ColorImage,
}

struct Loaded {
    key: &'static str,
    generation: u32,
    images: Vec<Option<DecodedLayer>>,
    plan: Plan,
}

/// How the layers move: sines of different lengths across and down.
pub(super) struct Motion {
    enabled: bool,
    amp: Vec2,
    period: (f64, f64),
}

impl Motion {
    /// A layer's offset at time `t`, in points: nearer ones swing more.
    fn offset(&self, t: f64, depth: f32) -> Vec2 {
        if !self.enabled {
            return Vec2::ZERO;
        }
        let swing = |period: f64, phase: f64| (t * TAU / period.max(1.0) + phase).sin() as f32;
        vec2(swing(self.period.0, 0.0) * self.amp.x, swing(self.period.1, 1.3) * self.amp.y) * depth
    }
}

pub(super) struct Scenes {
    user_dir: Option<PathBuf>,
    /// The game's folder the scenes were last looked for with.
    located_for: Option<Option<PathBuf>>,
    dir: Option<PathBuf>,
    /// Where a folder would be looked for first, to say so.
    hint: Option<PathBuf>,
    manifest: Manifest,
    states: HashMap<&'static str, State>,
    tx: Sender<Loaded>,
    rx: Receiver<Loaded>,
    /// Bumped when what was loading no longer matters.
    generation: u32,
    shown: Option<&'static str>,
    previous: Option<&'static str>,
    fade: f32,
    /// When the start page opened: the scenes' clock.
    started: Instant,
    /// The scenes found in the folder, in the order they take turns.
    available: Vec<&'static str>,
}

impl Scenes {
    pub(super) fn new(user_dir: Option<PathBuf>) -> Self {
        let (tx, rx) = channel();
        Scenes {
            user_dir,
            located_for: None,
            dir: None,
            hint: None,
            manifest: Manifest::default(),
            states: HashMap::new(),
            tx,
            rx,
            generation: 0,
            shown: None,
            previous: None,
            fade: 1.0,
            started: Instant::now(),
            available: Vec::new(),
        }
    }

    /// Looks for the folder again when the game's has changed.
    pub(super) fn locate(&mut self, game_dir: Option<&Path>) {
        let want = game_dir.map(Path::to_path_buf);
        if self.located_for.as_ref() == Some(&want) {
            return;
        }
        self.located_for = Some(want);
        let all = candidates(game_dir, self.user_dir.as_deref());
        self.hint = all.first().cloned();
        let found = all.into_iter().find(|d| has_scenes(d));
        if found != self.dir {
            self.generation += 1;
            self.states.clear();
            self.dir = found;
        }
        self.manifest = self.dir.as_deref().map(read_manifest).unwrap_or_default();
        // Which scenes there are to take turns: a folder without art is not
        // one, and a single scene simply stays.
        let available: Vec<&'static str> = {
            let manifest = &self.manifest;
            match self.dir.as_deref() {
                Some(dir) => KEYS.iter().copied().filter(|key| plan(dir, key, manifest).is_some()).collect(),
                None => Vec::new(),
            }
        };
        self.available = available;
        self.started = Instant::now();
        if self.dir.is_none() {
            self.make_default_folder();
        }
    }

    /// The app's own folder, with a note on what goes in it, so that it can
    /// be found from a file manager.
    fn make_default_folder(&mut self) {
        let Some(user) = &self.user_dir else { return };
        let dir = paths::scenes_dir(user);
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let note = dir.join("README.txt");
        if !note.exists() {
            let _ = std::fs::write(
                &note,
                "Scenes for the start page: one folder per scene (story, online, sandbox, benchmark),\n\
                 each with PNG layers, back to front by name (the first covers the screen; the rest\n\
                 are figures with transparency). Or a scenes.json: see the project's documentation.\n\
                 The scenes take turns every nine seconds (or scenes.json's rotate.seconds).\n\
                 The same folder name, glue5-scenes, is also looked for inside the game's folder.\n",
            );
        }
    }

    /// Takes in the scenes the threads have decoded.
    pub(super) fn poll(&mut self, ctx: &Context) {
        while let Ok(done) = self.rx.try_recv() {
            // Not wanted any more: dropped.
            if done.generation != self.generation || !self.states.contains_key(done.key) {
                continue;
            }
            let mut layers = Vec::new();
            for (i, (image, layer)) in done.images.into_iter().zip(&done.plan.layers).enumerate() {
                let Some(image) = image else { continue };
                let texture = ctx.load_texture(format!("scene-{}-{i}", done.key), image.original, TextureOptions::LINEAR);
                let blurred = ctx.load_texture(format!("scene-{}-{i}-acrylic", done.key), image.blurred, TextureOptions::LINEAR);
                layers.push(Layer { texture, blurred, depth: layer.depth, place: layer.place });
            }
            let state = if layers.is_empty() {
                State::Missing
            } else {
                State::Ready(Scene {
                    layers,
                    panel_right: done.plan.panel_right,
                    focus: done.plan.focus,
                    tint: done.plan.tint,
                    appear: 0.0,
                })
            };
            self.states.insert(done.key, state);
            ctx.request_repaint();
        }
    }

    fn ensure(&mut self, ctx: &Context, key: &'static str) {
        if self.states.contains_key(key) {
            return;
        }
        let Some(dir) = self.dir.clone() else {
            self.states.insert(key, State::Missing);
            return;
        };
        let Some(plan) = plan(&dir, key, &self.manifest) else {
            self.states.insert(key, State::Missing);
            return;
        };
        self.states.insert(key, State::Loading);
        let (tx, generation, ctx) = (self.tx.clone(), self.generation, ctx.clone());
        let spawned = std::thread::Builder::new().name(format!("scene-{key}")).spawn(move || {
            let root = dir.clone();
            let mut total_bytes = 0usize;
            let mut images = Vec::with_capacity(plan.layers.len());
            for layer in &plan.layers {
                let image = decode(&layer.path, &root).and_then(|original| {
                    // Account for both textures against the same scene budget.
                    let [w, h] = original.size;
                    let bytes = (original.pixels.len() + w.div_ceil(4) * h.div_ceil(4))
                        .saturating_mul(std::mem::size_of::<Color32>());
                    if total_bytes.saturating_add(bytes) > MAX_SCENE_BYTES {
                        log::warn!("scenes: {key} exceeds the 32 MiB decoded scene limit");
                        return None;
                    }
                    total_bytes += bytes;
                    let blurred = blur_background(&original);
                    Some(DecodedLayer { original, blurred })
                });
                images.push(image);
            }
            let _ = tx.send(Loaded { key, generation, images, plan });
            ctx.request_repaint();
        });
        if let Err(e) = spawned {
            log::warn!("scenes: no thread for {key}: {e}");
            self.states.insert(key, State::Missing);
        }
    }

    /// The scenes take turns on their own clock (`rotate.seconds` in
    /// `scenes.json`, nine seconds without it), whatever the start page's
    /// chosen mode: the scene on show fades over the last one, and the next is
    /// loaded behind it. At most three scenes are held at once.
    pub(super) fn show(&mut self, ctx: &Context) {
        let key = self.due();
        if self.shown != Some(key) {
            self.previous = self.shown;
            self.shown = Some(key);
            self.fade = if self.previous.is_some() { 0.0 } else { 1.0 };
        }
        self.ensure(ctx, key);
        let order = self.order();
        let at = order.iter().position(|k| *k == key).unwrap_or(0);
        let next = order[(at + 1) % order.len()];
        if matches!(self.states.get(key), Some(State::Ready(_) | State::Missing)) {
            self.ensure(ctx, next);
        }
        let keep = [Some(key), self.previous, Some(next)];
        self.states.retain(|k, _| keep.contains(&Some(*k)));
        // Wakes for the next turn even when nothing else moves the page.
        let left = self.seconds() - (self.started.elapsed().as_secs_f64() % self.seconds());
        ctx.request_repaint_after(Duration::from_secs_f64(left.max(0.05)));
    }

    /// The order the scenes take turns in: the manifest's, else the folder's.
    fn order(&self) -> Vec<&'static str> {
        let mut order: Vec<&'static str> = match &self.manifest.rotate {
            Some(rotate) => rotate
                .order
                .as_ref()
                .map(|names| {
                    names
                        .iter()
                        .filter_map(|name| KEYS.iter().copied().find(|key| key.eq_ignore_ascii_case(name)))
                        .collect()
                })
                .unwrap_or_default(),
            None => Vec::new(),
        };
        if order.is_empty() {
            order = if self.available.is_empty() { KEYS.to_vec() } else { self.available.clone() };
        }
        order
    }

    /// How long each scene stays, within what is sensible.
    fn seconds(&self) -> f64 {
        let asked = self.manifest.rotate.as_ref().and_then(|r| r.seconds).map_or(ROTATE_SECS, f64::from);
        asked.clamp(3.0, 600.0)
    }

    /// The scene the clock is on.
    fn due(&self) -> &'static str {
        let order = self.order();
        let turn = (self.started.elapsed().as_secs_f64() / self.seconds()) as usize;
        order[turn % order.len()]
    }

    /// Drop decoded CPU buffers that finished after the game took over.
    pub(super) fn discard_pending(&mut self) {
        while self.rx.try_recv().is_ok() {}
    }

    /// Lets everything go (the game starts) and looks again next time.
    pub(super) fn release(&mut self) {
        if self.states.is_empty() && self.shown.is_none() && self.located_for.is_none() {
            return;
        }
        self.generation += 1;
        self.states.clear();
        self.shown = None;
        self.previous = None;
        self.fade = 1.0;
        self.located_for = None;
        self.available.clear();
    }

    /// Which side the text panel goes, if a scene is on show.
    pub(super) fn panel_right(&self) -> Option<bool> {
        match self.states.get(self.shown?) {
            Some(State::Ready(scene)) => Some(scene.panel_right),
            _ => None,
        }
    }

    /// For the page's status: where the scenes are, and how many are loaded.
    pub(super) fn report(&self) -> String {
        match (&self.dir, &self.hint) {
            (Some(dir), _) => {
                let ready = self.states.values().filter(|s| matches!(s, State::Ready(_))).count();
                format!("Scenes: {} ({ready} loaded)", dir.display())
            }
            (None, Some(hint)) => format!("Scenes: none found. Put them in {}", hint.display()),
            (None, None) => "Scenes: none found".to_owned(),
        }
    }

    pub(super) fn motion(&self, enabled: bool) -> Motion {
        let [ax, ay] = self.manifest.amp.unwrap_or(DEFAULT_MOTION_AMP);
        let [px, py] = self.manifest.period.unwrap_or(DEFAULT_MOTION_PERIOD);
        Motion {
            enabled,
            amp: vec2(ax.clamp(0.0, 80.0), ay.clamp(0.0, 80.0)),
            period: (f64::from(px), f64::from(py)),
        }
    }

    /// Draws the scene on show over the last (fading), `home`: all the layers,
    /// else only the farthest. `lines` is called between the first layer and
    /// the rest, so that figures pass in front of it. True when it was.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &mut self,
        ctx: &Context,
        painter: &Painter,
        rect: Rect,
        t: f64,
        scale: f32,
        home: bool,
        motion: &Motion,
        lines: &dyn Fn(&Painter),
    ) -> bool {
        let Some(current) = self.shown else { return false };
        if !motion.enabled {
            self.fade = 1.0;
            self.previous = None;
            if let Some(State::Ready(scene)) = self.states.get_mut(current) {
                scene.appear = 1.0;
            }
        }
        let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
        let current_done = !matches!(self.states.get(current), Some(State::Loading) | None);
        if current_done && self.fade < 1.0 {
            self.fade = (self.fade + dt / FADE_SECS).min(1.0);
            ctx.request_repaint();
        }
        if self.fade >= 1.0 {
            self.previous = None;
        }
        let current_missing = matches!(self.states.get(current), Some(State::Missing));
        if let Some(State::Ready(scene)) = self.previous.and_then(|p| self.states.get(p)) {
            // Under the new one; if there is none, fading out on its own.
            let alpha = if current_missing { 1.0 - self.fade } else { 1.0 };
            paint(painter, rect, scene, alpha, t, scale, motion, !home, None);
        }
        let fade = self.fade;
        if let Some(State::Ready(scene)) = self.states.get_mut(current) {
            scene.appear = (scene.appear + dt / APPEAR_SECS).min(1.0);
            if scene.appear < 1.0 {
                ctx.request_repaint();
            }
            let alpha = fade.min(scene.appear);
            return paint(painter, rect, scene, alpha, t, scale, motion, !home, Some(lines));
        }
        false
    }
}

/// The layers of a scene, back to front. True when `lines` was called.
#[allow(clippy::too_many_arguments)]
fn paint(
    painter: &Painter,
    rect: Rect,
    scene: &Scene,
    alpha: f32,
    t: f64,
    scale: f32,
    motion: &Motion,
    background_only: bool,
    lines: Option<&dyn Fn(&Painter)>,
) -> bool {
    if alpha <= 0.0 {
        return false;
    }
    let mut drew = false;
    for (i, layer) in scene.layers.iter().enumerate() {
        if background_only && i > 0 {
            break;
        }
        if i == 1 {
            if let Some(lines) = lines {
                lines(painter);
                drew = true;
            }
        }
        let size = layer.texture.size_vec2();
        let (dest, uv) = match layer.place {
            Place::Cover { overscan } => {
                let aspect = size.x / size.y.max(1.0);
                let viewport_aspect = rect.width() / rect.height().max(1.0);
                let uv = if viewport_aspect > aspect {
                    let h = aspect / viewport_aspect;
                    Rect::from_min_max(pos2(0.0, (1.0 - h) * scene.focus.y), pos2(1.0, (1.0 - h) * scene.focus.y + h))
                } else {
                    let w = viewport_aspect / aspect;
                    Rect::from_min_max(pos2((1.0 - w) * scene.focus.x, 0.0), pos2((1.0 - w) * scene.focus.x + w, 1.0))
                };
                (rect, uv.expand(overscan * 0.5))
            }
            Place::Sprite { height, x, bottom } => {
                let drift = motion.offset(t, layer.depth) * scale;
                let horizontal_room = if motion.enabled { motion.amp.x * layer.depth * scale } else { 0.0 };
                (sprite_rect(rect, size, height, x, bottom, drift, horizontal_room), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)))
            }
        };
        let drift = motion.offset(t, layer.depth) * scale;
        let shifted = match layer.place {
            Place::Cover { overscan } => dest.translate(drift).expand(rect.width().max(rect.height()) * overscan),
            Place::Sprite { .. } => dest,
        };
        let color = Color32::from_rgba_unmultiplied(
            (scene.tint[0].clamp(0.0, 2.0) * 255.0) as u8,
            (scene.tint[1].clamp(0.0, 2.0) * 255.0) as u8,
            (scene.tint[2].clamp(0.0, 2.0) * 255.0) as u8,
            (255.0 * alpha).clamp(0.0, 255.0) as u8,
        );
        // Both copies have the same placement, UVs and motion. Only the
        // clipped panel uses the softened copy; the free side stays sharp.
        let panel = super::shell::panel_rect(rect, painter.ctx().content_rect());
        let free = Rect::from_min_max(pos2(panel.right(), rect.top()), rect.max);
        painter.with_clip_rect(panel).image(layer.blurred.id(), shifted, uv, color);
        painter.with_clip_rect(free).image(layer.texture.id(), shifted, uv, color);
        drew = true;
    }
    drew
}

/// Fit the complete figure in the space to the right of the menu, including
/// after parallax drift. Old manifest x positions cannot put it behind text.
fn sprite_rect(screen: Rect, texture: Vec2, height: f32, x: f32, bottom: f32, drift: Vec2, horizontal_room: f32) -> Rect {
    let free_left = screen.left() + screen.width() * 0.62;
    // Reserve room on both sides of wide figures, so the clamp doesn't pin
    // them to the panel edge for most of the swing. Limit the reservation to
    // avoid making the artwork unreasonably small on phones.
    let room = horizontal_room.clamp(0.0, screen.width() * 0.04);
    let max_width = (screen.right() - free_left - 2.0 * room).min(screen.width() * 0.36);
    let mut h = screen.height() * height;
    let mut w = h * texture.x / texture.y.max(1.0);
    if w > max_width {
        h *= max_width / w;
        w = max_width;
    }
    let requested = screen.left() + screen.width() * x.clamp(0.63, 0.846);
    let min_left = free_left + room;
    let max_left = (screen.right() - w - room).max(min_left);
    let base_left = (requested - w * 0.5).clamp(min_left, max_left);
    let left = (base_left + drift.x).clamp(free_left, screen.right() - w);
    let top = screen.top() + screen.height() * bottom - h + drift.y;
    Rect::from_min_size(pos2(left, top), vec2(w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_blur_is_clipped_to_the_panel_and_the_free_side_is_sharp() {
        let ctx = Context::default();
        let original = ctx.load_texture("scene-sharp-test", ColorImage::new([4, 4], vec![Color32::WHITE; 16]), TextureOptions::LINEAR);
        let blurred = ctx.load_texture("scene-blur-test", ColorImage::new([1, 1], vec![Color32::WHITE]), TextureOptions::LINEAR);
        let sharp_id = original.id();
        let blur_id = blurred.id();
        let scene = Scene {
            layers: vec![Layer { texture: original, blurred, depth: 0.0, place: Place::Cover { overscan: 0.0 } }],
            panel_right: false, focus: vec2(0.5, 0.5), tint: [1.0; 3], appear: 1.0,
        };
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 500.0));
        let motion = Motion { enabled: false, amp: Vec2::ZERO, period: (1.0, 1.0) };
        let mut output = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |root| {
            let painter = root.ctx().layer_painter(egui::LayerId::background());
            paint(&painter, screen, &scene, 1.0, 0.0, 1.0, &motion, false, None);
        });
        let mut sharp_clip = None;
        let mut blur_clip = None;
        for shape in &output.shapes {
            if let egui::Shape::Mesh(mesh) = &shape.shape {
                if mesh.texture_id == sharp_id { sharp_clip = Some(shape.clip_rect); }
                if mesh.texture_id == blur_id { blur_clip = Some(shape.clip_rect); }
            }
        }
        output.textures_delta.clear();
        let sharp = sharp_clip.expect("sharp scene texture");
        let blurred = blur_clip.expect("blurred panel texture");
        assert!((blurred.right() - 620.0).abs() < 0.1, "{blurred:?}");
        assert!((sharp.left() - 620.0).abs() < 0.1, "{sharp:?}");
    }

    #[test]
    fn figures_and_their_drift_stay_outside_the_menu() {
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(844.0, 390.0));
        for x in [-0.5, 0.2, 0.74, 1.5] {
            for drift in [-100.0, 0.0, 100.0] {
                let figure = sprite_rect(screen, vec2(1000.0, 1000.0), 0.95, x, 1.0, vec2(drift, 0.0), 32.0);
                assert!(figure.left() >= screen.left() + screen.width() * 0.62 - 0.01);
                assert!(figure.right() <= screen.right() + 0.01);
            }
        }
    }

    #[test]
    fn wide_figures_have_visible_room_to_move_without_crossing_the_panel() {
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(844.0, 390.0));
        let texture = vec2(1600.0, 800.0);
        let left = sprite_rect(screen, texture, 0.95, 0.74, 1.04, vec2(-32.0, 0.0), 32.0);
        let right = sprite_rect(screen, texture, 0.95, 0.74, 1.04, vec2(32.0, 0.0), 32.0);
        assert!((right.left() - left.left() - 64.0).abs() < 0.01);
        assert!(left.left() >= screen.width() * 0.62 - 0.01);
        assert!(right.right() <= screen.right() + 0.01);
    }

    #[test]
    fn default_parallax_is_visible_and_reduce_motion_stops_it() {
        let scenes = Scenes::new(None);
        let motion = scenes.motion(true);
        assert_eq!(motion.amp, vec2(32.0, 9.0));
        assert_eq!(motion.period, (16.0, 23.0));
        let start = motion.offset(0.0, 1.0);
        let after = motion.offset(4.0, 1.0);
        assert!((after.x - start.x).abs() >= 30.0);
        assert_eq!(scenes.motion(false).offset(4.0, 1.0), Vec2::ZERO);
    }

    #[test]
    fn background_blur_is_bounded_and_softens_edges() {
        let mut pixels = vec![Color32::BLACK; 32 * 16];
        for y in 0..16 {
            for x in 16..32 {
                pixels[y * 32 + x] = Color32::WHITE;
            }
        }
        let result = blur_background(&ColorImage::new([32, 16], pixels));
        assert_eq!(result.size, [16, 8]);
        let edge = result.pixels[2 * 16 + 7].r();
        assert!(edge > 0 && edge < 255, "blur should soften the edge");
        let tiny = blur_background(&ColorImage::new([1, 1], vec![Color32::WHITE]));
        assert_eq!(tiny.size, [1, 1]);
        assert_eq!(tiny.pixels[0], Color32::WHITE);
    }

    #[test]
    fn supplied_scene_manifest_shape_is_accepted() {
        let manifest: Manifest = serde_json::from_str(
            r#"{
                "amp": [20, 6],
                "period": [47, 61],
                "scenes": {
                    "story": {
                        "panel_side": "right",
                        "layers": [
                            { "file": "story/1-background.png", "depth": 0.2, "overscan": 0.06 },
                            { "file": "story/2-figure.png", "depth": 1.0, "height": 0.92, "x": 0.2, "bottom": 1.06 }
                        ]
                    }
                }
            }"#,
        )
        .unwrap();
        let plan = plan(Path::new("/user/glue5-scenes"), "story", &manifest).unwrap();
        assert_eq!(plan.layers.len(), 2);
        assert!(plan.panel_right);
        assert_eq!(plan.layers[0].path, PathBuf::from("/user/glue5-scenes/story/1-background.png"));
    }

    #[test]
    fn the_scenes_take_turns_every_nine_seconds_and_the_manifest_can_say_otherwise() {
        let mut scenes = Scenes::new(None);
        assert_eq!(scenes.seconds(), 9.0);
        assert_eq!(scenes.order(), KEYS.to_vec());
        scenes.manifest.rotate =
            Some(Rotate { seconds: Some(4.0), order: Some(vec!["benchmark".to_owned(), "story".to_owned()]) });
        assert_eq!(scenes.seconds(), 4.0);
        assert_eq!(scenes.order(), vec!["benchmark", "story"]);
        // Names that are not scenes are ignored; if none is left, the folder's
        // own order (here: no folder, so every scene) is used.
        scenes.manifest.rotate = Some(Rotate { seconds: None, order: Some(vec!["nothing".to_owned()]) });
        assert_eq!(scenes.seconds(), 9.0);
        assert_eq!(scenes.order(), KEYS.to_vec());
        // Out of the sensible range.
        scenes.manifest.rotate = Some(Rotate { seconds: Some(0.1), order: None });
        assert_eq!(scenes.seconds(), 3.0);
    }

    #[test]
    fn traversal_and_oversized_layer_lists_are_rejected() {
        let layer = |file: &str| LayerSpec {
            file: file.to_owned(),
            depth: None,
            overscan: None,
            height: None,
            x: None,
            bottom: None,
        };
        let manifest = Manifest {
            scenes: HashMap::from([(
                "story".to_owned(),
                SceneSpec {
                    layers: vec![layer("../outside.png")],
                    panel_side: None,
                    focus: None,
                    tint: None,
                },
            )]),
            ..Manifest::default()
        };
        assert!(plan(Path::new("/user/glue5-scenes"), "story", &manifest).is_none());

        let manifest = Manifest {
            scenes: HashMap::from([(
                "story".to_owned(),
                SceneSpec {
                    layers: (0..=MAX_LAYERS).map(|_| layer("story/layer.png")).collect(),
                    panel_side: None,
                    focus: None,
                    tint: None,
                },
            )]),
            ..Manifest::default()
        };
        assert!(plan(Path::new("/user/glue5-scenes"), "story", &manifest).is_none());
    }
}
