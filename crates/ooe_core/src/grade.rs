//! Running a program against every case and turning it into stars.

use oo_base::Outcome;

use crate::ctl::{Controller, Outs, Senses};
use crate::lang::{self, Act, DT, Dst, Instr, Op, ParseError, REGS, Sensor, Src};
use crate::learn::{self, ACTIONS, QTable};
use crate::sim::{Fail, Pose, World};
use crate::task::{Case, Goal, Metric, Pass, Task};

/// Training always uses the same seed, so a reward always grades the same.
pub const TRAIN_SEED: u64 = 0x5eed;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    Parse(ParseError),
    /// A block this level does not offer.
    LockedOp(Op),
    LockedSensor(Sensor),
    LockedAct(Act),
}

pub fn build(task: &Task, src: &str) -> Result<Vec<Instr>, BuildError> {
    let code = lang::parse(src).map_err(BuildError::Parse)?;
    for ins in &code {
        if !task.ops.contains(&ins.op) {
            return Err(BuildError::LockedOp(ins.op));
        }
        if let Dst::Out(a) = ins.d
            && !task.acts.contains(&a)
        {
            return Err(BuildError::LockedAct(a));
        }
        for s in ins.srcs() {
            match *s {
                Src::Sense(x) if !task.sensors.contains(&x) => {
                    return Err(BuildError::LockedSensor(x));
                }
                Src::Out(a) if !task.acts.contains(&a) => return Err(BuildError::LockedAct(a)),
                _ => {}
            }
        }
    }
    Ok(code)
}

/// What drives the robot: the player's program, optionally with a trained
/// table taking over the motor.
#[derive(Clone, Debug)]
pub struct Brain {
    pub ctl: Controller,
    pub table: Option<QTable>,
}

impl Brain {
    pub fn new(code: &[Instr], table: Option<QTable>) -> Self {
        Self {
            ctl: Controller::new(code.to_vec()),
            table,
        }
    }

    pub fn tick(&mut self, s: &Senses) -> Outs {
        let mut o = self.ctl.tick(s);
        if let Some(t) = &self.table {
            o[Act::Motor as usize] = ACTIONS[t.best(learn::state(s))];
        }
        o
    }
}

/// One control tick, recorded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub t: f32,
    pub pose: Pose,
    pub senses: Senses,
    pub outs: Outs,
    pub regs: [f32; REGS as usize],
    pub err: f32,
    pub docked: bool,
}

impl Sample {
    pub fn read(&self, s: Src) -> f32 {
        match s {
            Src::Reg(r) => self.regs[r as usize],
            Src::Sense(x) => self.senses[x as usize],
            Src::Out(a) => self.outs[a as usize],
            Src::Num(v) => v,
        }
    }
}

