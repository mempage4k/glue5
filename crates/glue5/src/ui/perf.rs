//! The application's performance overlay (Settings › Video › Performance
//! overlay: Glue 5), over the game in a corner of the safe area: the frame
//! rate, the last frames' times as a graph (bars: present to present, the
//! line: the GPU's), where a frame's time goes (the engine's main thread,
//! the renderer's thread, the GPU), and at full detail the busiest threads,
//! the renderer's counts, the resolution and memory.
//!
//! It costs little: the renderer gathers its figures only while it shows
//! (glue5_gpu::perf), the overlay is redrawn 20 times a second (its one
//! line 4 times), its numbers change 4 times a second and the threads are
//! read once a second, on a thread of their own: on Windows that walks every
//! thread of the system (tens of milliseconds), which no frame waits for.

use std::collections::HashMap;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use egui::{Align2, Color32, FontId, Id, LayerId, Order, Pos2, Rect, Shape, Stroke, pos2, vec2};
use glue5_gpu::census::Summary;
use glue5_gpu::perf::Snapshot;

use super::{Status, theme};
use crate::settings::{Corner, OverlayDetail, Settings, Upscaling};

const REDRAW: Duration = Duration::from_millis(50);
const FIGURES: Duration = Duration::from_millis(250);
const THREADS: Duration = Duration::from_secs(1);
/// Threads listed at full detail.
const BUSIEST: usize = 6;

const BACKGROUND: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 160);
const TEXT: Color32 = theme::palette::TEXT_HI;
const MUTED: Color32 = theme::palette::TEXT_MID;
const GOOD: Color32 = Color32::from_rgb(117, 184, 125);
const SLOW: Color32 = Color32::from_rgb(240, 180, 100);
const BAD: Color32 = Color32::from_rgb(190, 105, 92);
const GPU: Color32 = Color32::from_rgb(169, 164, 151);
const BAR: Color32 = theme::palette::SUN;
const TRACK: Color32 = Color32::from_rgba_premultiplied(30, 30, 30, 30);
const RULE: Color32 = Color32::from_rgba_premultiplied(40, 40, 40, 40);

const FONT: f32 = 11.0;
const ROW: f32 = 14.0;
const PAD: f32 = 6.0;
const GRAPH: f32 = 40.0;
/// From the safe area's edges.
const MARGIN: f32 = 8.0;

#[derive(Default)]
pub(super) struct PerfHud {
    drawn_at: Option<Instant>,
    figures: Figures,
    figures_at: Option<Instant>,
    threads: Threads,
    /// What the frames' HUD phases did, as the renderer's census sees them.
    census: Summary,
}

/// The numbers shown, held between their updates.
#[derive(Default)]
struct Figures {
    /// Mean frame time over the last quarter of the graph, and the longest
    /// in it all.
    frame: Option<f32>,
    longest: f32,
    render: Option<f32>,
    gpu: Option<f32>,
    draws: u32,
    passes: u32,
    submits: u32,
    gpu_bytes: u64,
}

#[derive(Default)]
struct Threads {
    /// CPU time so far, by thread.
    last: HashMap<u64, Duration>,
    at: Option<Instant>,
    /// The next sample, being read off the frame.
    pending: Option<JoinHandle<(Instant, Vec<glue5_runtime::cpu::Thread>)>>,
    /// The busiest, with their share of a core (percent).
    busiest: Vec<(String, f32)>,
    /// The engine's main thread's share.
    main: Option<f32>,
    footprint: Option<u64>,
}

impl Threads {
    /// Takes the sample read meanwhile, and starts the next when due.
    fn update(&mut self, now: Instant) {
        if self.pending.as_ref().is_some_and(|p| p.is_finished()) {
            if let Some(Ok((at, threads))) = self.pending.take().map(JoinHandle::join) {
                self.sample(at, threads);
            }
        }
        if self.pending.is_none() && self.at.is_none_or(|t| now - t >= THREADS) {
            self.pending = std::thread::Builder::new()
                .name("perf threads".into())
                .spawn(|| (Instant::now(), glue5_runtime::cpu::threads()))
                .ok();
        }
    }

