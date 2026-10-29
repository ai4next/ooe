//! The robot on screen. Each rig is drawn as a fixed list of parts (bars,
//! boxes, dots) computed from a pose; replay and the live showcase only move
//! those parts, never respawn them.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::tex::VisualTextures;
use oo_kit::theme::{Palette, mix};
use oo_kit::ui::{ScreenEntity, Ui};
use ooe_core::ctl::Outs;
use ooe_core::lang::Act as Out;
use ooe_core::sim::{
    BEACON_RANGE, BEACONS, CART_HALF, Drive, LEG, LINK, POLE_FALL, POLE_FMAX, POLE_HALF, Pose,
    ROVER_TRACK, ROVER_VMAX, Rig, World,
};

use crate::View;
use crate::state::{Screen, Session, t};

/// Maps world metres to screen pixels.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub origin: Vec2,
    pub scale: f32,
}

impl Frame {
    pub fn p(&self, x: f32, y: f32) -> Vec2 {
        self.origin + Vec2::new(x, y) * self.scale
    }

    fn m(&self, v: f32) -> f32 {
        v * self.scale
    }

    /// The world x at screen x.
    fn x_at(&self, px: f32) -> f32 {
        (px - self.origin.x) / self.scale
    }
}

#[derive(Clone, Copy, Debug)]
enum Shape {
    Seg { a: Vec2, b: Vec2, w: f32 },
    Box { c: Vec2, size: Vec2, rot: f32 },
    Dot { c: Vec2, size: f32 },
    Glow { c: Vec2, size: f32 },
}

#[derive(Clone, Copy, Debug)]
struct Piece {
    shape: Shape,
    color: Color,
    z: f32,
}

fn seg(a: Vec2, b: Vec2, w: f32, color: Color, z: f32) -> Piece {
    Piece {
        shape: Shape::Seg { a, b, w },
        color,
        z,
    }
}

fn bx(c: Vec2, size: Vec2, rot: f32, color: Color, z: f32) -> Piece {
    Piece {
        shape: Shape::Box { c, size, rot },
        color,
        z,
    }
}

fn dot(c: Vec2, size: f32, color: Color, z: f32) -> Piece {
    Piece {
        shape: Shape::Dot { c, size },
        color,
        z,
    }
}

fn glow(c: Vec2, size: f32, color: Color, z: f32) -> Piece {
    Piece {
        shape: Shape::Glow { c, size },
        color,
        z,
    }
}

/// A motor indicator: a bar from `at` along `dir`, `u` in -1..1 of `len` px.
fn bar(at: Vec2, dir: Vec2, u: f32, len: f32, color: Color) -> Piece {
    seg(at, at + dir * u.clamp(-1.0, 1.0) * len, 4.0, color, 8.5)
}

/// Marks a moving part by its index in [`pieces`].
#[derive(Component)]
pub struct Part(usize);

/// The stage on screen: how it maps the world, and the rig's constants.
#[derive(Component)]
pub struct Stage {
    frame: Frame,
    rig: Rig,
    live: bool,
}

/// The replay clock on the bench.
#[derive(Component)]
pub struct Clock;

fn sprite(tex: &VisualTextures, p: &Piece) -> Sprite {
    match p.shape {
        Shape::Seg { .. } => Sprite::from_color(p.color, Vec2::ONE),
        Shape::Box { size, .. } => tex.chip_sprite(p.color, size),
        Shape::Dot { size, .. } => tex.dot_sprite(p.color, size),
        Shape::Glow { size, .. } => tex.glow_sprite(p.color, size),
    }
}

