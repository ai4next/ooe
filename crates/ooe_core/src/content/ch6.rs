//! Chapter 6 · The Reef: walking is rhythmic falling.

use super::*;

pub(super) fn tasks() -> Vec<Task> {
    vec![left_right()]
}

fn walker(goal: f32) -> Rig {
    Rig::Walker(Walker {
        x: 0.0,
        v: 0.0,
        hip: [0.0; 2],
        lift: [0.0; 2],
        goal,
        air: 0.0,
    })
}

fn left_right() -> Task {
    Task {
        ops: &[
            Op::Mov,
            Op::Add,
            Op::Sub,
            Op::Mul,
            Op::Sel,
            Op::Min,
            Op::Max,
            Op::Abs,
            Op::Sin,
            Op::Cos,
        ],
        sensors: &[Sensor::T, Sensor::Pos, Sensor::Q1, Sensor::Q2],
        acts: &[Act::HipL, Act::HipR, Act::LiftL, Act::LiftR],
        duration: 8.0,
        pass: Pass::Finish,
        cases: || vec![case(31, walker(2.0)), case(32, walker(2.6)), case(33, walker(3.2))],
        goals: [Goal::new(Metric::Time, 2.0), Goal::new(Metric::Size, 7.0)],
        starter: "\
; swing both legs together | 两条腿一起摆
MUL r0, t, 5
SIN r0, r0
MUL hip_l, r0, 0.5
MUL hip_r, r0, 0.5
",
        reference: "\
; a rhythm: the phase of the stride | 节律：步伐的相位
MUL r0, t, 6
SIN r1, r0
; legs in opposition | 两腿相反
MUL hip_l, r1, 0.6
MUL hip_r, r1, -0.6
; lift whichever leg is swinging forward | 抬起正在向前摆的那条腿
COS r2, r0
SEL lift_l, r2, 1, 0
SEL lift_r, r2, 0, 1
",
        scope: &[
            Src::Sense(Sensor::Q1),
            Src::Sense(Sensor::Q2),
            Src::Out(Act::LiftL),
        ],
        title: Tr::new("First Steps", "第一步"),
        brief: Tr::new(
            "STRIDER has two legs, a hip motor on each and a foot it can lift.\nA planted foot pushes the body forward as the hip swings back. Walk to the flag.\nIf both feet leave the ground for long, it falls.",
            "STRIDER 有两条腿，每条腿一个髋关节电机，脚可以抬起。\n着地的脚在髋关节向后摆时把身体往前推。走到旗子处。\n如果两只脚同时离地太久，它就会摔倒。",
        ),
        predict: Predict {
            question: Tr::new(
                "Both legs swing forward and back together, feet always on the ground. How far does it get?",
                "两条腿一起前后摆，脚始终着地。它能走多远？",
            ),
            options: [
                Tr::new(
                    "Nowhere: it rocks back and forth on the spot.",
                    "原地前后晃，哪儿也去不了。",
                ),
                Tr::new("Forward, but slowly.", "向前走，但很慢。"),
                Tr::new("It falls over.", "摔倒。"),
            ],
            answer: 0,
            why: Tr::new(
                "Each forward swing undoes the backward one. Walking needs asymmetry: push while planted, lift while returning.",
                "每次向前摆都抵消了向后摆。行走需要不对称：着地时蹬，收回时抬脚。",
            ),
        },
        hints: [
            Tr::new(
                "A sine of time makes a rhythm. Give the two hips opposite signs so one pushes while the other returns.",
                "时间的正弦就是节律。让两个髋关节符号相反，一条腿蹬时另一条腿收。",
            ),
            Tr::new(
                "Lift the leg that is swinging forward. The cosine of the same phase tells you which one that is.",
                "抬起正在向前摆的那条腿。同一相位的余弦告诉你是哪一条。",
            ),
            Tr::new(
                "MUL r0, t, 6 / SIN r1, r0 / MUL hip_l, r1, 0.6 / MUL hip_r, r1, -0.6 / COS r2, r0 / SEL lift_l, r2, 1, 0 / SEL lift_r, r2, 0, 1",
                "MUL r0, t, 6 / SIN r1, r0 / MUL hip_l, r1, 0.6 / MUL hip_r, r1, -0.6 / COS r2, r0 / SEL lift_l, r2, 1, 0 / SEL lift_r, r2, 0, 1",
            ),
        ],
        insight: Insight {
            title: Tr::new("Gaits and rhythm", "步态与节律"),
            line: Tr::new(
                "A gait is a rhythm plus a rule for which foot carries the weight.",
                "步态就是一个节律，加上一条“由哪只脚承重”的规则。",
            ),
            reality: Tr::new(
                "Animals generate walking rhythms in the spinal cord; legged robots copy the idea with oscillators called CPGs.",
                "动物的行走节律由脊髓产生；足式机器人用称为 CPG 的振荡器模仿这个思路。",
            ),
            myth: Tr::new(
                "Walking is not planned step by step from scratch; most of it is a repeating pattern, adjusted on the fly.",
                "行走并不是每一步都从头规划；大部分是重复的模式，边走边调整。",
            ),
            simplified: Tr::new(
                "STRIDER cannot tip over sideways or stumble; it only falls if both feet stay in the air.",
                "STRIDER 不会侧翻也不会绊倒；只有双脚都悬空太久才会摔。",
            ),
        },
        ..stub("left-right", 6, 2)
    }
}
