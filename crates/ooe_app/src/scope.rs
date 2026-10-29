//! The oscilloscope tab: up to four signals of the replayed case against
//! time, with a cursor that follows playback. Clicking the plot seeks.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::ui::{ButtonSpec, Cursor, Tone, Ui};
use ooe_core::grade::Sample;
use ooe_core::lang::{DT, Dst, Src};

use crate::state::{Act, Redraw, Session, t};
use crate::{View, bench, editor};

/// Most signals on the scope at once.
pub const MAX_TRACES: usize = 4;

/// Bottom-left corner and size of the plot.
const ORIGIN: Vec2 = Vec2::new(-436.0, -322.0);
const SIZE: Vec2 = Vec2::new(1048.0, 162.0);
/// Points drawn per trace.
const POINTS: usize = 360;

/// The scope's phosphor green; used only for scope traces.
pub const PHOSPHOR: Color = Color::srgb(0.45, 1.0, 0.55);

/// Phosphor green first, then three contrasting channels.
fn channel(k: usize) -> Color {
    [
        PHOSPHOR,
        Color::srgb(1.0, 0.56, 0.26),
        Color::srgb(0.58, 0.66, 1.0),
        Color::srgb(1.0, 0.86, 0.38),
    ][k % MAX_TRACES]
}

/// How the plot maps samples to pixels; read by [`sync_cursor`].
#[derive(Component)]
pub struct ScopePlot {
    duration: f32,
    /// Each trace's signal and its (low, high) range.
    traces: Vec<(Src, f32, f32)>,
}

impl ScopePlot {
    fn x(&self, t: f32) -> f32 {
        ORIGIN.x + (t / self.duration).clamp(0.0, 1.0) * SIZE.x
    }

    fn y(&self, k: usize, v: f32) -> f32 {
        let (_, lo, hi) = self.traces[k];
        ORIGIN.y + ((v - lo) / (hi - lo)).clamp(-0.02, 1.02) * SIZE.y
    }
}

#[derive(Component)]
pub struct ScopeCursor;

/// The marker where trace `k` meets the cursor.
#[derive(Component)]
pub struct ScopeDot(usize);

/// Trace `k`'s value at the cursor.
#[derive(Component)]
pub struct ScopeValue(usize);