/// Moves a part's sprite to where its piece is now.
fn place(p: &Piece, tf: &mut Transform, sp: &mut Sprite) {
    sp.color = p.color;
    let (at, rot) = match p.shape {
        Shape::Seg { a, b, w } => {
            let d = b - a;
            sp.custom_size = Some(Vec2::new(d.length(), w));
            ((a + b) * 0.5, d.y.atan2(d.x))
        }
        Shape::Box { c, size, rot } => {
            sp.custom_size = Some(size);
            (c, rot)
        }
        Shape::Dot { c, size } | Shape::Glow { c, size } => {
            sp.custom_size = Some(Vec2::splat(size));
            (c, 0.0)
        }
    };
    tf.translation = at.extend(p.z);
    tf.rotation = Quat::from_rotation_z(rot);
}

/// Fits a rig into a box. `wide` leaves room for the showcase's moved targets.
fn frame(rig: &Rig, c: Vec2, size: Vec2, wide: bool) -> Frame {
    let bottom = c.y - size.y / 2.0;
    let fit = |lo: f32, hi: f32, ground: f32| {
        let scale = (size.x - 40.0) / (hi - lo);
        Frame {
            origin: Vec2::new(c.x - (lo + hi) / 2.0 * scale, bottom + ground),
            scale,
        }
    };
    match rig {
        Rig::Cart(k) if k.gravity => Frame {
            origin: Vec2::new(c.x, bottom + 24.0),
            scale: (size.y - 60.0) / k.rail,
        },
        Rig::Cart(k) => {
            let (lo, hi) = if wide && k.wall.is_none() {
                (-k.rail - 0.3, k.rail + 0.3)
            } else {
                let lo = k.x.min(k.target).min(0.0) - 1.0;
                let hi = match k.wall {
                    Some(w) => w + 0.4,
                    None => k.x.max(k.target).max(0.0) + 1.0,
                };
                (lo.max(-k.rail - 0.3), hi.min(k.rail + 0.3))
            };
            fit(lo, hi, size.y * 0.3)
        }
        Rig::Pole(p) => fit(-p.rail - 0.3, p.rail + 0.3, size.y * 0.3),
        Rig::Arm(_) => Frame {
            origin: Vec2::new(c.x - size.x * 0.06, c.y - size.y * 0.12),
            scale: (size.y * 0.62 - 14.0) / (2.0 * LINK),
        },
        Rig::Rover(r) => {
            let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
            for p in &r.line.pts {
                lo = lo.min(Vec2::from(*p));
                hi = hi.max(Vec2::from(*p));
            }
            let (lo, hi) = (lo - 0.4, hi + 0.4);
            let span = hi - lo;
            let scale = ((size.x - 40.0) / span.x).min((size.y - 40.0) / span.y);
            Frame {
                origin: c - (lo + hi) / 2.0 * scale,
                scale,
            }
        }
        Rig::Walker(w) => fit(-0.8, w.goal + 0.8, size.y * 0.2),
        Rig::Tilt(_) => Frame {
            origin: Vec2::new(c.x, bottom + size.y * 0.24),
            scale: size.y * 0.5,
        },
        Rig::Corridor(_) => fit(-0.4, 10.0, size.y * 0.32),
    }
}

fn dir(th: f32) -> Vec2 {
    Vec2::new(th.sin(), th.cos())
}

