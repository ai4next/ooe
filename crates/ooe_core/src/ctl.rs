//! The controller: runs a PATCH program once per tick against a sensor frame.

use crate::lang::{Act, DT, Dst, Instr, Op, REGS, Sensor, Src};

pub type Senses = [f32; Sensor::COUNT];
pub type Outs = [f32; Act::COUNT];

/// Integrators saturate here so a stuck error cannot wind up forever.
pub const WINDUP: f32 = 20.0;

/// Memory of one stateful row.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Mem {
    y: f32,
    prev: f32,
    acc: f32,
    primed: bool,
}

#[derive(Clone, Debug)]
pub struct Controller {
    pub code: Vec<Instr>,
    pub regs: [f32; REGS as usize],
    pub outs: Outs,
    mem: Vec<Mem>,
}

fn finite(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(-1.0e6, 1.0e6)
    } else {
        0.0
    }
}

impl Controller {
    pub fn new(code: Vec<Instr>) -> Self {
        let n = code.len();
        Self {
            code,
            regs: [0.0; REGS as usize],
            outs: [0.0; Act::COUNT],
            mem: vec![Mem::default(); n],
        }
    }

    fn read(&self, s: Src, senses: &Senses) -> f32 {
        match s {
            Src::Reg(r) => self.regs[r as usize],
            Src::Sense(x) => senses[x as usize],
            Src::Out(a) => self.outs[a as usize],
            Src::Num(v) => v,
        }
    }

    /// One control tick. Registers and outputs keep their values between
    /// ticks, so a row may read what it (or a later row) wrote last tick.
    pub fn tick(&mut self, senses: &Senses) -> Outs {
        for i in 0..self.code.len() {
            let ins = self.code[i];
            let a = |k: usize| self.read(ins.args[k], senses);
            let v = match ins.op {
                Op::Mov => a(0),
                Op::Add => a(0) + a(1),
                Op::Sub => a(0) - a(1),
                Op::Mul => a(0) * a(1),
                Op::Div => {
                    let d = a(1);
                    if d.abs() < 1e-9 { 0.0 } else { a(0) / d }
                }
                Op::Min => a(0).min(a(1)),
                Op::Max => a(0).max(a(1)),
                Op::Abs => a(0).abs(),
                Op::Clamp => {
                    let l = a(1).abs();
                    a(0).clamp(-l, l)
                }
                Op::Sel => {
                    if a(0) > 0.0 {
                        a(1)
                    } else {
                        a(2)
                    }
                }
                Op::Sqrt => a(0).max(0.0).sqrt(),
                Op::Sin => a(0).sin(),
                Op::Cos => a(0).cos(),
                Op::Atan2 => a(0).atan2(a(1)),
                Op::Acos => a(0).clamp(-1.0, 1.0).acos(),
                Op::Lpf => {
                    let (x, k) = (a(0), a(1).clamp(0.0, 1.0));
                    let m = &mut self.mem[i];
                    m.y = if m.primed { m.y + k * (x - m.y) } else { x };
                    m.primed = true;
                    m.y
                }
                Op::Deriv => {
                    let x = a(0);
                    let m = &mut self.mem[i];
                    let d = if m.primed { (x - m.prev) / DT } else { 0.0 };
                    m.prev = x;
                    m.primed = true;
                    d
                }
                Op::Integ => {
                    let x = a(0);
                    let m = &mut self.mem[i];
                    m.acc = finite(m.acc + x * DT);
                    m.acc
                }
                Op::Pid => {
                    let (e, kp, ki, kd) = (a(0), a(1), a(2), a(3));
                    let m = &mut self.mem[i];
                    m.acc = (m.acc + e * DT).clamp(-WINDUP, WINDUP);
                    let d = if m.primed { (e - m.prev) / DT } else { 0.0 };
                    m.prev = e;
                    m.primed = true;
                    kp * e + ki * m.acc + kd * d
                }
            };
            let v = finite(v);
            match ins.d {
                Dst::Reg(r) => self.regs[r as usize] = v,
                Dst::Out(o) => self.outs[o as usize] = v,
            }
        }
        self.outs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::parse;

    fn senses(pairs: &[(Sensor, f32)]) -> Senses {
        let mut s = [0.0; Sensor::COUNT];
        for &(k, v) in pairs {
            s[k as usize] = v;
        }
        s
    }

    #[test]
    fn rows_run_in_order_and_remember() {
        let mut c = Controller::new(parse("ADD r0, r0, 1\nMUL motor, r0, pos").unwrap());
        let s = senses(&[(Sensor::Pos, 2.0)]);
        c.tick(&s);
        let o = c.tick(&s);
        assert_eq!(o[Act::Motor as usize], 4.0);
    }

    #[test]
    fn pid_sums_and_differentiates() {
        let mut c = Controller::new(parse("PID motor, pos, 0, 1, 1").unwrap());
        let o = c.tick(&senses(&[(Sensor::Pos, 1.0)]));
        assert!((o[0] - DT).abs() < 1e-6, "first tick: no derivative kick");
        let o = c.tick(&senses(&[(Sensor::Pos, 2.0)]));
        assert!((o[0] - (3.0 * DT + 1.0 / DT)).abs() < 1e-3);
    }

    #[test]
    fn filters_start_on_their_input() {
        let mut c = Controller::new(parse("LPF r0, pos, 0.5\nDERIV r1, pos").unwrap());
        c.tick(&senses(&[(Sensor::Pos, 4.0)]));
        assert_eq!(c.regs[0], 4.0);
        assert_eq!(c.regs[1], 0.0);
        c.tick(&senses(&[(Sensor::Pos, 0.0)]));
        assert_eq!(c.regs[0], 2.0);
        assert!((c.regs[1] + 4.0 / DT).abs() < 1e-3);
    }

    #[test]
    fn bad_math_stays_finite() {
        let mut c = Controller::new(parse("DIV r0, 1, 0\nSQRT r1, -4\nACOS r2, 3").unwrap());
        c.tick(&senses(&[]));
        assert_eq!(c.regs[..3], [0.0, 0.0, 0.0]);
    }
}