/// Compact number for readouts.
pub fn short(v: f32) -> String {
    let a = v.abs();
    if a >= 100.0 {
        format!("{v:.0}")
    } else if a >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

/// Signals the player can put on the scope: the task's suggestions, then
/// every sensor, actuator and register the bench offers.
fn offered(s: &Session) -> Vec<Src> {
    let task = s.task();
    let mut out: Vec<Src> = task.scope.to_vec();
    out.extend(task.sensors.iter().map(|&x| Src::Sense(x)));
    out.extend(task.acts.iter().map(|&a| Src::Out(a)));
    if let Some(d) = editor::doc(s) {
        let mut regs: Vec<u8> = d
            .code
            .iter()
            .filter_map(|i| match i.d {
                Dst::Reg(r) => Some(r),
                Dst::Out(_) => None,
            })
            .collect();
        regs.sort_unstable();
        out.extend(regs.into_iter().map(Src::Reg));
    }
    let mut seen = Vec::new();
    out.retain(|x| {
        let new = !seen.contains(x);
        seen.push(*x);
        new
    });
    out.truncate(16);
    out
}

/// Each trace's range. Traces whose spans are within ×5 of each other share
/// one scale, so a position and its target can be compared by eye.
fn ranges(samples: &[Sample], traces: &[Src]) -> Vec<(f32, f32)> {
    let own: Vec<(f32, f32)> = traces
        .iter()
        .map(|&src| {
            let (lo, hi) = samples.iter().fold((0.0f32, 0.0f32), |(lo, hi), x| {
                let v = x.read(src);
                if v.is_finite() {
                    (lo.min(v), hi.max(v))
                } else {
                    (lo, hi)
                }
            });
            if hi - lo < 1e-4 {
                (-1.0, 1.0)
            } else {
                (lo, hi)
            }
        })
        .collect();
    let spans: Vec<f32> = own.iter().map(|(lo, hi)| hi - lo).collect();
    let max = spans.iter().copied().fold(0.0, f32::max);
    let min = spans.iter().copied().fold(f32::MAX, f32::min);
    let pad = |(lo, hi): (f32, f32)| {
        let m = (hi - lo) * 0.08;
        (lo - m, hi + m)
    };
    if !own.is_empty() && max / min <= 5.0 {
        let lo = own.iter().map(|r| r.0).fold(f32::MAX, f32::min);
        let hi = own.iter().map(|r| r.1).fold(f32::MIN, f32::max);
        vec![pad((lo, hi)); own.len()]
    } else {
        own.into_iter().map(pad).collect()
    }
}

pub fn draw(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    // Signal picker.
    ui.caption(
        t(l, "SIGNALS", "信号"),
        Vec2::new(-612.0, -140.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    for (i, src) in offered(s).into_iter().enumerate() {
        let p = Vec2::new(
            -574.0 + (i % 2) as f32 * 80.0,
            -164.0 - (i / 2) as f32 * 22.0,
        );
        let on = s.traces.iter().position(|x| *x == src);
        ui.button(
            ButtonSpec::new(p, Vec2::new(76.0, 19.0), src.to_string(), Act::Trace(src))
                .font(11.0)
                .tone(if on.is_some() {
                    Tone::Secondary
                } else {
                    Tone::Ghost
                }),
        );
        if let Some(k) = on {
            ui.dot(p + Vec2::new(-30.0, 0.0), 7.0, channel(k), 12.0);
        }
    }
    // Plot frame and grid.
    let center = ORIGIN + SIZE / 2.0;
    ui.well(center, SIZE + Vec2::new(16.0, 12.0));
    let secs = task.duration.max(DT);
    let every = if secs > 12.0 { 2.0 } else { 1.0 };
    let mut sec = 0.0;
    while sec <= secs + 1e-3 {
        let x = ORIGIN.x + sec / secs * SIZE.x;
        ui.rect(
            Vec2::new(x, center.y),
            Vec2::new(1.0, SIZE.y),
            pal.line.with_alpha(0.10),
            5.6,
        );
        ui.text(
            format!("{sec:.0}s"),
            Vec2::new(x, ORIGIN.y - 13.0),
            9.0,
            pal.text_faint,
            Anchor::CENTER,
        );
        sec += every;
    }
    for f in [0.25, 0.5, 0.75] {
        ui.rect(
            Vec2::new(center.x, ORIGIN.y + f * SIZE.y),
            Vec2::new(SIZE.x, 1.0),
            pal.line.with_alpha(0.08),
            5.6,
        );
    }
    let run = s.report.as_ref().and_then(|r| r.runs.get(s.case));
    let Some(run) = run.filter(|r| !r.samples.is_empty()) else {
        let msg = if s.traces.is_empty() {
            t(
                l,
                "Pick up to four signals on the left.",
                "在左边选最多四路信号。",
            )
        } else {
            t(
                l,
                "Press Run: every signal is recorded, 50 times a second.",
                "按“运行”：每路信号每秒记录 50 次。",
            )
        };
        ui.text(msg, center, 14.0, pal.text_faint, Anchor::CENTER);
        return;
    };
    let samples = &run.samples;
    let range = ranges(samples, &s.traces);
    let plot = ScopePlot {
        duration: secs,
        traces: s
            .traces
            .iter()
            .zip(&range)
            .map(|(&src, &(lo, hi))| (src, lo, hi))
            .collect(),
    };
    // Zero line when every trace shares a scale that crosses it.
    let shared = range.windows(2).all(|w| w[0] == w[1]);
    if shared
        && let Some(&(lo, hi)) = range.first()
        && lo < 0.0
        && hi > 0.0
    {
        ui.rect(
            Vec2::new(center.x, plot.y(0, 0.0)),
            Vec2::new(SIZE.x, 1.0),
            pal.text_faint.with_alpha(0.5),
            5.7,
        );
    }
    let n = samples.len();
    for (k, &src) in s.traces.iter().enumerate() {
        let color = channel(k);
        let pt = |i: usize| {
            let x = &samples[i];
            Vec2::new(plot.x(x.t), plot.y(k, x.read(src)))
        };
        let step = (n / POINTS).max(1);
        let mut prev = pt(0);
        let mut i = step;
        while i < n + step {
            let p = pt(i.min(n - 1));
            if p.distance(prev) > 0.5 {
                ui.line(prev, p, 1.8, color, 6.0 + k as f32 * 0.05);
                prev = p;
            }
            i += step;
        }
        let (lo, hi) = range[k];
        // Legend with the trace's range and its value at the cursor.
        let lx = ORIGIN.x + 4.0 + k as f32 * 200.0;
        let ly = ORIGIN.y + SIZE.y + 16.0;
        ui.dot(Vec2::new(lx, ly), 8.0, color, 12.0);
        ui.text(
            format!("{src}  {}…{}", short(lo), short(hi)),
            Vec2::new(lx + 9.0, ly),
            11.0,
            pal.text_dim,
            Anchor::CENTER_LEFT,
        );
        let e = ui.text(
            "",
            Vec2::new(lx + 190.0, ly),
            12.0,
            color,
            Anchor::CENTER_RIGHT,
        );
        ui.commands.entity(e).insert(ScopeValue(k));
        let e = ui.dot(pt(0), 9.0, color, 8.5);
        ui.commands.entity(e).insert(ScopeDot(k));
    }
    // Where the run ended early.
    if let Some(why) = run.why.filter(|_| s.play.revealed) {
        let x = plot.x(run.end());
        ui.rect(
            Vec2::new(x, center.y),
            Vec2::new(2.0, SIZE.y),
            pal.bad.with_alpha(0.7),
            7.0,
        );
        ui.text(
            format!("× {}", bench::why_text(why, &run.case.rig, l)),
            Vec2::new(x - 6.0, ORIGIN.y + SIZE.y - 10.0),
            11.0,
            pal.bad,
            Anchor::CENTER_RIGHT,
        );
    }
    // A fault's evidence, once the player has pointed at it.
    if s.play.revealed
        && v.game.save.has_evidence(task)
        && let Some(at) = bench::clue_at(s)
    {
        let x = plot.x(at);
        ui.rect(
            Vec2::new(x, center.y),
            Vec2::new(2.0, SIZE.y),
            pal.good.with_alpha(0.8),
            7.0,
        );
        ui.text(
            format!("✓ {}", t(l, "evidence", "证据")),
            Vec2::new(x + 6.0, ORIGIN.y + 10.0),
            11.0,
            pal.good,
            Anchor::CENTER_LEFT,
        );
    }
    let e = ui.rect(
        Vec2::new(ORIGIN.x, center.y),
        Vec2::new(1.5, SIZE.y + 6.0),
        pal.text.with_alpha(0.75),
        8.0,
    );
    ui.commands.entity(e).insert(ScopeCursor);
    ui.commands.spawn((plot, oo_kit::ui::ScreenEntity));
}

/// The sample under the playback cursor.
fn sample(s: &Session) -> Option<&Sample> {
    s.report
        .as_ref()?
        .runs
        .get(s.case)?
        .samples
        .get(s.play.step)
}

/// Moves the cursor and readouts to the playback position, updates the
/// editor's per-row values, and seeks when the plot is clicked.
pub fn sync_cursor(
    mut s: ResMut<Session>,
    cursor: Res<Cursor>,
    mut redraw: ResMut<Redraw>,
    plots: Query<&ScopePlot>,
    mut line: Query<&mut Transform, With<ScopeCursor>>,
    mut dots: Query<(&ScopeDot, &mut Transform), Without<ScopeCursor>>,
    mut values: Query<(&ScopeValue, &mut Text2d), Without<editor::RowValue>>,
    mut rows: Query<(&editor::RowValue, &mut Text2d), Without<ScopeValue>>,
) {
    let plot = plots.single().ok();
    if let Some(plot) = plot
        && cursor.down
        && !cursor.over_button
        && cursor.in_rect(ORIGIN + SIZE / 2.0, SIZE + Vec2::new(8.0, 8.0))
        && let Some(w) = cursor.world
        && s.report.is_some()
    {
        let t = (w.x - ORIGIN.x) / SIZE.x * plot.duration;
        let to = (t / DT).round().max(0.0) as usize;
        if s.play.playing {
            s.play.playing = false;
            redraw.0 = true;
        }
        bench::advance(&mut s, to);
    }
    let s = &*s;
    let now = sample(s);
    if let (Some(plot), Some(x)) = (plot, now) {
        let px = plot.x(x.t);
        for mut tf in &mut line {
            tf.translation.x = px;
        }
        for (dot, mut tf) in &mut dots {
            if let Some(&(src, ..)) = plot.traces.get(dot.0) {
                tf.translation.x = px;
                tf.translation.y = plot.y(dot.0, x.read(src));
            }
        }
        for (val, mut text) in &mut values {
            if let Some(&(src, ..)) = plot.traces.get(val.0) {
                let want = short(x.read(src));
                if text.0 != want {
                    text.0 = want;
                }
            }
        }
    }
    let code = s.report.as_ref().map(|r| &r.code);
    for (row, mut text) in &mut rows {
        let want = match (code.and_then(|c| c.get(row.0)), now) {
            (Some(ins), Some(x)) => format!("= {}", short(x.read(ins.d.into()))),
            _ => String::new(),
        };
        if text.0 != want {
            text.0 = want;
        }
    }
}
