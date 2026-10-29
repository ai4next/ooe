//! Chapter 1 · The Workshop: sensors in, motors out, and a loop in between.

use super::*;

pub(super) fn tasks() -> Vec<Task> {
    vec![eyes_open(), the_wall()]
}

fn eyes_open() -> Task {
    Task {
        title: Tr::new("Wake Up", "苏醒"),
        brief: Tr::new(
            "PIP has one wheel motor and one sensor: where it is.\nDrive it to the yellow flag and stop there.\nThe flag moves between tests, so read `target` instead of guessing.",
            "PIP 只有一个轮毂电机和一个传感器：自己在哪。\n把它开到黄旗处并停下。\n每次测试旗子位置都不同，所以要读 `target`，别猜。",
        ),
        ops: &[Op::Mov, Op::Sub, Op::Sel],
        duration: 6.0,
        pass: Pass::Settle { tol: 0.25 },
        cases: || {
            vec![
                case(1, Rig::Cart(servo_cart(0.0, 1.2))),
                case(2, Rig::Cart(servo_cart(0.0, 2.1))),
                case(3, Rig::Cart(servo_cart(0.0, 3.2))),
            ]
        },
        goals: [
            Goal::new(Metric::Time, 2.6),
            Goal::new(Metric::Overshoot, 0.05),
        ],
        starter: "\
; full speed ahead, forever | 全速前进，永不停止
MOV motor, 1
",
        reference: "\
; distance still to go | 剩余距离
SUB r0, target, pos
; cut the motor a little early: the wheels coast | 提前一点断电：轮子还会滑行
SUB r0, r0, 0.2
SEL motor, r0, 1.5, 0
",
        scope: &[
            Src::Sense(Sensor::Pos),
            Src::Sense(Sensor::Target),
            Src::Out(Act::Motor),
        ],
        predict: Predict {
            question: Tr::new(
                "You switch the motor off exactly when the cart reaches the flag. Where does it stop?",
                "小车恰好到达旗子时你关掉电机。它最终停在哪？",
            ),
            options: [
                Tr::new("Exactly on the flag.", "正好在旗子上。"),
                Tr::new("A little past the flag.", "比旗子稍远一点。"),
                Tr::new("A little before the flag.", "比旗子稍近一点。"),
            ],
            answer: 1,
            why: Tr::new(
                "A moving cart has momentum and the motor takes time to spin down, so it coasts past. Real controllers start braking early.",
                "运动的小车有惯性，电机也需要时间减速，所以会滑过头。真实的控制器会提前刹车。",
            ),
        },
        hints: [
            Tr::new(
                "`SUB r0, target, pos` is the distance left. `SEL` picks its second value when the first is positive.",
                "`SUB r0, target, pos` 就是剩余距离。`SEL` 在第一个值为正时选第二个值。",
            ),
            Tr::new(
                "Drive while the distance is positive, stop otherwise. Watch the scope: the cart keeps rolling after the motor stops.",
                "距离为正时开，否则停。看示波器：电机停了以后车还会继续滚。",
            ),
            Tr::new(
                "SUB r0, target, pos / SUB r0, r0, 0.2 / SEL motor, r0, 1.5, 0 — stop 0.2 m early and let it coast in.",
                "SUB r0, target, pos / SUB r0, r0, 0.2 / SEL motor, r0, 1.5, 0 —— 提前 0.2 米停电机，让它滑进去。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Closing the loop", "闭环"),
            line: Tr::new(
                "Reading a sensor every tick and deciding again is what makes a robot a robot.",
                "每个周期重新读传感器、重新决定，这才让机器人成为机器人。",
            ),
            reality: Tr::new(
                "Real motor controllers run this loop hundreds to thousands of times a second.",
                "真实的电机控制器每秒要跑几百到几千次这样的循环。",
            ),
            myth: Tr::new(
                "Robots do not follow a script of timed moves; they react to what they measure.",
                "机器人并不是照着定时动作的剧本走，而是对测量结果做出反应。",
            ),
            simplified: Tr::new(
                "Here the wheel speed follows the command with one smooth lag; real motors also slip and heat up.",
                "这里轮速只是平滑地滞后于指令；真实电机还会打滑、发热。",
            ),
        },
        ..stub("eyes-open", 1, 3)
    }
}

