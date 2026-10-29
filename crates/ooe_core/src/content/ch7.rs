//! Chapter 7 · The Fog Harbour: sensors lie.

use super::*;

pub(super) fn tasks() -> Vec<Task> {
    vec![two_sensors(), how_far()]
}

fn tilt(seed: u64, amp: [f32; 2], freq: [f32; 2], bias: f32) -> Case {
    case(
        seed,
        Rig::Tilt(Tilt {
            amp,
            freq,
            phase: seed as f32 * 0.7,
            gyro_bias: bias,
            accel_noise: 0.25,
        }),
    )
}

fn two_sensors() -> Task {
    Task {
        ops: CONTROL,
        sensors: &[Sensor::Dt, Sensor::Accel, Sensor::Gyro],
        acts: &[Act::Est],
        duration: 10.0,
        pass: Pass::Track { rms: 0.065 },
        cases: || {
            vec![
                tilt(81, [0.3, 0.1], [1.2, 3.1], 0.08),
                tilt(82, [0.5, 0.05], [0.7, 4.0], -0.12),
                tilt(83, [0.2, 0.2], [2.0, 0.5], 0.05),
            ]
        },
        goals: [Goal::new(Metric::Error, 0.06), Goal::new(Metric::Size, 5.0)],
        starter: "\
; the accelerometer knows which way is down | 加速度计知道哪边朝下
MOV est, accel
",
        reference: "\
; predict with the gyro | 用陀螺仪预测
MUL r0, gyro, dt
ADD r0, est, r0
; correct a little towards the accelerometer | 向加速度计稍作修正
SUB r1, accel, r0
MUL r1, r1, 0.06
ADD est, r0, r1
",
        scope: &[Src::Sense(Sensor::Accel), Src::Out(Act::Est)],
        title: Tr::new("Steady Gaze", "稳定视线"),
        brief: Tr::new(
            "A robot head is being rocked around. It has two ways to tell which way is down.\n`accel` gives the tilt directly but shakes with every vibration. `gyro` gives the turn rate smoothly but has a small bias.\nWrite `est`, your best guess of the tilt, and keep it close to the truth.",
            "一个机器人头部被来回摇晃。它有两种方式判断哪边朝下。\n`accel` 直接给出倾角，但每次振动都会抖。`gyro` 平滑地给出转速，却带一点偏差。\n写出 `est`——你对倾角的最佳估计，让它紧跟真实值。",
        ),
        predict: Predict {
            question: Tr::new(
                "You estimate the tilt only by adding up `gyro × dt`. After a minute, what does the estimate look like?",
                "你只靠累加 `gyro × dt` 来估计倾角。一分钟后，估计值会怎样？",
            ),
            options: [
                Tr::new("Smooth and correct.", "平滑且准确。"),
                Tr::new(
                    "Smooth, but drifting further and further away.",
                    "平滑，但离真实值越漂越远。",
                ),
                Tr::new("Shaky, like the accelerometer.", "像加速度计一样抖动。"),
            ],
            answer: 1,
            why: Tr::new(
                "Integrating keeps the shape but also integrates the bias, so the error grows forever. That is why the accelerometer is still needed.",
                "积分保住了形状，但也把偏差一起积了进去，误差会一直增长。所以加速度计仍然不可或缺。",
            ),
        },
        hints: [
            Tr::new(
                "Each sensor is bad in a different way: one is noisy now, the other is wrong in the long run.",
                "两个传感器坏的方式不一样：一个短期抖，另一个长期偏。",
            ),
            Tr::new(
                "Predict the new tilt from `est + gyro·dt`, then nudge it a few percent towards `accel`.",
                "先用 `est + gyro·dt` 预测新倾角，再向 `accel` 修正百分之几。",
            ),
            Tr::new(
                "MUL r0, gyro, dt / ADD r0, est, r0 / SUB r1, accel, r0 / MUL r1, r1, 0.06 / ADD est, r0, r1",
                "MUL r0, gyro, dt / ADD r0, est, r0 / SUB r1, accel, r0 / MUL r1, r1, 0.06 / ADD est, r0, r1",
            ),
        ],
        insight: Insight {
            title: Tr::new("Sensor fusion", "传感器融合"),
            line: Tr::new(
                "Two bad sensors with different flaws can make one good estimate.",
                "两个缺陷不同的坏传感器，可以合成一个好的估计。",
            ),
            reality: Tr::new(
                "Every phone, drone and VR headset fuses a gyro and an accelerometer like this, usually with a Kalman filter.",
                "每部手机、无人机和 VR 头显都这样融合陀螺仪和加速度计，通常用卡尔曼滤波。",
            ),
            myth: Tr::new(
                "Filtering the accelerometer alone is not enough: smooth it enough to kill the shake and it lags behind.",
                "只对加速度计滤波是不够的：平滑到能消除抖动时，它就会明显滞后。",
            ),
            simplified: Tr::new(
                "The bias here is constant; real gyro bias wanders with temperature.",
                "这里的偏差是恒定的；真实陀螺仪的偏差会随温度漂移。",
            ),
        },
        ..stub("two-sensors", 7, 2)
    }
}

