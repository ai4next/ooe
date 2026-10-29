//! Chapter 3 · The School: joints and space.

use super::*;

pub(super) fn tasks() -> Vec<Task> {
    vec![reach()]
}

fn arm(seed: u64, t: [f32; 2]) -> Case {
    case(
        seed,
        Rig::Arm(Arm {
            q: [0.3, 0.6],
            path: Path::Fixed(t),
            nudge: [0.0; 2],
        }),
    )
}

fn reach() -> Task {
    Task {
        ops: ALL_OPS,
        sensors: &[Sensor::Tx, Sensor::Ty, Sensor::Q1, Sensor::Q2],
        acts: &[Act::Shoulder, Act::Elbow],
        duration: 4.0,
        pass: Pass::Settle { tol: 0.02 },
        cases: || {
            vec![
                arm(61, [1.2, 0.9]),
                arm(62, [-0.6, 1.3]),
                arm(63, [1.7, -0.3]),
                arm(64, [0.4, 0.5]),
            ]
        },
        goals: [Goal::new(Metric::Time, 1.0), Goal::new(Metric::Size, 12.0)],
        starter: "\
; point the whole arm at the target | 整条手臂指向目标
ATAN2 shoulder, ty, tx
MOV elbow, 0
",
        reference: "\
; squared distance to the target | 到目标距离的平方
MUL r0, tx, tx
MUL r1, ty, ty
ADD r0, r0, r1
; law of cosines gives the elbow | 余弦定理求出肘关节角
SUB r0, r0, 2
MUL r0, r0, 0.5
ACOS elbow, r0
; aim at the target, minus the bend's offset | 对准目标，再减去弯曲带来的偏角
SIN r1, elbow
COS r2, elbow
ADD r2, r2, 1
ATAN2 r1, r1, r2
ATAN2 r3, ty, tx
SUB shoulder, r3, r1
",
        scope: &[
            Src::Sense(Sensor::Q1),
            Src::Sense(Sensor::Q2),
            Src::Out(Act::Elbow),
        ],
        live: Some(Live::Nudge),
        title: Tr::new("Reach", "伸手"),
        brief: Tr::new(
            "ARM has two 1 m links: a shoulder and an elbow. You command joint angles; the servos do the rest.\nThe target is at (`tx`, `ty`) from the shoulder. Put the fingertip on it.\nYou think in places, the motors think in angles: translate.",
            "ARM 有两节 1 米长的连杆：肩关节和肘关节。你下达关节角度，舵机负责转过去。\n目标在相对肩部 (`tx`, `ty`) 的位置。让指尖碰到它。\n你想的是位置，电机懂的是角度：做个翻译。",
        ),
        predict: Predict {
            question: Tr::new(
                "The target is 1.5 m away. You point the shoulder straight at it and keep the elbow straight. What happens?",
                "目标在 1.5 米外。你让肩部直指目标，肘部保持伸直。结果如何？",
            ),
            options: [
                Tr::new("The fingertip lands on the target.", "指尖正好碰到目标。"),
                Tr::new(
                    "The fingertip goes past the target, in line with it.",
                    "指尖越过目标，和它在一条线上。",
                ),
                Tr::new("The arm cannot point there at all.", "手臂根本指不过去。"),
            ],
            answer: 1,
            why: Tr::new(
                "A straight arm is 2 m long, so it overshoots along the same line. Reaching closer means bending the elbow, which swings the tip off the line, so the shoulder must compensate.",
                "伸直的手臂有 2 米长，所以会沿着同一直线伸过头。要够得近就得弯肘，而弯肘会让指尖偏离直线，于是肩部必须补偿。",
            ),
        },
        hints: [
            Tr::new(
                "First find the elbow bend from the distance alone. With two 1 m links: d² = 2 + 2·cos(elbow).",
                "先只用距离算出肘部弯曲角。两节 1 米连杆时：d² = 2 + 2·cos(肘角)。",
            ),
            Tr::new(
                "Then aim the shoulder at the target and subtract the angle the bend adds: atan2(sin e, 1 + cos e).",
                "再让肩部对准目标，减去弯曲带来的偏角：atan2(sin e, 1 + cos e)。",
            ),
            Tr::new(
                "MUL r0, tx, tx / MUL r1, ty, ty / ADD r0, r0, r1 / SUB r0, r0, 2 / MUL r0, r0, 0.5 / ACOS elbow, r0 / SIN r1, elbow / COS r2, elbow / ADD r2, r2, 1 / ATAN2 r1, r1, r2 / ATAN2 r3, ty, tx / SUB shoulder, r3, r1",
                "MUL r0, tx, tx / MUL r1, ty, ty / ADD r0, r0, r1 / SUB r0, r0, 2 / MUL r0, r0, 0.5 / ACOS elbow, r0 / SIN r1, elbow / COS r2, elbow / ADD r2, r2, 1 / ATAN2 r1, r1, r2 / ATAN2 r3, ty, tx / SUB shoulder, r3, r1",
            ),
        ],
        insight: Insight {
            title: Tr::new("Inverse kinematics", "逆运动学"),
            line: Tr::new(
                "Forward kinematics turns angles into a place; inverse kinematics turns a place back into angles.",
                "正运动学把角度变成位置；逆运动学把位置变回角度。",
            ),
            reality: Tr::new(
                "Factory arms, animation rigs and VR avatars all solve IK many times a second.",
                "工厂机械臂、动画骨骼和 VR 虚拟形象每秒都要解很多次逆运动学。",
            ),
            myth: Tr::new(
                "There is usually more than one answer: this arm can reach most points elbow-up or elbow-down.",
                "答案通常不止一个：这条手臂够到大多数点时，肘部朝上朝下都行。",
            ),
            simplified: Tr::new(
                "Two joints in a plane have a formula; arms with six or seven joints need iterative solvers.",
                "平面上的两关节有公式可解；六七个关节的手臂要用迭代求解。",
            ),
        },
        ..stub("reach", 3, 3)
    }
}
