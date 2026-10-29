//! The robots. Each rig is a small fixed-step model, honest enough that the
//! classic control lessons (overshoot, sag, noise, latency, falling) appear
//! on their own rather than being scripted.

use std::f32::consts::PI;

use oo_base::Rng;

use crate::ctl::{Outs, Senses};
use crate::lang::{Act, DT, Sensor};

/// Physics steps per control tick.
pub const SUB: usize = 5;
pub const G: f32 = 9.81;

/// How a cart's motor command acts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drive {
    /// A speed servo: the wheels reach `motor` m/s with time constant `tau`.
    Speed { tau: f32, vmax: f32 },
    /// A raw force in newtons.
    Force { mass: f32, friction: f32, fmax: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cart {
    pub x: f32,
    pub v: f32,
    pub drive: Drive,
    pub target: f32,
    /// Pulls towards -x (a lift); the floor at 0 holds it.
    pub gravity: bool,
    /// Range-finder wall ahead (+x), metres.
    pub wall: Option<f32>,
    /// Track ends at ±rail (cart) or ceiling at rail (lift).
    pub rail: f32,
    /// Docked: near target and still (shown to reward programs).
    pub tol: f32,
}

pub const CART_HALF: f32 = 0.2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pole {
    pub x: f32,
    pub v: f32,
    pub th: f32,
    pub w: f32,
    pub pole_mass: f32,
    pub cart_mass: f32,
    /// Where the cart should carry the pole (read as `target`).
    pub target: f32,
    /// Track ends at ±rail.
    pub rail: f32,
    /// Lean that counts as fallen; infinite for a pole free to swing round.
    pub fall: f32,
}

pub const POLE_HALF: f32 = 0.5;
pub const POLE_FMAX: f32 = 20.0;
pub const POLE_FALL: f32 = 0.7;
pub const POLE_RAIL: f32 = 2.4;

/// Where an arm's target is over time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Path {
    Fixed([f32; 2]),
    /// Lissajous figure: centre, radii, angular rates.
    Figure {
        c: [f32; 2],
        r: [f32; 2],
        w: [f32; 2],
    },
}