/// A rig's moving parts at `pose`. The count depends only on the rig type,
/// so [`sync_stage`] can match parts to pieces by index.
fn pieces(f: &Frame, rig: &Rig, pose: Pose, o: &Outs, pal: &Palette, failed: bool) -> Vec<Piece> {
    let body = if failed { pal.bad } else { pal.primary };
    let wheel = pal.text_dim;
    let hot = pal.primary_hot;
    let target = pal.secondary;
    let ghost = pal.tertiary;
    let u = |a: Out| o[a as usize];
    match (pose, rig) {
        (Pose::Cart { x, target: g }, Rig::Cart(k)) if k.gravity => {
            let (pw, ph) = (72.0, 12.0);
            let plat = f.p(0.0, x) + Vec2::new(0.0, ph / 2.0);
            let line = f.p(0.0, g);
            let fmax = match k.drive {
                Drive::Force { fmax, .. } => fmax,
                Drive::Speed { vmax, .. } => vmax,
            };
            vec![
                glow(
                    line + Vec2::new(58.0, 0.0),
                    44.0,
                    target.with_alpha(0.4),
                    6.5,
                ),
                seg(
                    line - Vec2::X * 54.0,
                    line + Vec2::X * 58.0,
                    2.0,
                    target,
                    6.6,
                ),
                seg(
                    f.p(0.0, k.rail),
                    plat + Vec2::Y * ph / 2.0,
                    2.0,
                    pal.line_bright,
                    6.7,
                ),
                bx(plat, Vec2::new(pw, ph), 0.0, body, 8.0),
                bx(
                    plat + Vec2::new(0.0, 16.0),
                    Vec2::new(28.0, 20.0),
                    0.0,
                    ghost,
                    7.9,
                ),
                bar(plat, Vec2::NEG_Y, u(Out::Motor) / fmax, 70.0, hot),
            ]
        }
        (Pose::Cart { x, target: g }, Rig::Cart(k)) => {
            let bw = f.m(2.0 * CART_HALF).max(26.0);
            let bh = 18.0;
            let base = f.p(x, 0.0);
            let c = base + Vec2::new(0.0, 12.0 + bh / 2.0);
            let umax = match k.drive {
                Drive::Speed { vmax, .. } => vmax,
                Drive::Force { fmax, .. } => fmax,
            };
            let eye = c + Vec2::new(bw / 2.0 - 4.0, 2.0);
            let beam = match k.wall {
                Some(w) => (
                    Vec2::new(f.p(w, 0.0).x - 5.0, eye.y),
                    target.with_alpha(0.4),
                ),
                None => (eye, Color::NONE),
            };
            vec![
                glow(f.p(g, 0.0), 54.0, target.with_alpha(0.35), 6.5),
                seg(f.p(g, 0.0), f.p(g, 0.0) + Vec2::Y * 48.0, 2.0, target, 6.6),
                seg(eye, beam.0, 1.5, beam.1, 6.7),
                dot(base + Vec2::new(-bw * 0.3, 6.0), 12.0, wheel, 7.8),
                dot(base + Vec2::new(bw * 0.3, 6.0), 12.0, wheel, 7.8),
                bx(c, Vec2::new(bw, bh), 0.0, body, 8.0),
                dot(eye, 6.0, pal.bg_deep, 8.2),
                bar(
                    c + Vec2::Y * (bh / 2.0 + 10.0),
                    Vec2::X,
                    u(Out::Motor) / umax,
                    bw,
                    hot,
                ),
            ]
        }
        (Pose::Pole { x, th, target: g }, _) => {
            let bw = f.m(0.5).max(30.0);
            let base = f.p(x, 0.0);
            let c = base + Vec2::new(0.0, 21.0);
            let pivot = c + Vec2::Y * 9.0;
            let tip = pivot + dir(th) * f.m(2.0 * POLE_HALF);
            let lean = mix(pal.text, pal.bad, (th.abs() / POLE_FALL).min(1.0));
            vec![
                glow(f.p(g, 0.0), 54.0, target.with_alpha(0.35), 6.5),
                seg(f.p(g, 0.0), f.p(g, 0.0) + Vec2::Y * 48.0, 2.0, target, 6.6),
                dot(base + Vec2::new(-bw * 0.3, 6.0), 12.0, wheel, 7.8),
                dot(base + Vec2::new(bw * 0.3, 6.0), 12.0, wheel, 7.8),
                bx(c, Vec2::new(bw, 18.0), 0.0, body, 8.0),
                seg(pivot, tip, 5.0, lean, 8.3),
                dot(pivot, 10.0, pal.text, 8.4),
                dot(tip, 16.0, lean, 8.4),
                bar(c, Vec2::X, u(Out::Motor) / POLE_FMAX, 46.0, hot),
            ]
        }
        (Pose::Arm { q, target: g }, _) => {
            let base = f.p(0.0, 0.0);
            let at = |a: f32, b: f32| {
                let e = Vec2::new(a.cos(), a.sin()) * LINK;
                (e, e + Vec2::new((a + b).cos(), (a + b).sin()) * LINK)
            };
            let (e, tip) = at(q[0], q[1]);
            let (e, tip) = (f.p(e.x, e.y), f.p(tip.x, tip.y));
            let (ce, ct) = at(u(Out::Shoulder), u(Out::Elbow));
            let (ce, ct) = (f.p(ce.x, ce.y), f.p(ct.x, ct.y));
            let aim = (u(Out::Shoulder) != 0.0 || u(Out::Elbow) != 0.0) as u8 as f32;
            let g = f.p(g[0], g[1]);
            vec![
                glow(g, 60.0, target.with_alpha(0.4), 6.5),
                dot(g, 12.0, target, 6.6),
                seg(tip, g, 1.5, pal.text_faint.with_alpha(0.6), 6.7),
                seg(base, ce, 2.0, ghost.with_alpha(0.35 * aim), 6.8),
                seg(ce, ct, 2.0, ghost.with_alpha(0.35 * aim), 6.8),
                seg(base, e, 10.0, body, 8.0),
                seg(e, tip, 7.0, mix(body, pal.text, 0.25), 8.1),
                dot(base, 22.0, pal.text, 8.3),
                dot(e, 14.0, pal.text, 8.3),
                dot(tip, 10.0, hot, 8.4),
            ]
        }
        (Pose::Rover { x, y, th }, _) => {
            let c = f.p(x, y);
            let fwd = Vec2::new(th.cos(), th.sin());
            let left = fwd.perp();
            let half = f.m(ROVER_TRACK / 2.0).max(8.0);
            let len = (half * 2.4).max(20.0);
            let (wl, wr) = (c + left * half, c - left * half);
            let vmax = ROVER_VMAX;
            vec![
                glow(c, len * 2.6, body.with_alpha(0.25), 6.5),
                bx(wl, Vec2::new(10.0, 4.0), th, wheel, 7.8),
                bx(wr, Vec2::new(10.0, 4.0), th, wheel, 7.8),
                bx(c, Vec2::new(len, half * 1.6), th, body, 8.0),
                dot(c + fwd * (len / 2.0), 7.0, target, 8.2),
                bar(wl, fwd, u(Out::Left) / vmax, 26.0, hot),
                bar(wr, fwd, u(Out::Right) / vmax, 26.0, hot),
            ]
        }
        (Pose::Walker { x, hip, lift }, _) => {
            let h = f.p(x, LEG);
            let foot = |i: usize| {
                let a = hip[i];
                f.p(x + LEG * a.sin(), LEG * (1.0 - a.cos()) + 0.25 * lift[i])
            };
            let shade = |i: usize, c: Color| {
                if lift[i] < 0.5 {
                    c
                } else {
                    mix(c, pal.bg, 0.45)
                }
            };
            let torso = Vec2::new(f.m(0.24).max(12.0), f.m(0.6));
            vec![
                seg(h, foot(1), 7.0, shade(1, ghost), 7.6),
                dot(foot(1), 12.0, shade(1, pal.text_dim), 7.7),
                bx(h + Vec2::Y * (torso.y / 2.0), torso, 0.0, body, 8.0),
                dot(
                    h + Vec2::Y * (torso.y + f.m(0.14)),
                    f.m(0.28).max(14.0),
                    body,
                    8.0,
                ),
                seg(h, foot(0), 7.0, shade(0, mix(body, pal.text, 0.3)), 8.2),
                dot(foot(0), 12.0, shade(0, pal.text), 8.3),
                dot(h, 10.0, pal.text, 8.4),
            ]
        }
        (Pose::Tilt { th, est }, _) => {
            let pivot = f.p(0.0, 0.0);
            let size = Vec2::new(f.m(0.26), f.m(0.9));
            vec![
                bx(
                    pivot + dir(est) * size.y / 2.0,
                    size,
                    -est,
                    ghost.with_alpha(0.3),
                    7.9,
                ),
                bx(pivot + dir(th) * size.y / 2.0, size, -th, body, 8.0),
                dot(pivot + dir(th) * f.m(0.72), 12.0, target, 8.2),
                seg(pivot, pivot + dir(th) * f.m(1.1), 1.5, pal.text_faint, 8.3),
                seg(pivot, pivot + dir(est) * f.m(1.1), 2.5, ghost, 8.4),
                dot(pivot + dir(est) * f.m(1.1), 10.0, ghost, 8.5),
            ]
        }
        (Pose::Corridor { x, est, fix }, _) => {
            let bw = f.m(0.4).max(24.0);
            let (base, eb) = (f.p(x, 0.0), f.p(est, 0.0));
            let c = base + Vec2::new(0.0, 20.0);
            let top = 56.0;
            vec![
                glow(
                    c,
                    90.0,
                    target.with_alpha(if fix { 0.55 } else { 0.0 }),
                    6.5,
                ),
                bx(
                    eb + Vec2::Y * 20.0,
                    Vec2::new(bw, 16.0),
                    0.0,
                    ghost.with_alpha(0.35),
                    7.0,
                ),
                seg(eb, eb + Vec2::Y * top, 2.0, ghost, 7.1),
                seg(
                    base + Vec2::Y * top,
                    eb + Vec2::Y * top,
                    2.0,
                    pal.warn.with_alpha(0.7),
                    7.2,
                ),
                dot(base + Vec2::new(-bw * 0.3, 6.0), 12.0, wheel, 7.8),
                dot(base + Vec2::new(bw * 0.3, 6.0), 12.0, wheel, 7.8),
                bx(c, Vec2::new(bw, 16.0), 0.0, body, 8.0),
            ]
        }
        _ => Vec::new(),
    }
}