    fn sample(&mut self, now: Instant, threads: Vec<glue5_runtime::cpu::Thread>) {
        let wall = self.at.map(|t| now - t);
        let mut shares: Vec<(String, f32)> = Vec::new();
        let mut last = HashMap::with_capacity(threads.len());
        self.main = None;
        for t in threads {
            if let (Some(wall), Some(before)) = (wall, self.last.get(&t.id)) {
                let share = t.cpu.saturating_sub(*before).as_secs_f32() / wall.as_secs_f32().max(0.001) * 100.0;
                if t.name == "engine main" {
                    self.main = Some(share);
                }
                let name = match t.name.strip_prefix("[RAGE] ") {
                    _ if t.name.is_empty() => "(unnamed)".to_owned(),
                    Some(rest) => rest.to_owned(),
                    None => t.name,
                };
                shares.push((name, share));
            }
            last.insert(t.id, t.cpu);
        }
        shares.sort_by(|a, b| b.1.total_cmp(&a.1));
        shares.truncate(BUSIEST);
        self.busiest = shares;
        self.last = last;
        self.at = Some(now);
        self.footprint = glue5_runtime::memory::footprint().0;
    }
}

impl PerfHud {
    /// Whether it is time to draw it again (when nothing else is drawn):
    /// for the graph's sake, or (one line) when its numbers change.
    pub(super) fn due(&self, detail: OverlayDetail) -> bool {
        let every = if detail == OverlayDetail::Minimal { FIGURES } else { REDRAW };
        self.drawn_at.is_none_or(|t| t.elapsed() >= every)
    }

    /// Not shown: the next showing starts its figures afresh.
    pub(super) fn hide(&mut self) {
        *self = PerfHud::default();
    }

