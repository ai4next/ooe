//! PATCH, the controller language. A program is a list of rows that run top
//! to bottom once per control tick (50 Hz):
//!
//! ```text
//! SUB r0, target, pos      ; error
//! PID motor, r0, 2, 0, 0.8
//! ```
//!
//! Each row writes one destination (a register `r0..r7` or an actuator) from
//! sources (registers, sensors, actuators, numbers). Filters and PID keep
//! their own memory between ticks, one slot per row.

use std::fmt;

pub const REGS: u8 = 8;
/// Control period, seconds.
pub const DT: f32 = 0.02;

macro_rules! named {
    ($(#[$m:meta])* $ty:ident { $($v:ident = $s:literal),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $ty { $($v),* }
        impl $ty {
            pub const ALL: &'static [$ty] = &[$($ty::$v),*];
            pub const COUNT: usize = $ty::ALL.len();
            pub fn name(self) -> &'static str {
                match self { $($ty::$v => $s),* }
            }
            pub fn parse(s: &str) -> Option<Self> {
                $ty::ALL.iter().copied().find(|x| x.name() == s)
            }
        }
    };
}

named!(
    /// What the robot can read.
    Sensor {
        T = "t",
        Dt = "dt",
        Pos = "pos",
        Vel = "vel",
        Target = "target",
        Dist = "dist",
        Angle = "angle",
        Gyro = "gyro",
        Accel = "accel",
        Offset = "offset",
        Heading = "heading",
        Q1 = "q1",
        Q2 = "q2",
        Tx = "tx",
        Ty = "ty",
        Odom = "odom",
        Beacon = "beacon",
        Fix = "fix",
        Dock = "dock",
        Fell = "fell",
    }
);

named!(
    /// What the robot can drive (or report).
    Act {
        Motor = "motor",
        Left = "left",
        Right = "right",
        Shoulder = "shoulder",
        Elbow = "elbow",
        Est = "est",
        HipL = "hip_l",
        HipR = "hip_r",
        LiftL = "lift_l",
        LiftR = "lift_r",
        Reward = "reward",
    }
);

named!(
    Op {
        Mov = "MOV",
        Add = "ADD",
        Sub = "SUB",
        Mul = "MUL",
        Div = "DIV",
        Min = "MIN",
        Max = "MAX",
        Abs = "ABS",
        Clamp = "CLAMP",
        Sel = "SEL",
        Sqrt = "SQRT",
        Sin = "SIN",
        Cos = "COS",
        Atan2 = "ATAN2",
        Acos = "ACOS",
        Lpf = "LPF",
        Deriv = "DERIV",
        Integ = "INTEG",
        Pid = "PID",
    }
);

impl Op {
    /// Number of sources after the destination.
    pub fn arity(self) -> usize {
        match self {
            Op::Mov | Op::Abs | Op::Sqrt | Op::Sin | Op::Cos | Op::Acos => 1,
            Op::Deriv | Op::Integ => 1,
            Op::Sel => 3,
            Op::Pid => 4,
            _ => 2,
        }
    }

    /// Keeps memory between ticks.
    pub fn is_stateful(self) -> bool {
        matches!(self, Op::Lpf | Op::Deriv | Op::Integ | Op::Pid)
    }

