//! The lab's levels. Every level ships a reference that the tests hold to
//! three stars, and a starter that must not pass for free.

use std::sync::LazyLock;

use oo_base::Tr;

use crate::lang::{Act, Op, Sensor, Src};
use crate::sim::*;
use crate::task::*;

mod ch1;
mod ch2;
mod ch3;
mod ch4;
mod ch5;
mod ch6;
mod ch7;
mod ch8;

/// v1 level ids and the level each became (save version 1).
pub const RENAMED: &[(&str, &str)] = &[
    ("wake-up", "eyes-open"),
    ("mind-the-wall", "the-wall"),
    ("soft-landing", "brake"),
    ("lift-off", "sag"),
    ("line-follower", "follow-line"),
    ("balance", "four-gains"),
    ("first-steps", "left-right"),
    ("steady-gaze", "two-sensors"),
    ("where-am-i", "how-far"),
    ("learn-to-dock", "self-park"),
    ("sim-to-real", "it-worked-in-sim"),
];

static TASKS: LazyLock<Vec<Task>> = LazyLock::new(|| {
    let mut v = ch1::tasks();
    for more in [
        ch2::tasks,
        ch3::tasks,
        ch4::tasks,
        ch5::tasks,
        ch6::tasks,
        ch7::tasks,
        ch8::tasks,
    ] {
        v.extend(more());
    }
    v.sort_by_key(|t| (t.chapter, t.slot));
    // Each level opens after the one before it in its chapter; a chapter
    // opens once the previous chapter's puzzles are done.
    for i in 1..v.len() {
        let (prev, t) = (&v[i - 1], &v[i]);
        let req = if prev.chapter == t.chapter {
            Some(prev.id)
        } else {
            v[..i]
                .iter()
                .rev()
                .find(|p| p.kind() == Kind::Puzzle)
                .map(|p| p.id)
        };
        v[i].requires = req.into_iter().collect();
    }
    v
});

pub fn tasks() -> &'static [Task] {
    &TASKS
}

pub fn task(id: &str) -> Option<&'static Task> {
    tasks().iter().find(|t| t.id == id)
}

pub fn in_chapter(n: u8) -> impl Iterator<Item = &'static Task> {
    tasks().iter().filter(move |t| t.chapter == n)
}

pub(crate) fn case(seed: u64, rig: Rig) -> Case {
    Case {
        seed,
        rig,
        real: Real::default(),
        pushes: Vec::new(),
        moves: Vec::new(),
    }
}

/// A wheeled cart with a speed servo.
pub(crate) fn servo_cart(x: f32, target: f32) -> Cart {
    Cart {
        x,
        v: 0.0,
        drive: Drive::Speed {
            tau: 0.15,
            vmax: 1.5,
        },
        target,
        gravity: false,
        wall: None,
        rail: 4.0,
        tol: 0.05,
    }
}

/// A sled pushed by a raw force: inertia, so it can overshoot.
pub(crate) fn force_cart(x: f32, target: f32, mass: f32) -> Cart {
    Cart {
        drive: Drive::Force {
            mass,
            friction: 0.5,
            fmax: 30.0,
        },
        ..servo_cart(x, target)
    }
}

pub(crate) fn pole(th: f32, x: f32) -> Pole {
    Pole {
        x,
        v: 0.0,
        th,
        w: 0.0,
        pole_mass: 0.1,
        cart_mass: 1.0,
        target: 0.0,
        rail: POLE_RAIL,
        fall: POLE_FALL,
    }
}

pub(crate) const MATH: &[Op] = &[
    Op::Mov,
    Op::Add,
    Op::Sub,
    Op::Mul,
    Op::Div,
    Op::Min,
    Op::Max,
    Op::Abs,
    Op::Clamp,
    Op::Sel,
];

pub(crate) const CONTROL: &[Op] = &[
    Op::Mov,
    Op::Add,
    Op::Sub,
    Op::Mul,
    Op::Div,
    Op::Min,
    Op::Max,
    Op::Abs,
    Op::Clamp,
    Op::Sel,
    Op::Lpf,
    Op::Deriv,
    Op::Integ,
    Op::Pid,
];

pub(crate) const ALL_OPS: &[Op] = Op::ALL;

const EMPTY: Tr = Tr::new("", "");