    /// Draws the overlay; returns where (what else is drawn keeps off it).
    pub(super) fn draw(&mut self, ctx: &egui::Context, safe: Rect, settings: &Settings, status: &Status) -> Rect {
        let now = Instant::now();
        self.drawn_at = Some(now);
        let snapshot = glue5_gpu::perf::snapshot();
        if self.figures_at.is_none_or(|t| now - t >= FIGURES) {
            self.figures_at = Some(now);
            self.figures = figures(&snapshot);
            self.census = glue5_gpu::census::summary();
        }
        let detail = settings.overlay_detail;
        if detail != OverlayDetail::Minimal {
            self.threads.update(now);
        }
        // The frame time aimed at: the frame rate's limit, else the display's.
        // The thermal back-off's cap counts: the game is held to it.
        let budget = 1000.0 / if status.frame_cap > 0 { status.frame_cap as f32 } else { status.refresh_rate.max(1.0) };
        let painter = ctx.layer_painter(LayerId::new(Order::Middle, Id::new("performance overlay")));
        let font = FontId::monospace(FONT);
        let f = &self.figures;

        if detail == OverlayDetail::Minimal {
            let mut job = egui::text::LayoutJob::default();
            let mut add = |text: &str, color| job.append(text, 0.0, egui::TextFormat::simple(font.clone(), color));
            match f.frame {
                Some(ms) => {
                    add(&format!("{:.0} fps", 1000.0 / ms), pace(ms, budget));
                    add(&format!(" · {ms:.1} ms"), TEXT);
                }
                None => add("– fps", MUTED),
            }
            if let Some(gpu) = f.gpu {
                add(&format!(" · GPU {gpu:.1}"), TEXT);
            }
            let galley = painter.layout_job(job);
            let rect = place(safe, settings.overlay_corner, galley.size() + vec2(PAD * 2.0, PAD * 1.4));
            painter.rect_filled(rect, 14, BACKGROUND);
            painter.galley(rect.min + vec2(PAD, PAD * 0.7), galley, TEXT);
            return rect;
        }

        let full = detail == OverlayDetail::Full;
        let width = if full { 228.0 } else { 204.0 };
        // Where a frame's time goes: the engine's main thread, the renderer's
        // thread, the GPU.
        let main = self.threads.main.zip(f.frame).map(|(share, ms)| share / 100.0 * ms);
        let costs = [("main", main, TEXT), ("render", f.render, TEXT), ("gpu", f.gpu, GPU)];
        let costs: Vec<_> = costs.into_iter().filter_map(|(l, ms, c)| ms.map(|ms| (l, ms, c))).collect();
        let bound = costs.iter().map(|c| c.1).fold(0.0, f32::max);
        let threads = if full { self.threads.busiest.len() } else { 0 };
        let mut height = PAD * 2.0 + 18.0 + GRAPH + 6.0 + ROW * (1 + costs.len()) as f32;
        let (w, h) = status.window;
        let scaled = status.back_buffer.filter(|&b| b != (w, h));
        let census = if full { census_lines(&self.census) } else { Vec::new() };
        if full {
            height += 7.0 + ROW * threads as f32 + 7.0 + ROW * (3 + scaled.is_some() as usize) as f32;
            if status.thermal.is_some() {
                height += ROW;
            }
            height += 7.0 + ROW * census.len() as f32;
        }
        let rect = place(safe, settings.overlay_corner, vec2(width, height));
        painter.rect_filled(rect, 14, BACKGROUND);
        let (left, right) = (rect.min.x + PAD, rect.max.x - PAD);
        let mut y = rect.min.y + PAD;

        // The frame rate, and what it aims at.
        let fps = f.frame.map_or("–".into(), |ms| format!("{:.0}", 1000.0 / ms));
        let color = f.frame.map_or(MUTED, |ms| pace(ms, budget));
        let at = painter.text(pos2(left, y), Align2::LEFT_TOP, fps, FontId::monospace(15.0), color);
        painter.text(pos2(at.max.x + 4.0, y + 4.0), Align2::LEFT_TOP, "fps", font.clone(), MUTED);
        let aim = if status.frame_cap > 0 {
            format!("{} max", status.frame_cap)
        } else {
            format!("{:.0} Hz", status.refresh_rate)
        };
        painter.text(pos2(right, y + 4.0), Align2::RIGHT_TOP, aim, font.clone(), MUTED);
        y += 18.0;

        graph(&painter, Rect::from_min_max(pos2(left, y + 2.0), pos2(right, y + 2.0 + GRAPH)), &snapshot, budget);
        y += GRAPH + 6.0;

        let row = |y: f32, label: &str, value: String, color: Color32| {
            painter.text(pos2(left, y), Align2::LEFT_TOP, label, font.clone(), MUTED);
            painter.text(pos2(right, y), Align2::RIGHT_TOP, value, font.clone(), color);
        };
        let frame = f.frame.map_or("–".into(), |ms| format!("{ms:.1} ms  max {:.1}", f.longest));
        row(y, "frame", frame, TEXT);
        y += ROW;
        for (label, ms, color) in &costs {
            // The one a frame waits on, when it takes most of the frame.
            let limits = *ms == bound && f.frame.is_some_and(|frame| *ms >= frame * 0.8);
            row(y, label, format!("{ms:.1} ms"), if limits { SLOW } else { *color });
            y += ROW;
        }
        if !full {
            return rect;
        }

        let rule = |y: f32| painter.hline(left..=right, y + 3.0, Stroke::new(0.5, RULE));
        rule(y);
        y += 7.0;
        let name_width = 96.0;
        for (name, share) in &self.threads.busiest {
            let name: String = name.chars().take(13).collect();
            painter.text(pos2(left, y), Align2::LEFT_TOP, name, font.clone(), TEXT);
            painter.text(pos2(right, y), Align2::RIGHT_TOP, format!("{share:.0}%"), font.clone(), TEXT);
            let track = Rect::from_min_max(pos2(left + name_width, y + 4.0), pos2(right - 34.0, y + 9.0));
            painter.rect_filled(track, 0, TRACK);
            let filled = track.width() * (share / 100.0).clamp(0.0, 1.0);
            let color = if *share > 85.0 { SLOW } else { BAR };
            painter.rect_filled(Rect::from_min_size(track.min, vec2(filled, track.height())), 0, color);
            y += ROW;
        }
        rule(y);
        y += 7.0;
        row(y, "draws", format!("{} · {} passes · {} sub", f.draws, f.passes, f.submits), TEXT);
        y += ROW;
        match scaled {
            Some((bw, bh)) => {
                row(y, "res", format!("{bw}×{bh} → {w}×{h}"), TEXT);
                y += ROW;
                row(y, "upscaler", upscaler(settings.upscaler).into(), TEXT);
            }
            None => row(y, "res", format!("{w}×{h}"), TEXT),
        }
        y += ROW;
        let gb = |bytes: u64| format!("{:.1} GB", bytes as f64 / (1u64 << 30) as f64);
        let memory = match self.threads.footprint {
            Some(used) => format!("{} · gpu {}", gb(used), gb(f.gpu_bytes)),
            None => format!("gpu {}", gb(f.gpu_bytes)),
        };
        row(y, "mem", memory, TEXT);
        y += ROW;
        // The phone's heat, and what the game is held to because of it.
        if let Some(t) = status.thermal {
            let value = if t.level > 0 { format!("{} · {} fps", t.state(), t.fps) } else { t.state().to_string() };
            row(y, "thermal", value, if t.level > 0 { SLOW } else { TEXT });
            y += ROW;
        }

        // What a frame's HUD phase does: whether it has one at all, how big it
        // is, how often it changes target (each change reloads a tile-based
        // GPU's tiles), and which of its programs read D3D's SV_Position, so
        // what the HUD's resolution could be made independent of the scene's
        // is measured rather than guessed.
        rule(y);
        y += 7.0;
        for (label, value, color) in &census {
            row(y, label, value.clone(), *color);
            y += ROW;
        }
        rect
    }
}