fn corridor(seed: u64, scale: f32) -> Case {
    case(
        seed,
        Rig::Corridor(Corridor {
            x: 0.0,
            v: 0.0,
            phase: seed as f32,
            scale,
        }),
    )
}

fn how_far() -> Task {
    Task {
        ops: CONTROL,
        sensors: &[
            Sensor::Dt,
            Sensor::Odom,
            Sensor::Fix,
            Sensor::Beacon,
            Sensor::Dist,
        ],
        acts: &[Act::Est],
        duration: 12.0,
        pass: Pass::Track { rms: 0.25 },
        cases: || vec![corridor(91, 0.12), corridor(92, -0.1), corridor(93, 0.15)],
        goals: [Goal::new(Metric::Error, 0.15), Goal::new(Metric::Size, 4.0)],
        starter: "\
; add up the wheel travel | 累加车轮走过的路程
MUL r0, odom, dt
ADD est, est, r0
",
        reference: "\
; dead reckoning | 航位推算
MUL r0, odom, dt
ADD r0, est, r0
; a beacon in view gives an absolute position | 看到信标就得到绝对位置
SUB r1, beacon, dist
SEL est, fix, r1, r0
",
        scope: &[Src::Out(Act::Est), Src::Sense(Sensor::Fix)],
        title: Tr::new("Where Am I?", "我在哪？"),
        brief: Tr::new(
            "SCOUT drives down a long corridor and must keep track of where it is in `est`.\nThe wheel counter `odom` is a little off: the wheels are not quite the size it thinks.\nNow and then a beacon comes into view: `fix` is 1, `beacon` is its known position and `dist` how far ahead it is.",
            "SCOUT 沿着长走廊行驶，必须在 `est` 里记住自己的位置。\n轮速计 `odom` 有点不准：轮子并不是它以为的那么大。\n时不时会看到一个信标：此时 `fix` 为 1，`beacon` 是信标的已知位置，`dist` 是它在前方多远。",
        ),
        predict: Predict {
            question: Tr::new(
                "The odometer reads 10% high. After driving 10 m with no beacons, how wrong is the estimate?",
                "里程计读数偏高 10%。不看信标开 10 米后，估计会差多少？",
            ),
            options: [
                Tr::new("About 10 cm.", "大约 10 厘米。"),
                Tr::new("About 1 m, and growing.", "大约 1 米，而且还在变大。"),
                Tr::new("It averages out to nothing.", "平均下来几乎为零。"),
            ],
            answer: 1,
            why: Tr::new(
                "A scale error never averages out: every metre adds 10 cm more. Dead reckoning always drifts; only an outside reference can reset it.",
                "比例误差永远不会抵消：每走一米就多出 10 厘米。航位推算一定会漂移，只有外部参照才能把它拉回来。",
            ),
        },
        hints: [
            Tr::new(
                "The starter adds up `odom × dt` and drifts. When `fix` is 1 you know exactly where you are.",
                "初始程序累加 `odom × dt`，会漂移。当 `fix` 为 1 时，你就确切知道自己在哪。",
            ),
            Tr::new(
                "The beacon is ahead of you by `dist`, so you are at `beacon - dist`. Use `SEL` to choose between that and dead reckoning.",
                "信标在你前方 `dist` 处，所以你在 `beacon - dist`。用 `SEL` 在它与航位推算之间选择。",
            ),
            Tr::new(
                "MUL r0, odom, dt / ADD r0, est, r0 / SUB r1, beacon, dist / SEL est, fix, r1, r0",
                "MUL r0, odom, dt / ADD r0, est, r0 / SUB r1, beacon, dist / SEL est, fix, r1, r0",
            ),
        ],
        insight: Insight {
            title: Tr::new("Localization", "定位"),
            line: Tr::new(
                "Count your steps to stay smooth, and use landmarks to stay right.",
                "数步子让估计平滑，看路标让估计正确。",
            ),
            reality: Tr::new(
                "Robot vacuums and self-driving cars combine wheel odometry with landmarks seen by cameras or lidar; building the map at the same time is SLAM.",
                "扫地机器人和自动驾驶汽车把轮式里程计与摄像头或激光雷达看到的路标结合；同时建图就是 SLAM。",
            ),
            myth: Tr::new(
                "GPS does not solve this indoors, and even outdoors it is only good to a few metres.",
                "GPS 在室内帮不上忙，即使在室外也只能精确到几米。",
            ),
            simplified: Tr::new(
                "SCOUT already knows where every beacon is and never mistakes one for another.",
                "SCOUT 事先知道每个信标的位置，也从不会认错信标。",
            ),
        },
        ..stub("how-far", 7, 3)
    }
}