fn wall_case(seed: u64, wall: f32) -> Case {
    let mut c = servo_cart(0.0, wall - CART_HALF - 0.1);
    c.wall = Some(wall);
    case(seed, Rig::Cart(c))
}

fn the_wall() -> Task {
    Task {
        ops: &[
            Op::Mov,
            Op::Sub,
            Op::Mul,
            Op::Sel,
            Op::Min,
            Op::Max,
            Op::Clamp,
        ],
        sensors: &[Sensor::Dist],
        duration: 6.0,
        pass: Pass::Settle { tol: 0.03 },
        cases: || vec![wall_case(11, 2.0), wall_case(12, 3.0), wall_case(13, 3.7)],
        goals: [
            Goal::new(Metric::Time, 3.0),
            Goal::new(Metric::Overshoot, 0.01),
        ],
        starter: "\
; stop when the wall is 10 cm away | 离墙 10 厘米时停下
SUB r0, dist, 0.1
SEL motor, r0, 1, 0
",
        reference: "\
; gap left to close | 还要缩短的距离
SUB r0, dist, 0.1
; speed proportional to the gap | 速度与距离成正比
MUL motor, r0, 2
",
        scope: &[Src::Sense(Sensor::Dist), Src::Out(Act::Motor)],
        title: Tr::new("Mind the Wall", "小心墙壁"),
        brief: Tr::new(
            "PIP now has a range finder instead of a map: `dist` is the gap to the wall ahead.\nThis program is supposed to park 10 cm from the wall. It hits the wall instead.\nFind out why, and fix it.",
            "PIP 现在没有地图，只有一个测距仪：`dist` 是到前方墙壁的距离。\n这段程序本该停在离墙 10 厘米处，结果却撞了上去。\n找出原因，修好它。",
        ),
        predict: Predict {
            question: Tr::new(
                "The motor command is proportional to the gap: `motor = 2 × gap`. What happens as the cart gets close?",
                "电机指令与剩余距离成正比：`motor = 2 × 距离`。小车靠近墙时会怎样？",
            ),
            options: [
                Tr::new(
                    "It slows down smoothly and stops at the mark.",
                    "平滑减速，停在标记处。",
                ),
                Tr::new(
                    "It stops short and never arrives.",
                    "停在半路，永远到不了。",
                ),
                Tr::new("It bounces off the wall and comes back.", "撞墙后弹回来。"),
            ],
            answer: 0,
            why: Tr::new(
                "Less gap means less speed, so it brakes all the way in. That is proportional control: the correction shrinks as the error does.",
                "距离越小速度越小，所以它一路都在刹车。这就是比例控制：误差越小，修正也越小。",
            ),
        },
        hints: [
            Tr::new(
                "Watch the scope: the motor drops to 0 at 10 cm, but the cart is still moving fast. It needs room to stop.",
                "看示波器：电机在 10 厘米处才归零，可小车还跑得很快。它需要刹车距离。",
            ),
            Tr::new(
                "Instead of full speed or nothing, make the speed depend on how far is left.",
                "别只用全速或停，让速度取决于还剩多远。",
            ),
            Tr::new(
                "SUB r0, dist, 0.1 / MUL motor, r0, 2 — speed proportional to the gap.",
                "SUB r0, dist, 0.1 / MUL motor, r0, 2 —— 速度与剩余距离成正比。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Proportional control", "比例控制"),
            line: Tr::new(
                "Push in proportion to the error, and the push fades out exactly as you arrive.",
                "按误差的大小成比例地推，到达时推力正好消失。",
            ),
            reality: Tr::new(
                "Parking sensors, lift doors and robot vacuums all slow down as they close the gap.",
                "倒车雷达、电梯门、扫地机器人都会在接近时减速。",
            ),
            myth: Tr::new(
                "Stopping at the line is not enough: anything that moves needs distance to brake.",
                "在线上才停是不够的：任何运动的东西都需要刹车距离。",
            ),
            simplified: Tr::new(
                "The range finder here is perfect; real ones are noisy and miss dark or shiny walls.",
                "这里的测距仪是完美的；真实的会有噪声，还会漏掉深色或反光的墙。",
            ),
        },
        ..stub("the-wall", 1, 4)
    }
}