/// The census's rows: what the frames since it came on add up to.
///
/// Empty labels are continuation lines (a program's name under the count of
/// them), so the column of labels stays readable at this width.
fn census_lines(c: &Summary) -> Vec<(String, String, Color32)> {
    if c.frames == 0 {
        return vec![("hud".to_owned(), "no frames yet".to_owned(), MUTED)];
    }
    let mut out = vec![(
        "hud phase".to_owned(),
        format!("{}/{} frames", c.with_hud, c.frames),
        if c.with_hud == 0 { MUTED } else { TEXT },
    )];
    if c.with_hud == 0 {
        // A menu over everything: no scene/HUD split to redirect, so the whole
        // frame takes the bicubic path.
        out.push(("".to_owned(), "all scene: bicubic".to_owned(), MUTED));
        return out;
    }
    out.push((
        "hud draws".to_owned(),
        format!("{}-{} mean {:.0}", c.hud_draws_min, c.hud_draws_max, c.hud_draws_mean),
        TEXT,
    ));
    out.push((
        "target swaps".to_owned(),
        format!("max {} mean {:.1}", c.target_set_changes_max, c.target_set_changes_mean),
        if c.target_set_changes_max > 4 { SLOW } else { TEXT },
    ));
    out.push((
        "BB target sets".to_owned(),
        format!("max {} mean {:.1}", c.bb_passes_max, c.bb_passes_mean),
        TEXT,
    ));
    out.push((
        "reads SV_Pos".to_owned(),
        if c.reads_position.is_empty() { "none".to_owned() } else { c.reads_position.len().to_string() },
        if c.reads_position.is_empty() { GOOD } else { SLOW },
    ));
    for name in c.reads_position.iter().take(2) {
        out.push(("".to_owned(), name.chars().take(26).collect(), MUTED));
    }
    out.push((
        "bb read / ds".to_owned(),
        format!("{}/{} frames", c.frames_reading_bb, c.frames_with_ds),
        if c.frames_reading_bb > 0 || c.frames_with_ds > 0 { SLOW } else { GOOD },
    ));
    if let [only] = c.sizes[..] {
        out.push(("hud sizes".to_owned(), format!("{}x{}", only.0, only.1), TEXT));
    } else if !c.sizes.is_empty() {
        out.push(("hud sizes".to_owned(), format!("{} distinct", c.sizes.len()), TEXT));
    }
    let other_violations: Vec<_> = c
        .violations
        .iter()
        .filter(|(v, _)| !v.starts_with("HUD draw into a scene-sized target "))
        .collect();
    let issue_count = c.scene_sized_targets.len() + other_violations.len();
    if issue_count > 0 {
        out.push((
            "BROKE".to_owned(),
            format!("{issue_count} issue{}", if issue_count == 1 { "" } else { "s" }),
            BAD,
        ));
        if let Some(issue) = c.scene_sized_targets.first() {
            out.push((
                "".to_owned(),
                format!("RT#{} {}×{} F{}", issue.id, issue.width, issue.height, issue.format),
                BAD,
            ));
            out.push((
                "".to_owned(),
                format!("D{}-{} S{} M{} sl{} pre {}/{}f", issue.first_draw, issue.last_draw, issue.samples, issue.mip, issue.slice, issue.written_before_scene_done_frames, issue.frames),
                BAD,
            ));
            out.push((
                "".to_owned(),
                format!("BB reads {} · {} HUD draws", issue.reads_into_bb_draws, issue.draws),
                BAD,
            ));
            if let Some(program) = issue.pixel_programs.first() {
                out.push(("".to_owned(), format!("PS {}", shorten_mid(program, 23)), MUTED));
            }
            if let Some(source) = c.scene_targets_read_by_hud.first() {
                out.push((
                    "".to_owned(),
                    format!("SRC RT#{} {}×{} mip{}", source.id, source.width, source.height, source.mip),
                    SLOW,
                ));
                if let Some(reader) = source.bb_reader_programs.first() {
                    out.push(("".to_owned(), format!("via {}", shorten_mid(reader, 22)), MUTED));
                }
            }
        } else if let Some((v, frames)) = other_violations.first() {
            out.push(("".to_owned(), format!("{} {frames}f", shorten_mid(v, 26)), BAD));
        }
    }
    out
}

