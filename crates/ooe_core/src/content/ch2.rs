//! Chapter 2 · The Lighthouse: gravity, inertia, and not overshooting.

use super::*;

pub(super) fn tasks() -> Vec<Task> {
    vec![brake(), sag()]
}

fn sled(seed: u64, target: f32, mass: f32) -> Case {
    let mut c = force_cart(0.0, target, mass);
    c.tol = 0.02;
    case(seed, Rig::Cart(c))
}

fn brake() -> Task {
    Task {
        ops: CONTROL,
        sensors: &[Sensor::Pos, Sensor::Vel, Sensor::Target],
        duration: 8.0,
        pass: Pass::Settle { tol: 0.02 },
        cases: || vec![sled(51, 2.5, 2.0), sled(52, -1.5, 2.0), sled(53, 3.0, 3.0)],
        limit: Some(Goal::new(Metric::Overshoot, 0.15)),
        goals: [
            Goal::new(Metric::Time, 3.0),
            Goal::new(Metric::Overshoot, 0.02),
        ],
        starter: "\
; spring towards the target | 像弹簧一样拉向目标
SUB r0, target, pos
MUL motor, r0, 20
",
        reference: "\
SUB r0, target, pos
; the spring, plus a damper on speed | 弹簧，再加一个针对速度的阻尼
MUL r0, r0, 30
MUL r1, vel, -18
ADD motor, r0, r1
",
        scope: &[
            Src::Sense(Sensor::Pos),
            Src::Sense(Sensor::Vel),
            Src::Out(Act::Motor),
        ],
        live: Some(Live::Target),
        title: Tr::new("Soft Landing", "软着陆"),
        brief: Tr::new(
            "SLED glides on ice: a force changes its speed, and it keeps sliding when the force stops.\nPark it on the mark. Overshooting by more than 15 cm fails the run.\nYou can read `vel` as well as `pos`.",
            "SLED 在冰面上滑行：推力改变的是速度，推力停了它还会继续滑。\n把它停在标记上。冲过头超过 15 厘米就算失败。\n除了 `pos`，你还能读 `vel`。",
        ),
        predict: Predict {
            question: Tr::new(
                "A spring pulls the sled towards the mark with nothing else. What does it do?",
                "只用一根弹簧把雪橇拉向标记，别的什么都不加。它会怎样？",
            ),
            options: [
                Tr::new("Slides in and stops.", "滑过去停下。"),
                Tr::new(
                    "Swings past the mark, back, and past again.",
                    "冲过标记、回来、又冲过去。",
                ),
                Tr::new("Stops before reaching it.", "没到就停了。"),
            ],
            answer: 1,
            why: Tr::new(
                "By the time it reaches the mark the spring has given it speed, and nothing takes that speed away. Ice has almost no friction to help.",
                "到达标记时弹簧已经给了它速度，却没有任何东西消耗这个速度。冰面几乎没有摩擦来帮忙。",
            ),
        },
        hints: [
            Tr::new(
                "The scope shows the sled oscillating. You need a force that opposes speed, like a shock absorber.",
                "示波器显示雪橇在来回振荡。你需要一个与速度相反的力，就像减震器。",
            ),
            Tr::new(
                "Add a term that pushes against `vel`. Tune it until the sled glides in without crossing the mark.",
                "加一项与 `vel` 相反的推力。调到雪橇能滑进去又不越过标记为止。",
            ),
            Tr::new(
                "SUB r0, target, pos / MUL r0, r0, 30 / MUL r1, vel, -18 / ADD motor, r0, r1",
                "SUB r0, target, pos / MUL r0, r0, 30 / MUL r1, vel, -18 / ADD motor, r0, r1",
            ),
        ],
        insight: Insight {
            title: Tr::new("Damping", "阻尼"),
            line: Tr::new(
                "Things with inertia need two forces: one that pulls towards the goal, one that resists speed.",
                "有惯性的东西需要两种力：一种拉向目标，一种抵抗速度。",
            ),
            reality: Tr::new(
                "Car suspensions, door closers and hard-disk heads are all tuned for just enough damping.",
                "汽车悬挂、闭门器、硬盘磁头都被调成恰到好处的阻尼。",
            ),
            myth: Tr::new(
                "Overshoot is not caused by going too fast, but by braking too late.",
                "超调不是因为太快，而是因为刹车太晚。",
            ),
            simplified: Tr::new(
                "Here `vel` is measured perfectly; real speed is usually estimated and noisy.",
                "这里 `vel` 是精确测量的；真实的速度通常是估算出来的，还带噪声。",
            ),
        },
        ..stub("brake", 2, 2)
    }
}

