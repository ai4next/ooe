//! What a level is: the robot, the test cases, what counts as success, the
//! mastery goals, and the teaching text around it.

use oo_base::Tr;

use crate::grade::Sample;
use crate::lang::{Act, Op, Sensor, Src};
use crate::sim::{Real, Rig};

pub use oo_base::{Islander, LevelKind as Kind};

/// Who a chapter's commission is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Client {
    One(Islander),
    /// Storm day: the whole island.
    Everyone,
}

impl Client {
    pub fn name(self) -> Tr {
        match self {
            Client::One(i) => match i {
                Islander::Zhou => Tr::new("Old Zhou", "老周"),
                Islander::Ke => Tr::new("Ake", "阿珂"),
                Islander::Man => Tr::new("Xiaoman", "小满"),
                Islander::Su => Tr::new("Granny Su", "苏婆"),
                Islander::De => Tr::new("Ade", "阿德"),
                Islander::Lina => Tr::new("Dr. Lina", "莉娜"),
                Islander::Kessler => Tr::new("Kessler", "Kessler"),
            },
            Client::Everyone => Tr::new("The whole island", "全岛"),
        }
    }

    pub fn role(self) -> Tr {
        match self {
            Client::One(i) => match i {
                Islander::Zhou => Tr::new("Harbour master", "港务长"),
                Islander::Ke => Tr::new("Schoolteacher", "小学老师"),
                Islander::Man => Tr::new("Postal courier", "邮差"),
                Islander::Su => Tr::new("Lighthouse keeper", "灯塔看守"),
                Islander::De => Tr::new("Fisherman", "渔民"),
                Islander::Lina => Tr::new("Clinic doctor", "诊所医生"),
                Islander::Kessler => Tr::new("Acquisitions, Helix", "Helix 收购负责人"),
            },
            Client::Everyone => Tr::new("Everyone, on storm day", "风暴日的所有人"),
        }
    }

    /// Signature colour (sRGB hex).
    pub fn color(self) -> u32 {
        match self {
            Client::One(i) => i.color(),
            Client::Everyone => 0xFFC857,
        }
    }
}

/// Short tag for a level kind.
pub fn kind_label(k: Kind) -> Tr {
    match k {
        Kind::Puzzle => Tr::new("PUZZLE", "解谜"),
        Kind::Fault => Tr::new("FAULT", "故障"),
        Kind::Commission => Tr::new("COMMISSION", "委托"),
    }
}

/// One stop on the island: a place, an idea, and the islander who needs it.
#[derive(Clone, Copy, Debug)]
pub struct Chapter {
    /// 1..=8.
    pub n: u8,
    pub place: Tr,
    /// The chapter's idea, in one line.
    pub theme: Tr,
    pub client: Client,
    /// What the client needs.
    pub need: Tr,
}

pub const CHAPTERS: [Chapter; 8] = [
    Chapter {
        n: 1,
        place: Tr::new("The Workshop", "工坊"),
        theme: Tr::new("Sensors in, motors out", "传感器进，电机出"),
        client: Client::One(Islander::Zhou),
        need: Tr::new(
            "The pier cart has to stop on its mark, not in the sea.",
            "栈桥上的货车要停准，不能掉进海里。",
        ),
    },
    Chapter {
        n: 2,
        place: Tr::new("The Lighthouse", "灯塔"),
        theme: Tr::new(
            "Gravity, inertia, and not overshooting",
            "重力、惯性与不冲过头",
        ),
        client: Client::One(Islander::Su),
        need: Tr::new(
            "The lighthouse lift never stops level with a floor.",
            "灯塔升降机老是停不准楼层。",
        ),
    },
    Chapter {
        n: 3,
        place: Tr::new("The School", "小学"),
        theme: Tr::new("Joints and space", "关节与空间"),
        client: Client::One(Islander::Ke),
        need: Tr::new(
            "An arm that can write on the blackboard.",
            "一只会在黑板上写字的手臂。",
        ),
    },
    Chapter {
        n: 4,
        place: Tr::new("The Post Office", "邮局"),
        theme: Tr::new("Two wheels and a map", "两个轮子，一张地图"),
        client: Client::One(Islander::Man),
        need: Tr::new(
            "A mail rover that finds its own way through the storm.",
            "一台风暴里会自己找路的送信车。",
        ),
    },
    Chapter {
        n: 5,
        place: Tr::new("The Clinic", "诊所"),
        theme: Tr::new(
            "Keeping unstable things up",
            "让不稳定的东西站住",
        ),
        client: Client::One(Islander::Lina),
        need: Tr::new(
            "A balancing cart that carries the medicine tray without spilling.",
            "一台端着药盘不洒的平衡车。",
        ),
    },
    Chapter {
        n: 6,
        place: Tr::new("The Reef", "礁石滩"),
        theme: Tr::new(
            "Walking is rhythmic falling",
            "走路是一种有节奏的摔倒",
        ),
        client: Client::One(Islander::De),
        need: Tr::new(
            "A walker that crosses the rocks at low tide to bring in the nets.",
            "一台退潮时走上礁石收网的步行机。",
        ),
    },
    Chapter {
        n: 7,
        place: Tr::new("The Fog Harbour", "雾港"),
        theme: Tr::new("Sensors lie", "传感器会撒谎"),
        client: Client::One(Islander::Kessler),
        need: Tr::new(
            "\"No GPS. Does your cart know where it is?\"",
            "“没有 GPS，你的车知道自己在哪吗？”",
        ),
    },
    Chapter {
        n: 8,
        place: Tr::new("Storm Day", "风暴日"),
        theme: Tr::new("Reality gives no retries", "现实不给重来"),
        client: Client::Everyone,
        need: Tr::new(
            "Every robot, working at once, in the worst weather in ten years.",
            "所有机器人同时上岗，在十年来最坏的天气里。",
        ),
    },
];