fn shorten_mid(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_owned();
    }
    if max <= 1 {
        return "…".chars().take(max).collect();
    }
    let left = max / 2;
    let right = max - 1 - left;
    let mut out = String::new();
    out.extend(chars[..left].iter().copied());
    out.push('…');
    out.extend(chars[chars.len() - right..].iter().copied());
    out
}

#[cfg(test)]
mod census_presentation_tests {
    use super::{census_lines, shorten_mid, BAD};
    use glue5_gpu::census::{SceneSizedTargetIssue, SceneTargetReadSummary, Summary};

    #[test]
    fn keeps_both_ends_of_a_long_diagnostic() {
        assert_eq!(shorten_mid("im::PS_Bicubic", 10), "im::P…ubic");
        assert_eq!(shorten_mid("short", 10), "short");
        assert_eq!(shorten_mid("abcdef", 1), "…");
    }

    #[test]
    fn broke_rows_show_target_subresource_and_shader() {
        let summary = Summary {
            frames: 1,
            with_hud: 1,
            scene_sized_targets: vec![SceneSizedTargetIssue {
                id: 42,
                width: 1280,
                height: 720,
                format: 5,
                mip: 1,
                slice: 2,
                samples: 4,
                frames: 1,
                draws: 3,
                first_draw: 2,
                last_draw: 4,
                written_before_scene_done_frames: 1,
                reads_into_bb_draws: 2,
                pixel_program_ids: vec![20],
                pixel_programs: vec!["im::PS_BlitScene".to_owned()],
            }],
            scene_targets_read_by_hud: vec![SceneTargetReadSummary {
                id: 31,
                width: 640,
                height: 360,
                format: 5,
                mip: 1,
                slice: 0,
                samples: 1,
                frames: 1,
                bb_read_draws: 1,
                bb_reader_program_ids: vec![30],
                bb_reader_programs: vec!["im::PS_Upscale".to_owned()],
            }],
            ..Summary::default()
        };
        let lines = census_lines(&summary);
        assert!(lines.iter().any(|(label, _, color)| label == "BROKE" && *color == BAD));
        assert!(lines.iter().any(|(_, value, _)| value.contains("RT#42")));
        assert!(lines.iter().any(|(_, value, _)| value.contains("D2-4 S4 M1 sl2 pre 1/1f")));
        assert!(lines.iter().any(|(_, value, _)| value.contains("PS im::PS_BlitScene")));
        assert!(lines.iter().any(|(_, value, _)| value.contains("SRC RT#31")));
        assert!(lines.iter().any(|(_, value, _)| value.contains("via im::PS_Upscale")));
    }
}