/// The pose on screen now: the live world, the replayed sample, or the
/// case's starting pose before anything has run.
fn now(s: &Session, live: bool) -> (Pose, Outs, bool) {
    if live && let Some(w) = &s.live.world {
        return (w.pose(), s.live.outs, false);
    }
    if !live
        && let Some(run) = s.report.as_ref().and_then(|r| r.runs.get(s.case))
        && let Some(x) = run.samples.get(s.play.step)
    {
        let failed = !run.ok() && s.play.step + 1 == run.samples.len();
        return (x.pose, x.outs, failed);
    }
    let pose = s
        .cases
        .get(s.case)
        .map_or(Pose::Tilt { th: 0.0, est: 0.0 }, |c| {
            World::new(c.rig.clone(), c.real, Vec::new(), c.seed).pose()
        });
    (pose, [0.0; Out::COUNT], false)
}

/// The case the stage frames: the selected one, or the showcase's first.
fn rig(s: &Session, live: bool) -> Option<&Rig> {
    let i = if live { 0 } else { s.case };
    s.cases.get(i).map(|c| &c.rig)
}

/// Ground line with a tick each metre and a label every `every` metres.
fn ruler(ui: &mut Ui, f: &Frame, c: Vec2, size: Vec2, every: i32) {
    let pal = ui.pal;
    let (l, r) = (c.x - size.x / 2.0 + 10.0, c.x + size.x / 2.0 - 10.0);
    let y = f.origin.y;
    ui.line(Vec2::new(l, y), Vec2::new(r, y), 2.0, pal.line_bright, 6.0);
    let (lo, hi) = (f.x_at(l).ceil() as i32, f.x_at(r).floor() as i32);
    for m in lo..=hi {
        let x = f.p(m as f32, 0.0).x;
        ui.line(
            Vec2::new(x, y),
            Vec2::new(x, y - 6.0),
            1.0,
            pal.line_bright,
            6.0,
        );
        if m % every == 0 {
            ui.text(
                format!("{m}"),
                Vec2::new(x, y - 16.0),
                10.0,
                pal.text_faint,
                Anchor::CENTER,
            );
        }
    }
}