impl Path {
    pub fn at(self, t: f32) -> [f32; 2] {
        match self {
            Path::Fixed(p) => p,
            Path::Figure { c, r, w } => [
                c[0] + r[0] * (w[0] * t).sin(),
                c[1] + r[1] * (w[1] * t).cos(),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arm {
    pub q: [f32; 2],
    pub path: Path,
    /// Target nudged by the player in the showcase.
    pub nudge: [f32; 2],
}

pub const LINK: f32 = 1.0;
const SERVO_K: f32 = 8.0;
const SERVO_W: f32 = 4.0;

impl Arm {
    pub fn elbow(&self) -> [f32; 2] {
        [LINK * self.q[0].cos(), LINK * self.q[0].sin()]
    }

    pub fn tip(&self) -> [f32; 2] {
        let e = self.elbow();
        let a = self.q[0] + self.q[1];
        [e[0] + LINK * a.cos(), e[1] + LINK * a.sin()]
    }

    pub fn target(&self, t: f32) -> [f32; 2] {
        let p = self.path.at(t);
        [p[0] + self.nudge[0], p[1] + self.nudge[1]]
    }
}

/// A line to follow, sampled as a polyline.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub pts: Vec<[f32; 2]>,
    /// Cumulative length at each point.
    pub len: Vec<f32>,
}

impl Line {
    /// `y = amp·sin(k·x)` for `x` in `0..length`.
    pub fn wave(length: f32, amp: f32, k: f32) -> Self {
        let n = (length / 0.05) as usize;
        let pts: Vec<[f32; 2]> = (0..=n)
            .map(|i| {
                let x = i as f32 * length / n as f32;
                [x, amp * (k * x).sin()]
            })
            .collect();
        let mut len = vec![0.0];
        for w in pts.windows(2) {
            let d = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
            len.push(len.last().unwrap() + d);
        }
        Self { pts, len }
    }

    pub fn total(&self) -> f32 {
        *self.len.last().unwrap_or(&0.0)
    }

    /// Nearest point from segment `from` onwards: (segment, progress, signed
    /// offset — positive when the point is left of the line, tangent angle).
    pub fn locate(&self, p: [f32; 2], from: usize) -> (usize, f32, f32, f32) {
        let mut best = (from, 0.0, f32::MAX, 0.0);
        let end = (from + 40).min(self.pts.len() - 1);
        for i in from.saturating_sub(4)..end {
            let (a, b) = (self.pts[i], self.pts[i + 1]);
            let d = [b[0] - a[0], b[1] - a[1]];
            let l2 = d[0] * d[0] + d[1] * d[1];
            let u = (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0);
            let q = [a[0] + u * d[0], a[1] + u * d[1]];
            let dist = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt();
            if dist < best.2.abs() {
                let side = d[0] * (p[1] - a[1]) - d[1] * (p[0] - a[0]);
                let s = self.len[i] + u * l2.sqrt();
                best = (i, s, dist.copysign(side), d[1].atan2(d[0]));
            }
        }
        (best.0, best.1, best.2, best.3)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rover {
    pub x: f32,
    pub y: f32,
    pub th: f32,
    pub wheels: [f32; 2],
    pub line: Line,
    pub seg: usize,
    pub progress: f32,
    pub offset: f32,
    pub tangent: f32,
    pub finished: bool,
}

pub const ROVER_TRACK: f32 = 0.3;
pub const ROVER_VMAX: f32 = 1.5;
pub const ROVER_LOST: f32 = 0.35;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Walker {
    pub x: f32,
    pub v: f32,
    /// Hip angles, forward positive.
    pub hip: [f32; 2],
    /// 0 = foot down, 1 = foot fully raised.
    pub lift: [f32; 2],
    pub goal: f32,
    /// Seconds with no foot on the ground.
    pub air: f32,
}

pub const LEG: f32 = 1.0;
pub const HIP_MAX: f32 = 0.7;
const HIP_W: f32 = 6.0;
const LIFT_TAU: f32 = 0.05;
const AIR_MAX: f32 = 0.15;

impl Walker {
    pub fn stance(&self, i: usize) -> bool {
        self.lift[i] < 0.5
    }

    pub fn foot(&self, i: usize) -> [f32; 2] {
        let h = self.hip[i];
        [
            self.x + LEG * h.sin(),
            LEG * (1.0 - h.cos()) + 0.25 * self.lift[i],
        ]
    }
}

/// An IMU strapped to a robot that rocks back and forth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tilt {
    pub amp: [f32; 2],
    pub freq: [f32; 2],
    pub phase: f32,
    pub gyro_bias: f32,
    pub accel_noise: f32,
}

impl Tilt {
    pub fn truth(&self, t: f32) -> (f32, f32) {
        let a = self.freq[0] * t + self.phase;
        let b = self.freq[1] * t;
        let th = self.amp[0] * a.sin() + self.amp[1] * b.sin();
        let w = self.amp[0] * self.freq[0] * a.cos() + self.amp[1] * self.freq[1] * b.cos();
        (th, w)
    }
}

/// A robot rolling down a corridor with odometry and a few beacons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corridor {
    pub x: f32,
    pub v: f32,
    pub phase: f32,
    /// Wheel-size error: odometry reads `v·(1+scale)`.
    pub scale: f32,
}

pub const BEACONS: [f32; 4] = [1.6, 4.1, 6.7, 9.2];
pub const BEACON_RANGE: f32 = 0.25;

impl Corridor {
    pub fn speed(&self, t: f32) -> f32 {
        0.65 + 0.3 * (0.8 * t + self.phase).sin()
    }

    pub fn beacon(&self) -> Option<f32> {
        BEACONS
            .iter()
            .copied()
            .find(|b| (self.x - b).abs() < BEACON_RANGE)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Rig {
    Cart(Cart),
    Pole(Pole),
    Arm(Arm),
    Rover(Rover),
    Walker(Walker),
    Tilt(Tilt),
    Corridor(Corridor),
}

/// Ways a run ends early.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fail {
    /// Hit the wall or the ceiling.
    Crash,
    /// Ran off the end of the track.
    OffTrack,
    Fell,
    /// Lost the line.
    Lost,
}

/// What separates the simulator from a real robot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Real {
    /// Ticks between a command and the motor acting on it.
    pub lag: u8,
    /// Standard deviation of position/angle sensor noise.
    pub noise: f32,
    /// Constant gyro / velocity sensor bias.
    pub bias: f32,
    /// Mass multiplier.
    pub mass: f32,
    /// Commands smaller than this do nothing (static friction).
    pub deadzone: f32,
    /// Motor strength multiplier.
    pub gain: f32,
}

impl Default for Real {
    fn default() -> Self {
        Self {
            lag: 0,
            noise: 0.0,
            bias: 0.0,
            mass: 1.0,
            deadzone: 0.0,
            gain: 1.0,
        }
    }
}

/// A snapshot for drawing and replay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pose {
    Cart {
        x: f32,
        target: f32,
    },
    Pole {
        x: f32,
        th: f32,
        target: f32,
    },
    Arm {
        q: [f32; 2],
        target: [f32; 2],
    },
    Rover {
        x: f32,
        y: f32,
        th: f32,
    },
    Walker {
        x: f32,
        hip: [f32; 2],
        lift: [f32; 2],
    },
    Tilt {
        th: f32,
        est: f32,
    },
    Corridor {
        x: f32,
        est: f32,
        fix: bool,
    },
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(2.0 * PI) - PI
}

/// A pole's lean from upright, within ±π (untouched when already there).
fn lean(th: f32) -> f32 {
    if th.abs() <= PI { th } else { wrap(th) }
}

fn out(o: &Outs, a: Act) -> f32 {
    o[a as usize]
}

/// A rig plus the world around it: time, sensor noise, command latency and
/// scheduled pushes.
#[derive(Clone, Debug)]
pub struct World {
    pub rig: Rig,
    pub t: f32,
    pub real: Real,
    /// (time, impulse): velocity kicks, applied once each.
    pub pushes: Vec<(f32, f32)>,
    /// (time, target): the cart's or pole's target moves there.
    pub moves: Vec<(f32, f32)>,
    pub fail: Option<Fail>,
    /// Integral of |command| over time.
    pub effort: f32,
    /// The estimate tasks' last `est` output, and the truth it estimates
    /// (as it was when the sensors were read).
    pub est: f32,
    pub truth: f32,
    rng: Rng,
    queue: Vec<Outs>,
}

impl World {
    pub fn new(rig: Rig, real: Real, pushes: Vec<(f32, f32)>, seed: u64) -> Self {
        let truth = match &rig {
            Rig::Tilt(k) => k.truth(0.0).0,
            Rig::Corridor(c) => c.x,
            _ => 0.0,
        };
        Self {
            rig,
            t: 0.0,
            real,
            pushes,
            moves: Vec::new(),
            fail: None,
            effort: 0.0,
            est: 0.0,
            truth,
            rng: Rng::new(seed ^ 0x9e37_79b9_7f4a_7c15),
            queue: Vec::new(),
        }
    }

    fn noisy(&mut self, v: f32, sd: f32) -> f32 {
        if sd > 0.0 {
            v + sd * self.rng.normal() as f32
        } else {
            v
        }
    }

    /// The main error signal: what the task is trying to drive to zero.
    pub fn error(&self) -> f32 {
        match &self.rig {
            Rig::Cart(c) => c.target - c.x,
            Rig::Pole(p) => lean(p.th),
            Rig::Arm(a) => {
                let (p, g) = (a.tip(), a.target(self.t));
                ((p[0] - g[0]).powi(2) + (p[1] - g[1]).powi(2)).sqrt()
            }
            Rig::Rover(r) => r.offset,
            Rig::Walker(w) => w.goal - w.x,
            Rig::Tilt(_) | Rig::Corridor(_) => self.est - self.truth,
        }
    }

    pub fn docked(&self) -> bool {
        match &self.rig {
            Rig::Cart(c) => (c.target - c.x).abs() < c.tol && c.v.abs() < 0.05,
            Rig::Pole(p) => lean(p.th).abs() < 0.05 && (p.x - p.target).abs() < 0.3,
            Rig::Rover(r) => r.finished,
            Rig::Walker(w) => w.x >= w.goal,
            _ => false,
        }
    }

    pub fn pose(&self) -> Pose {
        match &self.rig {
            Rig::Cart(c) => Pose::Cart {
                x: c.x,
                target: c.target,
            },
            Rig::Pole(p) => Pose::Pole {
                x: p.x,
                th: lean(p.th),
                target: p.target,
            },
            Rig::Arm(a) => Pose::Arm {
                q: a.q,
                target: a.target(self.t),
            },
            Rig::Rover(r) => Pose::Rover {
                x: r.x,
                y: r.y,
                th: r.th,
            },
            Rig::Walker(w) => Pose::Walker {
                x: w.x,
                hip: w.hip,
                lift: w.lift,
            },
            Rig::Tilt(_) => Pose::Tilt {
                th: self.truth,
                est: self.est,
            },
            Rig::Corridor(c) => Pose::Corridor {
                x: c.x,
                est: self.est,
                fix: c.beacon().is_some(),
            },
        }
    }
}

impl World {
    /// Reads every sensor for this tick. Sensors a rig does not have read 0.
    pub fn senses(&mut self) -> Senses {
        let mut s = [0.0; Sensor::COUNT];
        s[Sensor::T as usize] = self.t;
        s[Sensor::Dt as usize] = DT;
        s[Sensor::Fell as usize] = f32::from(self.fail.is_some());
        s[Sensor::Dock as usize] = f32::from(self.docked());
        let (noise, bias) = (self.real.noise, self.real.bias);
        let t = self.t;
        let mut set = |k: Sensor, v: f32| s[k as usize] = v;
        match self.rig.clone() {
            Rig::Cart(c) => {
                set(Sensor::Pos, self.noisy(c.x, noise));
                set(Sensor::Vel, self.noisy(c.v + bias, noise));
                set(Sensor::Target, c.target);
                if let Some(w) = c.wall {
                    set(
                        Sensor::Dist,
                        self.noisy(w - c.x - CART_HALF, noise.max(0.003)),
                    );
                }
            }
            Rig::Pole(p) => {
                set(Sensor::Pos, self.noisy(p.x, noise));
                set(Sensor::Vel, self.noisy(p.v, noise));
                set(Sensor::Angle, self.noisy(lean(p.th), noise));
                set(Sensor::Gyro, self.noisy(p.w + bias, noise * 4.0));
                set(Sensor::Target, p.target);
            }
            Rig::Arm(a) => {
                let g = a.target(t);
                set(Sensor::Q1, self.noisy(a.q[0], noise));
                set(Sensor::Q2, self.noisy(a.q[1], noise));
                set(Sensor::Tx, g[0]);
                set(Sensor::Ty, g[1]);
            }
            Rig::Rover(r) => {
                set(Sensor::Offset, self.noisy(r.offset, noise.max(0.002)));
                set(Sensor::Heading, wrap(r.th - r.tangent));
                set(Sensor::Vel, (r.wheels[0] + r.wheels[1]) / 2.0);
            }
            Rig::Walker(w) => {
                set(Sensor::Pos, w.x);
                set(Sensor::Vel, w.v);
                set(Sensor::Target, w.goal);
                set(Sensor::Q1, w.hip[0]);
                set(Sensor::Q2, w.hip[1]);
            }
            Rig::Tilt(k) => {
                let (th, w) = k.truth(t);
                let shake = 0.6 * (37.0 * t).sin() * k.accel_noise;
                let a = self.noisy(th + shake, k.accel_noise);
                set(Sensor::Angle, a);
                set(Sensor::Accel, a);
                set(Sensor::Gyro, self.noisy(w + k.gyro_bias, 0.02));
            }
            Rig::Corridor(c) => {
                set(Sensor::Odom, self.noisy(c.v * (1.0 + c.scale), 0.02));
                if let Some(b) = c.beacon() {
                    set(Sensor::Fix, 1.0);
                    set(Sensor::Beacon, b);
                    set(Sensor::Dist, self.noisy(b - c.x, 0.01));
                }
            }
        }
        s
    }

    /// Applies one tick of commands (after the rig's latency) and advances
    /// the physics by [`DT`].
    pub fn step(&mut self, cmd: Outs) {
        self.est = out(&cmd, Act::Est);
        self.truth = match &self.rig {
            Rig::Tilt(k) => k.truth(self.t).0,
            Rig::Corridor(c) => c.x,
            _ => 0.0,
        };
        self.queue.push(cmd);
        let o = if self.queue.len() > self.real.lag as usize {
            self.queue.remove(0)
        } else {
            [0.0; Act::COUNT]
        };
        let r = self.real;
        let shape = |u: f32| {
            if u.abs() < r.deadzone {
                0.0
            } else {
                u * r.gain
            }
        };
        let motor = shape(out(&o, Act::Motor));
        let h = DT / SUB as f32;
        let t0 = self.t;
        let kick: f32 = self
            .pushes
            .iter()
            .filter(|(at, _)| *at >= t0 && *at < t0 + DT)
            .map(|p| p.1)
            .sum();
        let moved = self
            .moves
            .iter()
            .rfind(|(at, _)| *at >= t0 && *at < t0 + DT)
            .map(|m| m.1);
        if let Some(g) = moved {
            match &mut self.rig {
                Rig::Cart(c) => c.target = g,
                Rig::Pole(p) => p.target = g,
                _ => {}
            }
        }
        let mut effort = 0.0;
        match &mut self.rig {
            Rig::Cart(c) => {
                c.v += kick;
                for _ in 0..SUB {
                    match c.drive {
                        Drive::Speed { tau, vmax } => {
                            let want = motor.clamp(-vmax, vmax);
                            c.v += (want - c.v) * h / tau;
                            effort += want.abs() * h;
                        }
                        Drive::Force {
                            mass,
                            friction,
                            fmax,
                        } => {
                            let f = motor.clamp(-fmax, fmax);
                            let m = mass * r.mass;
                            let g = if c.gravity { m * G } else { 0.0 };
                            c.v += (f - friction * c.v - g) / m * h;
                            effort += f.abs() * h;
                        }
                    }
                    c.x += c.v * h;
                    if c.gravity && c.x <= 0.0 {
                        c.x = 0.0;
                        c.v = c.v.max(0.0);
                    }
                }
                let hit = c.wall.is_some_and(|w| c.x + CART_HALF >= w);
                if hit || (c.gravity && c.x > c.rail) {
                    self.fail = Some(Fail::Crash);
                } else if !c.gravity && c.x.abs() > c.rail {
                    self.fail = Some(Fail::OffTrack);
                }
            }
            Rig::Pole(p) => {
                p.w += kick;
                let f = motor.clamp(-POLE_FMAX, POLE_FMAX);
                let (m, mc) = (p.pole_mass * r.mass, p.cart_mass * r.mass);
                let mt = m + mc;
                for _ in 0..SUB {
                    let (s, c) = p.th.sin_cos();
                    let tmp = (f + m * POLE_HALF * p.w * p.w * s) / mt;
                    let alpha = (G * s - c * tmp) / (POLE_HALF * (4.0 / 3.0 - m * c * c / mt));
                    let acc = tmp - m * POLE_HALF * alpha * c / mt;
                    p.v += acc * h;
                    p.x += p.v * h;
                    p.w += alpha * h;
                    p.th += p.w * h;
                }
                effort = f.abs() * DT;
                if p.th.abs() > p.fall {
                    self.fail = Some(Fail::Fell);
                } else if p.x.abs() > p.rail {
                    self.fail = Some(Fail::OffTrack);
                }
            }
            Rig::Arm(a) => {
                let cmd = [out(&o, Act::Shoulder), out(&o, Act::Elbow)];
                let wmax = SERVO_W * r.gain;
                for _ in 0..SUB {
                    for (q, c) in a.q.iter_mut().zip(cmd) {
                        let w = (SERVO_K * (c - *q)).clamp(-wmax, wmax);
                        *q += w * h;
                        effort += w.abs() * h;
                    }
                }
            }
            Rig::Rover(v) => {
                let vmax = ROVER_VMAX * r.gain;
                let cmd = [shape(out(&o, Act::Left)), shape(out(&o, Act::Right))];
                for _ in 0..SUB {
                    for (wh, c) in v.wheels.iter_mut().zip(cmd) {
                        let want = c.clamp(-vmax, vmax);
                        *wh += (want - *wh) * h / 0.1;
                    }
                    let fwd = (v.wheels[0] + v.wheels[1]) / 2.0;
                    let turn = (v.wheels[1] - v.wheels[0]) / ROVER_TRACK;
                    v.th += turn * h;
                    v.x += fwd * v.th.cos() * h;
                    v.y += fwd * v.th.sin() * h;
                }
                let (seg, s, off, tan) = v.line.locate([v.x, v.y], v.seg);
                (v.seg, v.offset, v.tangent) = (seg, off, tan);
                v.progress = v.progress.max(s);
                if v.progress >= v.line.total() - 0.05 {
                    v.finished = true;
                }
                if off.abs() > ROVER_LOST && !v.finished {
                    self.fail = Some(Fail::Lost);
                }
            }
            Rig::Walker(w) => {
                let hip = [out(&o, Act::HipL), out(&o, Act::HipR)];
                let lift = [out(&o, Act::LiftL), out(&o, Act::LiftR)];
                let x0 = w.x;
                for _ in 0..SUB {
                    let mut moves = Vec::new();
                    for j in 0..2 {
                        let want = hip[j].clamp(-HIP_MAX, HIP_MAX);
                        let dq = (want - w.hip[j]).clamp(-HIP_W * h, HIP_W * h);
                        let down = w.stance(j);
                        w.hip[j] += dq;
                        w.lift[j] += (lift[j].clamp(0.0, 1.0) - w.lift[j]) * h / LIFT_TAU;
                        if down {
                            // A planted foot stays put; the body moves over it.
                            moves.push(-LEG * w.hip[j].cos() * dq);
                        }
                    }
                    match moves.as_slice() {
                        [] => {
                            w.air += h;
                            w.x += w.v * h;
                        }
                        [d] => {
                            w.air = 0.0;
                            w.x += d;
                        }
                        [a, b] => {
                            w.air = 0.0;
                            w.x += (a + b) / 2.0;
                        }
                        _ => unreachable!(),
                    }
                }
                w.v = (w.x - x0) / DT;
                if w.air > AIR_MAX {
                    self.fail = Some(Fail::Fell);
                }
            }
            Rig::Tilt(_) => {}
            Rig::Corridor(c) => {
                c.v = c.speed(t0);
                c.x += c.v * DT;
            }
        }
        self.effort += effort;
        self.t += DT;
    }
}