pub fn chapter(n: u8) -> &'static Chapter {
    &CHAPTERS[(n.clamp(1, 8) - 1) as usize]
}

/// What a run must achieve on every case.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pass {
    /// Bring the error within `tol` and keep it there for the last half second.
    Settle { tol: f32 },
    /// Keep the error's RMS (after the first second) under `rms`.
    Track { rms: f32 },
    /// Do not fall or leave the track before time runs out.
    Survive,
    /// Reach the end before time runs out.
    Finish,
    /// Do not fall or leave the track, and be docked when time runs out.
    Arrive,
}

/// Lower-is-better measurements of a run (worst case across test cases).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    /// Seconds to settle (or to finish).
    Time,
    /// How far past the target it went.
    Overshoot,
    /// RMS of the error after the first second.
    Error,
    /// Total actuator effort.
    Effort,
    /// Rows in the program.
    Size,
}

impl Metric {
    pub fn label(self) -> Tr {
        match self {
            Metric::Time => Tr::new("time", "用时"),
            Metric::Overshoot => Tr::new("overshoot", "超调"),
            Metric::Error => Tr::new("error", "误差"),
            Metric::Effort => Tr::new("effort", "能耗"),
            Metric::Size => Tr::new("rows", "行数"),
        }
    }

    /// Formats a value of this metric.
    pub fn show(self, v: f32) -> String {
        match self {
            Metric::Time => format!("{v:.2}s"),
            Metric::Size => format!("{}", v.round() as i32),
            Metric::Effort => format!("{v:.1}"),
            _ => format!("{v:.3}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Goal {
    pub metric: Metric,
    pub max: f32,
}

impl Goal {
    pub const fn new(metric: Metric, max: f32) -> Self {
        Self { metric, max }
    }
}

/// One test: a robot in a starting state, how real it is, and when it gets pushed.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    pub seed: u64,
    pub rig: Rig,
    pub real: Real,
    /// (time, velocity kick).
    pub pushes: Vec<(f32, f32)>,
    /// (time, new target): the target moves mid-run.
    pub moves: Vec<(f32, f32)>,
}

/// What the arrow keys do in the Showcase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Live {
    /// Kick the robot.
    Push,
    /// Move the cart's target.
    Target,
    /// Move the arm's target.
    Nudge,
}

/// The Predict step: commit to an answer before running anything.
#[derive(Clone, Copy, Debug)]
pub struct Predict {
    pub question: Tr,
    pub options: [Tr; 3],
    pub answer: usize,
    pub why: Tr,
}

/// The card earned on a solve: what the player just touched, honestly framed.
#[derive(Clone, Copy, Debug)]
pub struct Insight {
    pub title: Tr,
    pub line: Tr,
    /// How real robots do it.
    pub reality: Tr,
    /// A common misconception, corrected.
    pub myth: Tr,
    /// What the simulator simplifies away.
    pub simplified: Tr,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub id: &'static str,
    /// 1..=8.
    pub chapter: u8,
    /// 1..=6 within the chapter: 1–4 puzzles, 5 the fault, 6 the commission.
    pub slot: u8,
    pub title: Tr,
    /// What the lab wants, in the voice of KIT, the workshop assistant.
    /// Lines separated by `\n`.
    pub brief: Tr,
    pub ops: &'static [Op],
    pub sensors: &'static [Sensor],
    pub acts: &'static [Act],
    /// Seconds per case.
    pub duration: f32,
    pub pass: Pass,
    pub cases: fn() -> Vec<Case>,
    /// Must be met to count as solved.
    pub limit: Option<Goal>,
    /// Second and third star.
    pub goals: [Goal; 2],
    pub starter: &'static str,
    pub reference: &'static str,
    /// Signals the scope shows by default.
    pub scope: &'static [Src],
    /// The program writes `reward`; a Q-learner trains on it and drives.
    pub learn: bool,
    pub live: Option<Live>,
    pub predict: Predict,
    pub hints: [Tr; 3],
    pub insight: Insight,
    /// Fault levels: the evidence to point at before fixing.
    pub clue: Option<Clue>,
    /// Filled in by `content`: the previous level in the chapter, or the
    /// previous chapter's last puzzle.
    pub requires: Vec<&'static str>,
}

/// A fault's evidence: the moment things start to go wrong in the
/// starter's first failing case, which the player points at on the scope.
#[derive(Clone, Copy, Debug)]
pub struct Clue {
    /// The trace that shows it.
    pub signal: Src,
    /// The moment, found in the starter's samples (seconds).
    pub find: fn(&[Sample]) -> Option<f32>,
    /// How close a pick must be, seconds.
    pub within: f32,
    /// What the player is looking for, e.g. "the first time it reverses".
    pub what: Tr,
}

impl Task {
    pub fn kind(&self) -> Kind {
        Kind::of_slot(self.slot.saturating_sub(1))
    }

    /// `3-2`, as in the level table.
    pub fn code(&self) -> String {
        format!("{}-{}", self.chapter, self.slot)
    }

    pub fn chapter(&self) -> &'static Chapter {
        chapter(self.chapter)
    }
}