fn lift(seed: u64, target: f32, mass: f32) -> Case {
    let mut c = force_cart(0.0, target, 1.0);
    c.gravity = true;
    c.rail = 3.5;
    c.tol = 0.02;
    Case {
        real: Real {
            mass,
            ..Real::default()
        },
        ..case(seed, Rig::Cart(c))
    }
}

fn sag() -> Task {
    Task {
        ops: CONTROL,
        sensors: &[Sensor::Pos, Sensor::Vel, Sensor::Target],
        duration: 8.0,
        pass: Pass::Settle { tol: 0.03 },
        cases: || vec![lift(41, 1.0, 1.0), lift(42, 1.6, 0.6), lift(43, 1.2, 1.6)],
        goals: [
            Goal::new(Metric::Time, 4.0),
            Goal::new(Metric::Overshoot, 0.1),
        ],
        starter: "\
; push harder the further below the target | 离目标越远推力越大
SUB r0, target, pos
MUL r1, vel, -6
MUL r0, r0, 30
ADD motor, r0, r1
",
        reference: "\
; P lifts, I learns the weight, D keeps it from bouncing | P 负责抬升，I 学会重量，D 防止弹跳
SUB r0, target, pos
PID motor, r0, 40, 20, 10
",
        scope: &[
            Src::Sense(Sensor::Pos),
            Src::Sense(Sensor::Target),
            Src::Out(Act::Motor),
        ],
        live: Some(Live::Target),
        title: Tr::new("Lift Off", "升空"),
        brief: Tr::new(
            "HOIST is a lift platform driven by a raw force, and gravity pulls it down all the time.\nThis spring-and-damper program gets close, then sags below the target. Heavier loads sag more.\nMake it hold the exact height, whatever it carries.",
            "HOIST 是一个由原始推力驱动的升降台，重力一直在把它往下拉。\n这段弹簧加阻尼的程序能接近目标，却总是停在目标下方。负载越重，下沉越多。\n让它无论载多重都稳稳停在目标高度。",
        ),
        predict: Predict {
            question: Tr::new(
                "A pure spring controller holds a heavy load. Where does it settle?",
                "一个纯弹簧控制器托着重物。它最终停在哪？",
            ),
            options: [
                Tr::new("Exactly at the target.", "正好在目标处。"),
                Tr::new("Above the target.", "目标上方。"),
                Tr::new(
                    "Below the target, by an amount that grows with the weight.",
                    "目标下方，越重差得越多。",
                ),
            ],
            answer: 2,
            why: Tr::new(
                "A spring only pushes when stretched. To hold the weight up it must stay stretched, so there is always some error left: steady-state error.",
                "弹簧只有被拉伸时才出力。要托住重量就必须一直拉伸着，所以总会剩下一点误差：稳态误差。",
            ),
        },
        hints: [
            Tr::new(
                "Watch the error on the scope: it settles, but not at zero. Something has to supply the force for the weight.",
                "看示波器上的误差：它稳住了，但不是零。需要有东西专门提供托住重量的力。",
            ),
            Tr::new(
                "An integral adds up the error over time, so it keeps growing until the error is gone. `PID` has one built in.",
                "积分把误差随时间累加，误差不消失它就一直增长。`PID` 自带积分。",
            ),
            Tr::new(
                "SUB r0, target, pos / PID motor, r0, 40, 20, 10 — P lifts, I learns the weight, D damps.",
                "SUB r0, target, pos / PID motor, r0, 40, 20, 10 —— P 抬升，I 学会重量，D 阻尼。",
            ),
        ],
        insight: Insight {
            title: Tr::new("PID", "PID 控制"),
            line: Tr::new(
                "P reacts to the error now, I to the error so far, D to where the error is heading.",
                "P 看现在的误差，I 看累计的误差，D 看误差的走向。",
            ),
            reality: Tr::new(
                "PID loops run in drones, thermostats, cruise control and most of the world's industrial machines.",
                "无人机、恒温器、定速巡航以及世界上大多数工业设备里都在跑 PID 回路。",
            ),
            myth: Tr::new(
                "More gain is not always better: too much I makes the platform overshoot and wobble.",
                "增益并非越大越好：积分太强会让平台冲过头并来回晃。",
            ),
            simplified: Tr::new(
                "The weight here never changes mid-run and the motor never overheats.",
                "这里负载在运行中不会变化，电机也不会过热。",
            ),
        },
        ..stub("sag", 2, 3)
    }
}