    /// The row a fresh block of this kind starts as.
    pub fn template(self) -> Instr {
        let args = match self {
            Op::Pid => [Src::Num(0.0), Src::Num(1.0), Src::Num(0.0), Src::Num(0.0)],
            Op::Lpf => [Src::Num(0.0), Src::Num(0.2), Src::Num(0.0), Src::Num(0.0)],
            _ => [Src::Num(0.0); 4],
        };
        Instr {
            op: self,
            d: Dst::Reg(0),
            args,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Src {
    Reg(u8),
    Sense(Sensor),
    Out(Act),
    Num(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dst {
    Reg(u8),
    Out(Act),
}

impl From<Dst> for Src {
    fn from(d: Dst) -> Self {
        match d {
            Dst::Reg(r) => Src::Reg(r),
            Dst::Out(a) => Src::Out(a),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instr {
    pub op: Op,
    pub d: Dst,
    /// Only the first `op.arity()` are used; the rest stay `Num(0)`.
    pub args: [Src; 4],
}

impl Instr {
    pub fn srcs(&self) -> &[Src] {
        &self.args[..self.op.arity()]
    }
}

/// Numbers print short: `2`, `0.5`, `-1.25`.
pub fn num(v: f32) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    if r == 0.0 { "0".into() } else { format!("{r}") }
}

impl fmt::Display for Src {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Src::Reg(r) => write!(f, "r{r}"),
            Src::Sense(s) => f.write_str(s.name()),
            Src::Out(a) => f.write_str(a.name()),
            Src::Num(v) => f.write_str(&num(*v)),
        }
    }
}

impl fmt::Display for Dst {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Src::from(*self).fmt(f)
    }
}

impl fmt::Display for Instr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.op.name(), self.d)?;
        for s in self.srcs() {
            write!(f, ", {s}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseErrorKind {
    UnknownOp(String),
    BadSource(String),
    BadDest(String),
    Arity { want: usize, got: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// 0-based source line.
    pub line: usize,
    pub kind: ParseErrorKind,
}

fn reg(s: &str) -> Option<u8> {
    s.strip_prefix('r')?.parse().ok().filter(|r| *r < REGS)
}

pub fn parse_src(s: &str) -> Option<Src> {
    if let Some(r) = reg(s) {
        return Some(Src::Reg(r));
    }
    if let Some(x) = Sensor::parse(s) {
        return Some(Src::Sense(x));
    }
    if let Some(a) = Act::parse(s) {
        return Some(Src::Out(a));
    }
    s.parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .map(Src::Num)
}

pub fn parse_dst(s: &str) -> Option<Dst> {
    match parse_src(s)? {
        Src::Reg(r) => Some(Dst::Reg(r)),
        Src::Out(a) => Some(Dst::Out(a)),
        _ => None,
    }
}

/// Parses one row (comments already stripped). Blank rows are `None`.
pub fn parse_line(text: &str) -> Result<Option<Instr>, ParseErrorKind> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let (op, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let op = Op::parse(&op.to_ascii_uppercase())
        .ok_or_else(|| ParseErrorKind::UnknownOp(op.to_string()))?;
    let parts: Vec<String> = if rest.trim().is_empty() {
        Vec::new()
    } else {
        rest.split(',')
            .map(|a| a.trim().to_ascii_lowercase())
            .collect()
    };
    let want = op.arity() + 1;
    if parts.len() != want {
        return Err(ParseErrorKind::Arity {
            want,
            got: parts.len(),
        });
    }
    let d = parse_dst(&parts[0]).ok_or_else(|| ParseErrorKind::BadDest(parts[0].clone()))?;
    let mut args = [Src::Num(0.0); 4];
    for (i, p) in parts[1..].iter().enumerate() {
        args[i] = parse_src(p).ok_or_else(|| ParseErrorKind::BadSource(p.clone()))?;
    }
    Ok(Some(Instr { op, d, args }))
}

/// Strips a `;` or `#` comment, returning the body and the note.
pub fn split_comment(raw: &str) -> (&str, Option<&str>) {
    match raw.find([';', '#']) {
        Some(i) => (&raw[..i], Some(raw[i + 1..].trim())),
        None => (raw, None),
    }
}

pub fn parse(src: &str) -> Result<Vec<Instr>, ParseError> {
    let mut out = Vec::new();
    for (line, raw) in src.lines().enumerate() {
        let (body, _) = split_comment(raw);
        if let Some(i) = parse_line(body).map_err(|kind| ParseError { line, kind })? {
            out.push(i);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_round_trip() {
        let src = "SUB r0, target, pos\nPID motor, r0, 2, 0, 0.8\nSEL est, fix, beacon, -1.5";
        let code = parse(src).unwrap();
        assert_eq!(code.len(), 3);
        let back: Vec<String> = code.iter().map(|i| i.to_string()).collect();
        assert_eq!(back.join("\n"), src);
    }

    #[test]
    fn errors_name_the_line() {
        let e = parse("MOV motor, 1\nFLY r0, 1").unwrap_err();
        assert_eq!(e.line, 1);
        assert!(matches!(e.kind, ParseErrorKind::UnknownOp(_)));
        assert!(matches!(
            parse_line("ADD r0, 1"),
            Err(ParseErrorKind::Arity { want: 3, got: 2 })
        ));
        assert!(matches!(
            parse_line("MOV pos, 1"),
            Err(ParseErrorKind::BadDest(_))
        ));
        assert!(matches!(
            parse_line("MOV r0, r9"),
            Err(ParseErrorKind::BadSource(_))
        ));
        assert_eq!(parse("  ; only a note\n\n").unwrap(), Vec::new());
    }

    #[test]
    fn numbers_print_short() {
        assert_eq!(num(2.0), "2");
        assert_eq!(num(-0.25), "-0.25");
        assert_eq!(num(0.1 + 0.2), "0.3");
        assert_eq!(num(-0.0), "0");
    }
}
