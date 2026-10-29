//! Chapter 8 · Storm Day: reality gives no retries.

use super::*;

pub(super) fn tasks() -> Vec<Task> {
    vec![self_park(), it_worked_in_sim()]
}

fn dock(seed: u64, target: f32) -> Case {
    case(seed, Rig::Cart(servo_cart(0.0, target)))
}

fn self_park() -> Task {
    Task {
        ops: CONTROL,
        sensors: &[Sensor::Pos, Sensor::Vel, Sensor::Target, Sensor::Dock],
        acts: &[Act::Reward],
        duration: 8.0,
        pass: Pass::Settle { tol: 0.1 },
        cases: || {
            vec![
                dock(101, 1.5),
                dock(102, -1.0),
                dock(103, 0.7),
                dock(104, -1.8),
            ]
        },
        goals: [Goal::new(Metric::Time, 4.0), Goal::new(Metric::Size, 3.0)],
        starter: "\
; reward only for success | 只奖励成功
MOV reward, dock
",
        reference: "\
; closer is better, every tick | 每一步都是越近越好
SUB r0, target, pos
ABS r0, r0
SUB reward, dock, r0
",
        scope: &[
            Src::Sense(Sensor::Pos),
            Src::Sense(Sensor::Target),
            Src::Out(Act::Reward),
        ],
        learn: true,
        title: Tr::new("Learn to Dock", "学会停靠"),
        brief: Tr::new(
            "This time you do not write the controller. A learner tries random wheel speeds, thousands of times, and keeps what pays.\nYou write `reward`: how good each moment is. `dock` is 1 while the cart rests on its charger.\nPress Run to train, then watch what it learned.",
            "这次控制器不由你来写。一个学习器会成千上万次地乱试轮速，留下有回报的做法。\n你要写的是 `reward`：每一刻有多好。小车停在充电座上时 `dock` 为 1。\n按运行开始训练，然后看看它学会了什么。",
        ),
        predict: Predict {
            question: Tr::new(
                "The reward is 1 only when docked and 0 otherwise. What does the learner do?",
                "只有停靠成功时奖励为 1，否则为 0。学习器会怎样？",
            ),
            options: [
                Tr::new("Learns to dock quickly.", "很快学会停靠。"),
                Tr::new(
                    "Wanders aimlessly: it almost never finds the reward to learn from.",
                    "漫无目的地乱走：它几乎碰不到奖励，无从学起。",
                ),
                Tr::new("Drives away from the dock.", "朝远离充电座的方向开。"),
            ],
            answer: 1,
            why: Tr::new(
                "Random driving rarely stops exactly on the dock, so every attempt scores the same zero. Without a hint of 'warmer', there is nothing to learn.",
                "随机乱开很少恰好停在充电座上，每次尝试得分都是零。没有“越来越近”的提示，就没什么可学。",
            ),
        },
        hints: [
            Tr::new(
                "Look at the learning curve: with a sparse reward it stays flat. Give a little reward for getting closer.",
                "看学习曲线：稀疏奖励时它一直是平的。靠近一点就给一点奖励。",
            ),
            Tr::new(
                "Punish the distance to the target every tick, and keep `dock` as the prize. Careful: rewarding speed or position alone teaches the wrong thing.",
                "每一步都按到目标的距离扣分，再保留 `dock` 作为奖品。小心：只奖励速度或位置会教出错误的行为。",
            ),
            Tr::new(
                "SUB r0, target, pos / ABS r0, r0 / SUB reward, dock, r0",
                "SUB r0, target, pos / ABS r0, r0 / SUB reward, dock, r0",
            ),
        ],
        insight: Insight {
            title: Tr::new("Reward shaping", "奖励塑形"),
            line: Tr::new(
                "A learner optimises exactly what you reward, not what you meant.",
                "学习器优化的正是你奖励的东西，而不是你心里想的。",
            ),
            reality: Tr::new(
                "Reinforcement learning taught robots to walk, grasp and fly drones; designing the reward is often the hardest part.",
                "强化学习教会了机器人行走、抓取和驾驶无人机；设计奖励往往是最难的部分。",
            ),
            myth: Tr::new(
                "The robot does not understand the goal. Give it a loophole and it will find it.",
                "机器人并不理解目标。只要奖励有漏洞，它就会钻进去。",
            ),
            simplified: Tr::new(
                "This learner sees a 63-cell table and five wheel speeds; real ones use neural networks and millions of trials.",
                "这个学习器只看一张 63 格的表和五档轮速；真实的学习器用神经网络和上百万次尝试。",
            ),
        },
        ..stub("self-park", 8, 4)
    }
}

fn robot(seed: u64, real: Real, th: f32, pushes: &[(f32, f32)]) -> Case {
    Case {
        real,
        pushes: pushes.to_vec(),
        ..case(seed, Rig::Pole(pole(th, 0.0)))
    }
}

