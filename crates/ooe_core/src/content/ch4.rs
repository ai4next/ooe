//! Chapter 4 · The Post Office: two wheels and a map.

use super::*;

pub(super) fn tasks() -> Vec<Task> {
    vec![follow_line()]
}

fn rover_case(seed: u64, amp: f32, k: f32) -> Case {
    let line = Line::wave(8.0, amp, k);
    let th = (amp * k).atan();
    case(
        seed,
        Rig::Rover(Rover {
            x: 0.0,
            y: 0.0,
            th,
            wheels: [0.0; 2],
            line,
            seg: 0,
            progress: 0.0,
            offset: 0.0,
            tangent: th,
            finished: false,
        }),
    )
}

fn follow_line() -> Task {
    Task {
        sensors: &[Sensor::Offset, Sensor::Heading],
        acts: &[Act::Left, Act::Right],
        duration: 15.0,
        pass: Pass::Finish,
        cases: || {
            vec![
                rover_case(21, 0.5, 0.9),
                rover_case(22, 0.8, 0.7),
                rover_case(23, 0.4, 1.6),
            ]
        },
        goals: [Goal::new(Metric::Time, 9.0), Goal::new(Metric::Error, 0.05)],
        starter: "\
; both wheels at the same speed: straight ahead | 两轮同速：直走
MOV left, 0.8
MOV right, 0.8
",
        reference: "\
; how hard to turn back towards the line | 往线上拐回去的力度
MUL r0, offset, 4
MUL r1, heading, 2
ADD r0, r0, r1
; left of the line: speed up the left wheel | 偏左：左轮加速
ADD left, 1.2, r0
SUB right, 1.2, r0
",
        scope: &[
            Src::Sense(Sensor::Offset),
            Src::Out(Act::Left),
            Src::Out(Act::Right),
        ],
        title: Tr::new("Line Follower", "循线小车"),
        brief: Tr::new(
            "ROVER has two wheels and a camera that sees the line underneath.\n`offset` is how far it sits left of the line, `heading` how far it points away from it.\nSteer by driving the wheels at different speeds, and reach the end of the track.",
            "ROVER 有两个轮子和一个看着脚下黑线的摄像头。\n`offset` 是它偏到线左边多远，`heading` 是它的朝向偏离线多少。\n让两个轮子转速不同来转向，开到赛道终点。",
        ),
        predict: Predict {
            question: Tr::new(
                "The left wheel spins faster than the right. Which way does the rover turn?",
                "左轮比右轮转得快。小车往哪边转？",
            ),
            options: [
                Tr::new("Left.", "向左。"),
                Tr::new("Right.", "向右。"),
                Tr::new("It goes straight, just faster.", "直走，只是更快。"),
            ],
            answer: 1,
            why: Tr::new(
                "The faster wheel travels further, so the rover swings around the slower one: it turns right. Tanks and wheelchairs steer the same way.",
                "快的轮子走得更远，小车绕着慢的那边转：向右转。坦克和轮椅也是这样转向的。",
            ),
        },
        hints: [
            Tr::new(
                "Same speed on both wheels goes straight. Add something to one side and subtract it from the other to turn.",
                "两轮同速就直走。给一边加、另一边减，就能转弯。",
            ),
            Tr::new(
                "Turn harder the further off the line you are. Looking at `heading` too stops the zig-zag.",
                "离线越远就转得越猛。同时看 `heading` 可以避免左右摇摆。",
            ),
            Tr::new(
                "MUL r0, offset, 4 / MUL r1, heading, 2 / ADD r0, r0, r1 / ADD left, 1.2, r0 / SUB right, 1.2, r0",
                "MUL r0, offset, 4 / MUL r1, heading, 2 / ADD r0, r0, r1 / ADD left, 1.2, r0 / SUB right, 1.2, r0",
            ),
        ],
        insight: Insight {
            title: Tr::new("Differential drive", "差速驱动"),
            line: Tr::new(
                "Two fixed wheels are enough to go anywhere on a floor: speed is their average, turning is their difference.",
                "两个固定的轮子就能在地面上去任何地方：速度是两轮的平均，转向是两轮的差。",
            ),
            reality: Tr::new(
                "Warehouse robots follow lines and floor codes this way, carrying shelves around all day.",
                "仓库机器人就是这样沿着地上的线和二维码行驶，整天搬运货架。",
            ),
            myth: Tr::new(
                "A robot that wobbles along a line is not broken; it is reacting too late or too hard.",
                "沿线摇摆的机器人并没有坏；它只是反应太晚或太猛。",
            ),
            simplified: Tr::new(
                "These wheels never slip and the camera sees the line perfectly; real rovers deal with dust and glare.",
                "这里的轮子从不打滑，摄像头也总能看清线；真实的小车要面对灰尘和反光。",
            ),
        },
        ..stub("follow-line", 4, 2)
    }
}