fn figures(s: &Snapshot) -> Figures {
    // The last quarter of the graph: about half a second.
    let recent = |v: &[f32]| {
        let tail = &v[v.len().saturating_sub(glue5_gpu::perf::FRAMES / 4)..];
        (!tail.is_empty()).then(|| tail.iter().sum::<f32>() / tail.len() as f32)
    };
    Figures {
        frame: recent(&s.frame_ms),
        longest: s.frame_ms.iter().copied().fold(0.0, f32::max),
        render: recent(&s.render_ms),
        gpu: recent(&s.gpu_ms),
        draws: s.draws,
        passes: s.passes,
        submits: s.submits,
        gpu_bytes: s.gpu_bytes,
    }
}

/// How a frame time compares with the one aimed at.
fn pace(ms: f32, budget: f32) -> Color32 {
    if ms <= budget * 1.2 {
        GOOD
    } else if ms <= budget * 2.0 {
        SLOW
    } else {
        BAD
    }
}

/// A box of `size` in `corner` of the safe area.
fn place(safe: Rect, corner: Corner, size: egui::Vec2) -> Rect {
    let inner = safe.shrink(MARGIN);
    let x = match corner {
        Corner::TopLeft | Corner::BottomLeft => inner.min.x,
        Corner::TopRight | Corner::BottomRight => inner.max.x - size.x,
    };
    let y = match corner {
        Corner::TopLeft | Corner::TopRight => inner.min.y,
        Corner::BottomLeft | Corner::BottomRight => inner.max.y - size.y,
    };
    Rect::from_min_size(pos2(x, y), size)
}

/// The frames as bars (coloured by pace) under lines at the budget and
/// twice it, the GPU's times as a line over them.
fn graph(painter: &egui::Painter, rect: Rect, s: &Snapshot, budget: f32) {
    let top = (budget * 3.0).min(100.0);
    let y_of = |ms: f32| rect.max.y - (ms / top).clamp(0.0, 1.0) * rect.height();
    let step = rect.width() / glue5_gpu::perf::FRAMES as f32;
    // The newest frame at the right edge.
    let x_of = |i: usize, n: usize| rect.max.x - (n - i) as f32 * step;
    let n = s.frame_ms.len();
    for (i, &ms) in s.frame_ms.iter().enumerate() {
        let x = x_of(i, n);
        let bar = Rect::from_min_max(pos2(x, y_of(ms)), pos2(x + (step - 0.5).max(0.5), rect.max.y));
        painter.rect_filled(bar, 0.0, pace(ms, budget));
    }
    for ms in [budget, budget * 2.0] {
        let y = y_of(ms);
        painter.extend(Shape::dashed_line(
            &[pos2(rect.min.x, y), pos2(rect.max.x, y)],
            Stroke::new(0.5, RULE),
            2.0,
            2.0,
        ));
    }
    let n = s.gpu_ms.len();
    if n >= 2 {
        let points: Vec<Pos2> =
            s.gpu_ms.iter().enumerate().map(|(i, &ms)| pos2(x_of(i, n) + step / 2.0, y_of(ms))).collect();
        painter.add(Shape::line(points, Stroke::new(1.0, GPU)));
    }
}

fn upscaler(u: Upscaling) -> &'static str {
    match u {
        Upscaling::Bicubic => "bicubic",
        Upscaling::MetalFx => "MetalFX",
        Upscaling::MetalFxTemporal => "MetalFX Temporal",
        Upscaling::Fsr => "FSR 2",
    }
}