fn it_worked_in_sim() -> Task {
    Task {
        ops: CONTROL,
        sensors: &[Sensor::Pos, Sensor::Vel, Sensor::Angle, Sensor::Gyro],
        duration: 10.0,
        pass: Pass::Survive,
        cases: || {
            let r = |lag, noise, bias, mass, gain| Real {
                lag,
                noise,
                bias,
                mass,
                deadzone: 0.3,
                gain,
            };
            vec![
                robot(111, Real::default(), 0.1, &[(4.0, 0.5)]),
                robot(112, r(1, 0.01, 0.05, 1.3, 0.8), -0.1, &[(5.0, -0.5)]),
                robot(113, r(2, 0.05, -0.08, 0.8, 1.3), 0.08, &[(3.0, 0.4)]),
                robot(114, r(3, 0.04, 0.1, 1.5, 0.7), -0.05, &[(6.0, 0.6)]),
            ]
        },
        goals: [
            Goal::new(Metric::Error, 0.055),
            Goal::new(Metric::Effort, 50.0),
        ],
        starter: "\
; perfect in the simulator | 在仿真里完美无缺
MUL r0, angle, 40
; fall rate, from the angle sensor | 下落速度：由角度传感器求导
DERIV r1, angle
MUL r1, r1, 8
MUL r2, pos, 1
MUL r3, vel, 2
ADD r0, r0, r1
ADD r2, r2, r3
ADD motor, r0, r2
",
        reference: "\
MUL r0, angle, 40
; the gyro measures the rate directly: no noisy derivative | 陀螺仪直接测速率：不用对噪声求导
MUL r1, gyro, 10
MUL r2, pos, 1
MUL r3, vel, 2
ADD r0, r0, r1
ADD r2, r2, r3
ADD motor, r0, r2
",
        scope: &[
            Src::Sense(Sensor::Angle),
            Src::Sense(Sensor::Gyro),
            Src::Out(Act::Motor),
        ],
        live: Some(Live::Push),
        title: Tr::new("Sim to Real", "从仿真到现实"),
        brief: Tr::new(
            "The balance controller works perfectly in simulation. Now it goes onto four real robots.\nTheir motors lag and have a dead zone, their sensors are noisy and biased, and no two weigh the same.\nIt falls off the track on real hardware. Make one program that keeps all four up.",
            "平衡控制器在仿真里完美运行。现在要把它装到四台真实的机器人上。\n它们的电机有延迟和死区，传感器有噪声和偏差，重量也各不相同。\n在真机上它会冲出轨道。写一个能让四台都立住的程序。",
        ),
        predict: Predict {
            question: Tr::new(
                "You take the derivative of a slightly noisy angle sensor to get the fall rate. What does the result look like?",
                "你对带一点噪声的角度传感器求导来得到下落速度。结果会怎样？",
            ),
            options: [
                Tr::new("A clean fall rate.", "干净的下落速度。"),
                Tr::new(
                    "The same small noise, a little delayed.",
                    "同样小的噪声，略有延迟。",
                ),
                Tr::new(
                    "Huge spikes: the derivative amplifies the noise.",
                    "巨大的尖刺：求导会放大噪声。",
                ),
            ],
            answer: 2,
            why: Tr::new(
                "A derivative divides each tiny jump by a tiny time step, so 1 cm of noise becomes metres per second. The motor then shakes the robot apart.",
                "求导是把每个小跳变除以很小的时间步，于是 1 厘米的噪声会变成每秒好几米。电机随之剧烈抖动。",
            ),
        },
        hints: [
            Tr::new(
                "Compare the four robots on the scope. The motor command is jumping around: where does the noise come from?",
                "在示波器上比较四台机器人。电机指令在乱跳：噪声从哪来？",
            ),
            Tr::new(
                "There is a sensor that measures the fall rate directly. Use it instead of differentiating the angle.",
                "有一个传感器能直接测量下落速度。用它，而不是对角度求导。",
            ),
            Tr::new(
                "Replace `DERIV r1, angle / MUL r1, r1, 8` with `MUL r1, gyro, 10`.",
                "把 `DERIV r1, angle / MUL r1, r1, 8` 换成 `MUL r1, gyro, 10`。",
            ),
        ],
        insight: Insight {
            title: Tr::new("The reality gap", "现实差距"),
            line: Tr::new(
                "A controller is only as good as the worst robot it has to run on.",
                "控制器的好坏，取决于它要运行的最差的那台机器人。",
            ),
            reality: Tr::new(
                "Teams train robots on thousands of randomised simulators, with different masses, delays and noise, so the real one is just another variation.",
                "团队会在成千上万个随机化的仿真器里训练机器人，质量、延迟、噪声各不相同，这样真机只不过是又一种变体。",
            ),
            myth: Tr::new(
                "A perfect simulation score proves little; robust beats optimal once hardware is involved.",
                "仿真满分说明不了什么；一旦上了真机，鲁棒胜过最优。",
            ),
            simplified: Tr::new(
                "Real hardware also has backlash, cable drag, battery sag and wear. Four robots are a very small sample.",
                "真实硬件还有齿轮间隙、线缆拖拽、电池掉压和磨损。四台机器人只是很小的样本。",
            ),
        },
        clue: Some(Clue {
            signal: Src::Out(Act::Motor),
            // The noisy derivative first slams the motor to its limit.
            find: |s| {
                s.iter()
                    .find(|s| s.outs[Act::Motor as usize].abs() > POLE_FMAX)
                    .map(|s| s.t)
            },
            within: 0.3,
            what: Tr::new(
                "the first time the motor command hits its limit",
                "电机指令第一次撞到上限的时刻",
            ),
        }),
        ..stub("it-worked-in-sim", 8, 5)
    }
}