/// What does not move: track, walls, the line, beacons, the shaft.
fn draw_static(ui: &mut Ui, f: &Frame, rig: &Rig, c: Vec2, size: Vec2) {
    let pal = ui.pal;
    match rig {
        Rig::Cart(k) if k.gravity => {
            let (floor, ceil) = (f.p(0.0, 0.0), f.p(0.0, k.rail));
            for sx in [-50.0, 50.0] {
                let a = Vec2::new(floor.x + sx, floor.y);
                ui.line(a, Vec2::new(a.x, ceil.y), 2.0, pal.line, 6.0);
            }
            ui.line(
                floor - Vec2::X * 80.0,
                floor + Vec2::X * 80.0,
                3.0,
                pal.line_bright,
                6.0,
            );
            ui.line(
                ceil - Vec2::X * 60.0,
                ceil + Vec2::X * 60.0,
                3.0,
                pal.warn,
                6.0,
            );
            for i in 0..=(k.rail * 2.0) as i32 {
                let y = f.p(0.0, i as f32 * 0.5).y;
                let w = if i % 2 == 0 { 10.0 } else { 5.0 };
                ui.line(
                    Vec2::new(floor.x - 50.0 - w, y),
                    Vec2::new(floor.x - 50.0, y),
                    1.0,
                    pal.line_bright,
                    6.0,
                );
                // The floor line already marks 0 m.
                if i % 2 == 0 && i > 0 {
                    ui.text(
                        format!("{} m", i / 2),
                        Vec2::new(floor.x - 66.0, y),
                        10.0,
                        pal.text_faint,
                        Anchor::CENTER_RIGHT,
                    );
                }
            }
        }
        Rig::Cart(k) => {
            ruler(ui, f, c, size, 1);
            for end in [-k.rail, k.rail] {
                let p = f.p(end, 0.0);
                if (p.x - c.x).abs() < size.x / 2.0 - 8.0 {
                    ui.rect(p + Vec2::Y * 8.0, Vec2::new(4.0, 16.0), pal.warn, 6.1);
                }
            }
            if let Some(w) = k.wall {
                let p = f.p(w, 0.0);
                let wall = Vec2::new(12.0, 96.0);
                ui.rect(
                    p + Vec2::new(wall.x / 2.0, wall.y / 2.0),
                    wall,
                    pal.line_bright,
                    6.2,
                );
                ui.rect(
                    p + Vec2::new(1.0, wall.y / 2.0),
                    Vec2::new(2.0, wall.y),
                    pal.text_dim,
                    6.3,
                );
            }
        }
        Rig::Pole(p) => {
            ruler(ui, f, c, size, 1);
            let (a, b) = (f.p(-p.rail, 0.0), f.p(p.rail, 0.0));
            ui.line(a, b, 4.0, pal.line_bright, 6.1);
            for p in [a, b] {
                ui.rect(p + Vec2::Y * 9.0, Vec2::new(4.0, 18.0), pal.warn, 6.2);
            }
        }
        Rig::Arm(_) => {
            let base = f.p(0.0, 0.0);
            for i in -2..=4 {
                for j in -1..=4 {
                    let p = f.p(i as f32 * 0.5, j as f32 * 0.5);
                    if (p - c).abs().cmplt(size / 2.0 - 8.0).all() {
                        ui.dot(p, 3.0, pal.line_bright, 5.6);
                    }
                }
            }
            ui.rect(
                base - Vec2::Y * 14.0,
                Vec2::new(44.0, 16.0),
                pal.line_bright,
                6.0,
            );
        }
        Rig::Rover(r) => {
            let pts: Vec<Vec2> = r.line.pts.iter().map(|p| f.p(p[0], p[1])).collect();
            for w in pts.chunks(3).collect::<Vec<_>>().windows(2) {
                ui.line(w[0][0], w[1][0], 4.0, pal.text_dim, 6.0);
            }
            if let (Some(a), Some(b)) = (pts.first(), pts.last()) {
                ui.line(*w_last(&pts, 3), *b, 4.0, pal.text_dim, 6.0);
                ui.rect(*a, Vec2::new(4.0, 22.0), pal.text_faint, 6.1);
                ui.rect(*b, Vec2::new(5.0, 26.0), pal.good, 6.1);
            }
        }
        Rig::Walker(w) => {
            ruler(ui, f, c, size, 1);
            let g = f.p(w.goal, 0.0);
            ui.line(g, g + Vec2::Y * 80.0, 2.0, pal.text_dim, 6.1);
            ui.rect(
                g + Vec2::new(14.0, 72.0),
                Vec2::new(26.0, 16.0),
                pal.good,
                6.2,
            );
        }
        Rig::Tilt(_) => {
            let pivot = f.p(0.0, 0.0);
            ui.line(
                pivot - Vec2::X * 120.0,
                pivot + Vec2::X * 120.0,
                2.0,
                pal.line_bright,
                6.0,
            );
            ui.rect(
                pivot - Vec2::Y * 6.0,
                Vec2::new(36.0, 12.0),
                pal.line_bright,
                6.1,
            );
            for i in -3..=3 {
                let p = pivot + dir(i as f32 * 0.2) * f.m(1.18);
                ui.dot(p, if i == 0 { 6.0 } else { 4.0 }, pal.text_faint, 6.0);
            }
        }
        Rig::Corridor(_) => {
            ruler(ui, f, c, size, 2);
            for b in BEACONS {
                let p = f.p(b, 0.0);
                ui.rect(
                    p + Vec2::Y * 3.0,
                    Vec2::new(f.m(2.0 * BEACON_RANGE), 6.0),
                    pal.secondary.with_alpha(0.25),
                    6.1,
                );
                ui.line(p, p + Vec2::Y * 76.0, 2.0, pal.line_bright, 6.2);
                ui.dot(p + Vec2::Y * 78.0, 10.0, pal.secondary, 6.3);
            }
        }
    }
}