/// A level skeleton; content files fill in the rest with struct update.
pub(crate) fn stub(id: &'static str, chapter: u8, slot: u8) -> Task {
    Task {
        id,
        chapter,
        slot,
        title: EMPTY,
        brief: EMPTY,
        ops: MATH,
        sensors: &[Sensor::Pos, Sensor::Target],
        acts: &[Act::Motor],
        duration: 6.0,
        pass: Pass::Survive,
        cases: Vec::new,
        limit: None,
        goals: [Goal::new(Metric::Time, 0.0), Goal::new(Metric::Size, 0.0)],
        starter: "",
        reference: "",
        scope: &[],
        learn: false,
        live: None,
        predict: Predict {
            question: EMPTY,
            options: [EMPTY; 3],
            answer: 0,
            why: EMPTY,
        },
        hints: [EMPTY; 3],
        insight: Insight {
            title: EMPTY,
            line: EMPTY,
            reality: EMPTY,
            myth: EMPTY,
            simplified: EMPTY,
        },
        clue: None,
        requires: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::grade::{Verdict, build, grade};

    #[test]
    fn ids_are_unique_and_requirements_exist() {
        let mut seen = HashSet::new();
        for t in tasks() {
            assert!(seen.insert(t.id), "duplicate {}", t.id);
            for r in &t.requires {
                assert!(seen.contains(r), "{} requires later/unknown {r}", t.id);
            }
        }
        assert_eq!(tasks().len(), 17);
        for t in tasks() {
            assert!((1..=8).contains(&t.chapter) && (1..=6).contains(&t.slot), "{}", t.id);
        }
        let mut slots = HashSet::new();
        assert!(tasks().iter().all(|t| slots.insert((t.chapter, t.slot))));
        for (_, new) in RENAMED {
            assert!(task(new).is_some(), "{new}");
        }
    }

    #[test]
    fn every_reference_earns_three_stars() {
        for t in tasks() {
            let rep = grade(t, t.reference).unwrap_or_else(|e| panic!("{}: {e:?}", t.id));
            assert_eq!(rep.verdict, Verdict::Pass, "{}", t.id);
            assert_eq!(
                rep.stars(),
                3,
                "{}: {:?} vs {:?}",
                t.id,
                rep.measure,
                t.goals
            );
        }
    }

    #[test]
    fn no_starter_passes() {
        for t in tasks() {
            let rep = grade(t, t.starter).unwrap_or_else(|e| panic!("{}: {e:?}", t.id));
            assert!(!rep.passed(), "{} starter passes", t.id);
        }
    }

    #[test]
    fn text_is_bilingual_and_predictions_valid() {
        for t in tasks() {
            let mut all = vec![t.title, t.brief, t.predict.question, t.predict.why];
            all.extend(t.predict.options);
            all.extend(t.hints);
            let i = t.insight;
            all.extend([i.title, i.line, i.reality, i.myth, i.simplified]);
            for tr in all {
                assert!(tr.is_complete(), "{}: {tr:?}", t.id);
            }
            assert!(t.predict.answer < 3, "{}", t.id);
        }
    }

    #[test]
    fn sources_are_bilingual_and_parse() {
        for t in tasks() {
            for src in [t.starter, t.reference] {
                build(t, src).unwrap_or_else(|e| panic!("{}: {e:?}", t.id));
                for line in src.lines().filter(|l| l.contains(';')) {
                    assert!(line.contains(" | "), "{}: {line}", t.id);
                }
            }
            assert!(!t.scope.is_empty(), "{}", t.id);
            assert!(
                t.limit
                    .is_none_or(|g| g.metric != t.goals[0].metric || g.max >= t.goals[0].max)
            );
        }
    }

    #[test]
    fn faults_have_findable_clues() {
        for t in tasks() {
            assert_eq!(t.clue.is_some(), t.kind() == Kind::Fault, "{}", t.id);
            if let Some(c) = t.clue {
                assert!(c.what.is_complete(), "{}", t.id);
                let rep = grade(t, t.starter).unwrap();
                let run = rep.runs.iter().find(|r| !r.ok()).unwrap();
                let at = (c.find)(&run.samples);
                assert!(at.is_some_and(|a| a > 0.0 && a <= run.end()), "{}: {at:?}", t.id);
            }
        }
    }

    #[test]
    fn cases_vary() {
        for t in tasks() {
            let cs = (t.cases)();
            assert!(cs.len() >= 3, "{}", t.id);
            assert!(cs.windows(2).all(|w| w[0] != w[1]), "{}", t.id);
        }
    }
}