/// Why a case did not pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Why {
    Failed(Fail),
    /// Never came to rest on the target.
    Unsettled,
    /// Time ran out before the finish.
    Late,
    /// Error RMS above the level's bound.
    Drift {
        rms: f32,
        max: f32,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Measure {
    pub time: f32,
    pub overshoot: f32,
    pub error: f32,
    pub effort: f32,
    pub size: f32,
}

impl Measure {
    pub fn get(&self, m: Metric) -> f32 {
        match m {
            Metric::Time => self.time,
            Metric::Overshoot => self.overshoot,
            Metric::Error => self.error,
            Metric::Effort => self.effort,
            Metric::Size => self.size,
        }
    }

    pub fn meets(&self, g: Goal) -> bool {
        self.get(g.metric) <= g.max + 1e-6
    }

    fn worst(&mut self, o: &Measure) {
        self.time = self.time.max(o.time);
        self.overshoot = self.overshoot.max(o.overshoot);
        self.error = self.error.max(o.error);
        self.effort = self.effort.max(o.effort);
    }
}

#[derive(Clone, Debug)]
pub struct CaseRun {
    pub case: Case,
    pub samples: Vec<Sample>,
    pub why: Option<Why>,
    pub measure: Measure,
}

impl CaseRun {
    pub fn ok(&self) -> bool {
        self.why.is_none()
    }

    /// When the run ended early, and how.
    pub fn end(&self) -> f32 {
        self.samples.last().map_or(0.0, |s| s.t)
    }
}

fn record(w: &World, s: Senses, outs: Outs, brain: &Brain) -> Sample {
    Sample {
        t: w.t,
        pose: w.pose(),
        senses: s,
        outs,
        regs: brain.ctl.regs,
        err: w.error(),
        docked: w.docked(),
    }
}

/// The last stretch a Settle run must hold steady for.
pub const HOLD: f32 = 0.5;

pub fn run_case(task: &Task, brain: &mut Brain, case: Case) -> CaseRun {
    let mut w = World::new(case.rig.clone(), case.real, case.pushes.clone(), case.seed);
    w.moves = case.moves.clone();
    let ticks = (task.duration / DT).round() as usize;
    let mut samples = Vec::with_capacity(ticks + 1);
    let s0 = w.senses();
    samples.push(record(&w, s0, [0.0; Act::COUNT], brain));
    let mut senses = s0;
    let mut finished = None;
    for _ in 0..ticks {
        let outs = brain.tick(&senses);
        w.step(outs);
        senses = w.senses();
        samples.push(record(&w, senses, outs, brain));
        if w.fail.is_some() {
            break;
        }
        if task.pass == Pass::Finish && w.docked() {
            finished = Some(w.t);
            break;
        }
    }
    let e0 = samples[0].err;
    let mut m = Measure {
        effort: w.effort,
        ..Default::default()
    };
    if e0.abs() > 1e-6 {
        m.overshoot = samples
            .iter()
            .map(|s| -s.err * e0.signum())
            .fold(0.0, f32::max);
    }
    let late: Vec<f32> = samples
        .iter()
        .filter(|s| s.t > 1.0)
        .map(|s| s.err)
        .collect();
    if !late.is_empty() {
        m.error = (late.iter().map(|e| e * e).sum::<f32>() / late.len() as f32).sqrt();
    }
    let mut why = w.fail.map(Why::Failed);
    match task.pass {
        Pass::Settle { tol } => {
            let last_out = samples.iter().rposition(|s| s.err.abs() > tol);
            m.time = last_out.map_or(0.0, |i| samples.get(i + 1).map_or(w.t, |s| s.t));
            if why.is_none() && m.time > task.duration - HOLD {
                why = Some(Why::Unsettled);
            }
        }
        Pass::Track { rms } => {
            if why.is_none() && m.error > rms {
                why = Some(Why::Drift {
                    rms: m.error,
                    max: rms,
                });
            }
        }
        Pass::Finish => {
            m.time = finished.unwrap_or(w.t);
            if why.is_none() && finished.is_none() {
                why = Some(Why::Late);
            }
        }
        Pass::Survive => m.time = w.t,
        Pass::Arrive => {
            m.time = w.t;
            if why.is_none() && !w.docked() {
                why = Some(Why::Unsettled);
            }
        }
    }
    CaseRun {
        case,
        samples,
        why,
        measure: m,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    Pass,
    /// The first failing case.
    Miss {
        case: usize,
        why: Why,
    },
    /// Works, but misses the level's hard limit.
    OverLimit {
        goal: Goal,
        got: f32,
    },
}

#[derive(Clone, Debug)]
pub struct Report {
    pub code: Vec<Instr>,
    pub runs: Vec<CaseRun>,
    pub verdict: Verdict,
    pub measure: Measure,
    pub goals_met: [bool; 2],
    /// Learning levels: the trained table and its learning curve.
    pub table: Option<QTable>,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.verdict == Verdict::Pass
    }

    pub fn stars(&self) -> u8 {
        if !self.passed() {
            return 0;
        }
        1 + self.goals_met.iter().filter(|m| **m).count() as u8
    }

    pub fn outcome(&self, task: &Task) -> Option<Outcome> {
        self.passed().then(|| Outcome {
            stars: self.stars(),
            metric: self.measure.get(task.goals[0].metric) as f64,
            alt: self.measure.get(task.goals[1].metric) as f64,
        })
    }
}

/// The brain a level runs: learning levels train first.
pub fn brain(task: &Task, code: &[Instr]) -> Brain {
    let table = task
        .learn
        .then(|| (task.cases)().into_iter().next())
        .flatten()
        .map(|c| learn::train(code, &c.rig, task.duration, TRAIN_SEED));
    Brain::new(code, table)
}

pub fn grade(task: &Task, src: &str) -> Result<Report, BuildError> {
    let code = build(task, src)?;
    Ok(grade_code(task, code))
}

pub fn grade_code(task: &Task, code: Vec<Instr>) -> Report {
    let proto = brain(task, &code);
    let runs: Vec<CaseRun> = (task.cases)()
        .into_iter()
        .map(|c| run_case(task, &mut proto.clone(), c))
        .collect();
    let mut measure = Measure {
        size: code.len() as f32,
        ..Default::default()
    };
    for r in &runs {
        measure.worst(&r.measure);
    }
    let mut verdict = runs
        .iter()
        .enumerate()
        .find_map(|(i, r)| r.why.map(|why| Verdict::Miss { case: i, why }))
        .unwrap_or(Verdict::Pass);
    if verdict == Verdict::Pass
        && let Some(goal) = task.limit
        && !measure.meets(goal)
    {
        verdict = Verdict::OverLimit {
            goal,
            got: measure.get(goal.metric),
        };
    }
    let goals_met = task.goals.map(|g| measure.meets(g));
    Report {
        code,
        runs,
        verdict,
        measure,
        goals_met,
        table: proto.table,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::task;

    #[test]
    fn locked_blocks_are_rejected() {
        let t = task("eyes-open").unwrap();
        assert_eq!(
            grade(t, "PID motor, pos, 1, 0, 0").unwrap_err(),
            BuildError::LockedOp(Op::Pid)
        );
        assert_eq!(
            grade(t, "MOV motor, gyro").unwrap_err(),
            BuildError::LockedSensor(Sensor::Gyro)
        );
        assert_eq!(
            grade(t, "MOV left, 1").unwrap_err(),
            BuildError::LockedAct(Act::Left)
        );
        assert!(matches!(grade(t, "MOV pos, 1"), Err(BuildError::Parse(_))));
    }

    #[test]
    fn grading_is_deterministic() {
        let t = task("it-worked-in-sim").unwrap();
        let a = grade(t, t.reference).unwrap();
        let b = grade(t, t.reference).unwrap();
        assert_eq!(a.measure, b.measure);
        assert_eq!(a.runs[2].samples, b.runs[2].samples);
    }

    #[test]
    fn crashes_end_the_run_and_say_why() {
        let t = task("the-wall").unwrap();
        let rep = grade(t, t.starter).unwrap();
        assert_eq!(
            rep.verdict,
            Verdict::Miss {
                case: 0,
                why: Why::Failed(Fail::Crash)
            }
        );
        assert!(rep.runs[0].end() < t.duration);
    }

    #[test]
    fn hard_limits_block_the_pass() {
        let t = task("brake").unwrap();
        let rep = grade(
            t,
            "SUB r0, target, pos\nMUL r0, r0, 30\nMUL r1, vel, -10\nADD motor, r0, r1",
        )
        .unwrap();
        assert!(matches!(rep.verdict, Verdict::OverLimit { .. }));
        assert_eq!(rep.stars(), 0);
    }

    #[test]
    fn rewards_decide_what_is_learned() {
        let t = task("self-park").unwrap();
        let shaped = grade(t, t.reference).unwrap();
        assert!(shaped.passed());
        let table = shaped.table.as_ref().unwrap();
        assert!(
            table.recent_success(100) > 0.5,
            "{}",
            table.recent_success(100)
        );
        let sparse = grade(t, t.starter).unwrap();
        assert!(!sparse.passed());
    }

    #[test]
    fn samples_cover_the_run() {
        let t = task("eyes-open").unwrap();
        let rep = grade(t, t.reference).unwrap();
        let r = &rep.runs[0];
        assert_eq!(r.samples.len(), (t.duration / DT).round() as usize + 1);
        assert_eq!(r.samples[0].t, 0.0);
        assert!(r.samples.last().unwrap().docked);
    }
}