/// The last point a `chunks(n)` walk stopped at.
fn w_last(pts: &[Vec2], n: usize) -> &Vec2 {
    &pts[(pts.len() - 1) / n * n]
}

/// The bench's view of the robot, inside the right card.
pub const BENCH: (Vec2, Vec2) = (Vec2::new(421.0, 152.0), Vec2::new(398.0, 252.0));

pub fn draw_bench_stage(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let (c, size) = BENCH;
    ui.well(c, size);
    let l = v.lang;
    let view = match rig(v.s, false) {
        Some(Rig::Rover(_)) => t(l, "TOP VIEW", "俯视"),
        Some(Rig::Tilt(_)) => t(l, "IMU · TILT", "惯性测量 · 倾角"),
        _ => t(l, "SIDE VIEW", "侧视"),
    };
    ui.caption(
        view,
        Vec2::new(c.x - size.x / 2.0, c.y + size.y / 2.0 + 12.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    draw_stage(ui, v, c, size, false);
}

/// Draws the stage for the bench (`live = false`) or the showcase.
pub fn draw_stage(ui: &mut Ui, v: &View, c: Vec2, size: Vec2, live: bool) {
    let pal = ui.pal;
    let s = v.s;
    let Some(rig) = rig(s, live) else { return };
    let f = frame(rig, c, size, live);
    draw_static(ui, &f, rig, c, size);
    let (pose, outs, failed) = now(s, live);
    for (i, p) in pieces(&f, rig, pose, &outs, &pal, failed)
        .iter()
        .enumerate()
    {
        let mut tf = Transform::default();
        let mut sp = sprite(&ui.assets.tex, p);
        place(p, &mut tf, &mut sp);
        ui.commands.spawn((sp, tf, Part(i), ScreenEntity));
    }
    ui.commands.spawn((
        Stage {
            frame: f,
            rig: rig.clone(),
            live,
        },
        ScreenEntity,
    ));
    let top = c.y + size.y / 2.0 - 16.0;
    let right = c.x + size.x / 2.0 - 16.0;
    if !matches!(rig, Rig::Tilt(_)) {
        let m = if f.scale < 30.0 { 2.0 } else { 1.0 };
        let a = Vec2::new(right - f.m(m), top);
        ui.line(a, Vec2::new(right, top), 2.0, pal.text_faint, 6.0);
        for x in [a.x, right] {
            ui.line(
                Vec2::new(x, top - 4.0),
                Vec2::new(x, top + 4.0),
                1.0,
                pal.text_faint,
                6.0,
            );
        }
        ui.text(
            format!("{m} m"),
            Vec2::new(a.x - 8.0, top),
            10.0,
            pal.text_faint,
            Anchor::CENTER_RIGHT,
        );
    }
    let l = v.lang;
    let mut keys = Vec::new();
    match rig {
        Rig::Tilt(_) | Rig::Corridor(_) => {
            keys.push((pal.primary, t(l, "truth", "真值")));
            keys.push((pal.tertiary, t(l, "your est", "你的 est")));
        }
        Rig::Arm(_) => {
            keys.push((pal.secondary, t(l, "target", "目标")));
            keys.push((pal.tertiary, t(l, "commanded", "指令姿态")));
        }
        Rig::Walker(_) => keys.push((
            pal.text_faint,
            t(l, "faded foot = lifted", "变暗的脚 = 抬起"),
        )),
        Rig::Rover(_) => keys.push((pal.primary_hot, t(l, "wheel speed", "轮速"))),
        Rig::Cart(_) | Rig::Pole(_) => {
            keys.push((pal.secondary, t(l, "target", "目标")));
            keys.push((pal.primary_hot, t(l, "motor", "电机")));
        }
    }
    if matches!(rig, Rig::Corridor(_)) {
        keys.push((pal.secondary, t(l, "beacon", "信标")));
    }
    let mut x = c.x - size.x / 2.0 + 16.0;
    let y = c.y - size.y / 2.0 + 14.0;
    for (color, label) in keys {
        ui.dot(Vec2::new(x, y), 8.0, color, 6.5);
        ui.text(
            label,
            Vec2::new(x + 9.0, y),
            11.0,
            pal.text_dim,
            Anchor::CENTER_LEFT,
        );
        // CJK glyphs are about twice as wide as Latin ones at this size.
        let w: f32 = label
            .chars()
            .map(|ch| if ch.is_ascii() { 6.2 } else { 11.5 })
            .sum();
        x += 22.0 + w;
    }
}

/// Moves the robot's parts to the current pose, and updates the clock.
pub fn sync_stage(
    s: Res<Session>,
    pal: Res<Palette>,
    screen: Res<State<Screen>>,
    stages: Query<&Stage>,
    mut parts: Query<(&Part, &mut Transform, &mut Sprite)>,
    mut clock: Query<&mut Text2d, With<Clock>>,
) {
    if let Ok(stage) = stages.single() {
        let (pose, outs, failed) = now(&s, stage.live);
        let ps = pieces(&stage.frame, &stage.rig, pose, &outs, &pal, failed);
        for (part, mut tf, mut sp) in &mut parts {
            if let Some(p) = ps.get(part.0) {
                place(p, &mut tf, &mut sp);
            }
        }
    }
    if *screen.get() != Screen::Bench {
        return;
    }
    let total = s.task().duration;
    let at = s
        .report
        .as_ref()
        .and_then(|r| r.runs.get(s.case))
        .and_then(|r| r.samples.get(s.play.step))
        .map_or(0.0, |x| x.t);
    for mut text in &mut clock {
        let want = format!("t  {at:.2} / {total:.0} s");
        if text.0 != want {
            text.0 = want;
        }
    }
}
