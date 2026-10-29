//! Chapter 5 · The Clinic: keeping unstable things up.

use super::*;

pub(super) fn tasks() -> Vec<Task> {
    vec![
        falling(),
        stay_on_rail(),
        four_gains(),
        swing_up(),
        shivers(),
        the_tray(),
    ]
}

fn pole_case(seed: u64, th: f32, pushes: &[(f32, f32)]) -> Case {
    Case {
        pushes: pushes.to_vec(),
        ..case(seed, Rig::Pole(pole(th, 0.0)))
    }
}

/// POLE at `x` leaning `th`, with sensor/motor reality and shoves.
fn pole_at(seed: u64, th: f32, x: f32, real: Real, pushes: &[(f32, f32)]) -> Case {
    Case {
        real,
        pushes: pushes.to_vec(),
        ..case(seed, Rig::Pole(pole(th, x)))
    }
}

/// The clinic corridor: noisy sensors and a track that ends 1 m from centre.
fn corridor(seed: u64, th: f32, x: f32, pushes: &[(f32, f32)]) -> Case {
    let real = Real {
        noise: 0.01,
        ..Real::default()
    };
    let mut c = pole_at(seed, th, x, real, pushes);
    if let Rig::Pole(p) = &mut c.rig {
        p.rail = 1.0;
    }
    c
}

/// POLE hanging at `th` (near ±π), free to swing all the way round.
fn hanging(seed: u64, th: f32, x: f32) -> Case {
    let mut c = pole_at(seed, th, x, Real::default(), &[]);
    if let Rig::Pole(p) = &mut c.rig {
        p.fall = f32::INFINITY;
    }
    c
}

fn falling() -> Task {
    Task {
        ops: &[Op::Mov, Op::Add, Op::Sub, Op::Mul, Op::Sel],
        sensors: &[Sensor::Angle, Sensor::Gyro],
        duration: 5.0,
        pass: Pass::Survive,
        cases: || {
            vec![
                pole_case(51, 0.1, &[]),
                pole_case(52, -0.15, &[]),
                pole_case(53, 0.05, &[(2.0, -0.6)]),
            ]
        },
        goals: [Goal::new(Metric::Effort, 2.0), Goal::new(Metric::Size, 3.0)],
        starter: "\
; push against the lean, to tip it back | 顶着倾斜的方向推，想把它推回去
MUL motor, angle, -30
",
        reference: "\
; the lean, plus where it is heading | 倾角，再加上它正往哪边倒
MUL r0, gyro, 0.2
ADD r0, r0, angle
; drive the cart under it | 把小车开到它下面
MUL motor, r0, 40
",
        scope: &[Src::Sense(Sensor::Angle), Src::Out(Act::Motor)],
        title: Tr::new("Bound to Fall", "必然倒下"),
        brief: Tr::new(
            "Dr. Lina wants a cart that carries the medicine tray down the clinic corridor without spilling. I said yes before I thought about it.\nThis is POLE: a cart with a pole standing on top. Nothing so far was like it. Leave it alone and it does not settle: it falls.\nKeep the pole up for 5 seconds. `angle` is the lean, `gyro` how fast it is falling.",
            "莉娜医生想要一台车，端着药盘穿过诊所走廊，一滴不洒。我没多想就答应了。\n这是 POLE：一辆小车，上面立着一根杆。它和之前的一切都不一样：放着不管，它不会自己停稳，只会倒下。\n让杆立住 5 秒。`angle` 是倾角，`gyro` 是它倒下的速度。",
        ),
        predict: Predict {
            question: Tr::new(
                "POLE leans 0.1 rad and the motor is off. What happens to the lean?",
                "POLE 倾斜 0.1 rad，电机关着。倾角会怎样？",
            ),
            options: [
                Tr::new("It stays at 0.1.", "保持在 0.1。"),
                Tr::new(
                    "It swings back to upright, like a pendulum.",
                    "像钟摆一样摆回竖直。",
                ),
                Tr::new(
                    "It grows faster and faster until the pole falls.",
                    "越变越快，直到杆倒下。",
                ),
            ],
            answer: 2,
            why: Tr::new(
                "Upside down, gravity pulls harder the further the pole leans, so a small lean grows on its own. Doing nothing is never safe.",
                "倒立时，杆倾得越多，重力拉得越狠，所以小小的倾角会自己越变越大。什么都不做永远不安全。",
            ),
        },
        hints: [
            Tr::new(
                "The starter pushes against the lean, and the lean grows faster. Flip the sign: drive the cart towards the side the pole leans.",
                "初始程序顶着倾斜的方向推，倾角反而长得更快。把符号反过来：车往杆倒的那边开。",
            ),
            Tr::new(
                "Chasing the lean alone works but wastes effort: the cart always arrives late. Add some `gyro` to react to the fall before the lean builds up.",
                "只追倾角也能立住，但很费力：小车总是晚到一步。加一点 `gyro`，在倾角变大之前就对下落做出反应。",
            ),
            Tr::new(
                "`MUL r0, gyro, 0.2` / `ADD r0, r0, angle` / `MUL motor, r0, 40`.",
                "`MUL r0, gyro, 0.2` / `ADD r0, r0, angle` / `MUL motor, r0, 40`。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Chasing the fall", "追着倒的方向"),
            line: Tr::new(
                "You cannot push an unstable thing back up; you have to get underneath it, and keep getting underneath it.",
                "不稳定的东西推不回去；你得钻到它下面去，并且一直钻到它下面去。",
            ),
            reality: Tr::new(
                "A cyclist steers into the fall, a rocket swivels its engine under its weight, and a waiter walks under a wobbling tray.",
                "骑车的人往倒的方向打把，火箭把发动机摆到重心下方，服务员会跟着摇晃的托盘走。",
            ),
            myth: Tr::new(
                "Pushing against the lean feels like correcting it. It moves the cart out from under the weight and the pole tips faster.",
                "顶着倾斜推，感觉像在纠正。其实那是把小车从重心下面挪开，杆倒得更快。",
            ),
            simplified: Tr::new(
                "POLE's sensors are perfect, its motor instant and its track long. The rest of this chapter takes those away one by one.",
                "这里 POLE 的传感器是完美的，电机瞬时响应，轨道也够长。本章接下来会把这些一样样拿走。",
            ),
        },
        ..stub("falling", 5, 1)
    }
}

fn stay_on_rail() -> Task {
    Task {
        ops: CONTROL,
        sensors: &[Sensor::Pos, Sensor::Vel, Sensor::Angle, Sensor::Gyro],
        duration: 10.0,
        pass: Pass::Survive,
        cases: || {
            vec![
                corridor(61, 0.1, 0.0, &[(4.0, 0.6)]),
                corridor(62, -0.1, 0.5, &[(3.0, -0.8)]),
                corridor(63, 0.05, -0.6, &[(2.0, 0.8), (6.0, -0.8)]),
            ]
        },
        goals: [
            Goal::new(Metric::Error, 0.045),
            Goal::new(Metric::Effort, 10.0),
        ],
        starter: "\
; balance only: lean and fall rate | 只管平衡：倾角与下落速度
MUL r0, angle, 40
MUL r1, gyro, 8
ADD motor, r0, r1
",
        reference: "\
; outer loop: how far and how fast from home | 外环：离中间多远、多快
MUL r0, pos, 0.03
MUL r1, vel, 0.05
ADD r0, r0, r1
; inner loop: balance, offset by the outer loop | 内环：平衡，加上外环的偏移
ADD r0, angle, r0
MUL r0, r0, 40
MUL r1, gyro, 8
ADD motor, r0, r1
",
        scope: &[
            Src::Sense(Sensor::Angle),
            Src::Sense(Sensor::Pos),
            Src::Out(Act::Motor),
        ],
        title: Tr::new("Stay on the Rail", "别跑出轨道"),
        brief: Tr::new(
            "POLE stays up now, and wanders off while it does. The clinic corridor is narrow: here the track ends 1 m either side of centre.\nKeep the pole up for 10 seconds without leaving the track. Someone will shove it, and the sensors are a little noisy.\nYou cannot just drive the cart home: that tips the pole. Ask the pole to lean towards home, and let the balance loop carry the cart there.",
            "POLE 现在立得住了，可一边立一边往外溜。诊所的走廊很窄：这里的轨道在中心两侧 1 m 处就到头了。\n让杆立住 10 秒，而且不能出轨道。中途会有人推它，传感器也有点噪声。\n不能直接把车开回中间：那会把杆弄倒。让杆朝中间倾一点，再让平衡回路把小车带过去。",
        ),
        predict: Predict {
            question: Tr::new(
                "POLE is upright, 0.5 m right of centre. To bring it home, which way should the pole lean first?",
                "POLE 直立着，在中心右侧 0.5 m。要让它回到中间，杆应该先往哪边倾？",
            ),
            options: [
                Tr::new(
                    "Left, towards home: the cart steps right for a moment to tip it.",
                    "向左，朝着中间：小车先往右挪一下把它弄倾。",
                ),
                Tr::new("Right, away from home.", "向右，背离中间。"),
                Tr::new(
                    "It should stay upright while the cart drives left.",
                    "保持竖直，小车直接往左开。",
                ),
            ],
            answer: 0,
            why: Tr::new(
                "To move a balanced pole you must tilt it the way you want to go, and the cart then follows the lean. So the outer loop asks for a lean; the inner loop makes it happen.",
                "要移动一根立着的杆，得先让它朝想去的方向倾，小车再跟着倾角走。所以外环要一个倾角，内环负责实现它。",
            ),
        },
        hints: [
            Tr::new(
                "Watch `pos` on the scope: the pole stays up, but the cart drifts until it runs off the end of the track.",
                "看示波器上的 `pos`：杆是立住了，小车却一直漂，直到冲出轨道尽头。",
            ),
            Tr::new(
                "Turn position into a lean: add a little `pos` and `vel` to `angle` before the balance gain. Too much and the inner loop cannot keep up.",
                "把位置变成倾角：在乘平衡增益之前，给 `angle` 加上一点 `pos` 和 `vel`。加多了，内环会跟不上。",
            ),
            Tr::new(
                "(angle + 0.03·pos + 0.05·vel)·40 + 8·gyro → motor.",
                "(angle + 0.03·pos + 0.05·vel)·40 + 8·gyro → motor。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Cascade control", "串级控制"),
            line: Tr::new(
                "An outer loop sets the goal of an inner loop: position asks for a lean, and the lean loop moves the cart.",
                "外环给内环定目标：位置要一个倾角，倾角回路再去移动小车。",
            ),
            reality: Tr::new(
                "A drone nests its loops: position sets velocity, velocity sets tilt, tilt sets the motor speeds. Each inner loop is faster than the one around it.",
                "无人机的回路是一层套一层的：位置决定速度，速度决定倾角，倾角决定电机转速。每个内环都比外面那层快。",
            ),
            myth: Tr::new(
                "Controlling only the angle feels like the whole job. The pole stays up, and the robot walks off the edge.",
                "只控角度，感觉就是全部工作了。杆是立住了，机器人却走下了边缘。",
            ),
            simplified: Tr::new(
                "Real cascades run the inner loop many times faster than the outer one; here both run every tick.",
                "真实的串级控制里，内环比外环快很多倍；这里两者每个 tick 都跑一次。",
            ),
        },
        ..stub("stay-on-rail", 5, 2)
    }
}

fn four_gains() -> Task {
    Task {
        ops: CONTROL,
        sensors: &[Sensor::Pos, Sensor::Vel, Sensor::Angle, Sensor::Gyro],
        duration: 10.0,
        pass: Pass::Survive,
        cases: || {
            vec![
                pole_case(71, 0.1, &[]),
                pole_case(72, -0.15, &[(3.0, 0.6)]),
                pole_case(73, 0.05, &[(2.0, -0.8), (6.0, 0.8)]),
            ]
        },
        goals: [
            Goal::new(Metric::Error, 0.05),
            Goal::new(Metric::Effort, 40.0),
        ],
        starter: "\
; push the cart the way the pole leans | 杆往哪边倒，就往哪边推车
SEL motor, angle, 20, -20
",
        reference: "\
; catch the pole: lean and fall rate | 接住杆：倾角与下落速度
MUL r0, angle, 40
MUL r1, gyro, 8
; then drift the cart home | 再把小车慢慢带回中间
MUL r2, pos, 1
MUL r3, vel, 2
ADD r0, r0, r1
ADD r2, r2, r3
ADD motor, r0, r2
",
        scope: &[
            Src::Sense(Sensor::Angle),
            Src::Sense(Sensor::Pos),
            Src::Out(Act::Motor),
        ],
        live: Some(Live::Push),
        title: Tr::new("Balance", "平衡"),
        brief: Tr::new(
            "POLE is a cart with a free-swinging pole standing on top. Gravity wants it down.\nKeep the pole up for 10 seconds by moving the cart under it. Someone will shove it.\n`angle` is the lean, `gyro` how fast it is falling.",
            "POLE 是一辆小车，车上立着一根可以自由摆动的杆。重力想让它倒下。\n移动小车去托住它，让杆立满 10 秒。中途会有人推它。\n`angle` 是倾角，`gyro` 是它倒下的速度。",
        ),
        predict: Predict {
            question: Tr::new(
                "The pole leans right. Which way should the cart move to catch it?",
                "杆向右倾。小车应该往哪边动才能接住它？",
            ),
            options: [
                Tr::new("Left, to pull it back.", "向左，把它拉回来。"),
                Tr::new("Right, to get back under it.", "向右，回到它下面。"),
                Tr::new("Stay still and wait.", "不动，等着。"),
            ],
            answer: 1,
            why: Tr::new(
                "Like balancing a broom on your hand: you move your hand towards the lean, so the base gets back under the weight.",
                "就像在手上立扫帚：手往倒的方向移，让底部回到重心下方。",
            ),
        },
        hints: [
            Tr::new(
                "Full force left or right makes the cart chatter and wastes energy. Push in proportion to the lean.",
                "左右全力推会让小车抖个不停，还浪费能量。按倾角成比例地推。",
            ),
            Tr::new(
                "Add `gyro` so you react to the fall before the lean grows. Small terms on `pos` and `vel` stop the cart wandering off the track.",
                "加上 `gyro`，在倾角变大之前就对下落做出反应。再给 `pos` 和 `vel` 加一点小权重，防止小车跑出轨道。",
            ),
            Tr::new(
                "40·angle + 8·gyro + 1·pos + 2·vel → motor.",
                "40·angle + 8·gyro + 1·pos + 2·vel → motor。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Unstable balance", "不稳定平衡"),
            line: Tr::new(
                "Some systems fall over unless you keep acting; control turns an unstable thing into a stable one.",
                "有些系统不持续干预就会倒；控制能把不稳定的东西变得稳定。",
            ),
            reality: Tr::new(
                "Self-balancing scooters, rockets on ascent and walking robots all solve an inverted pendulum.",
                "自平衡车、上升中的火箭和行走机器人都在解倒立摆问题。",
            ),
            myth: Tr::new(
                "Balancing is not about staying still: the cart is always moving a little to stay under the pole.",
                "平衡并不是静止不动：小车一直在微微移动，好待在杆的下方。",
            ),
            simplified: Tr::new(
                "The sensors are perfect and the motor responds instantly. The Field will take that away.",
                "这里的传感器是完美的，电机也是瞬时响应的。试验场会把这些都拿走。",
            ),
        },
        ..stub("four-gains", 5, 3)
    }
}

/// Arithmetic, `COS` for the swing energy, a filter and a PID.
const SWING: &[Op] = &[
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
    Op::Cos,
    Op::Lpf,
    Op::Pid,
];

fn swing_up() -> Task {
    Task {
        ops: SWING,
        sensors: &[Sensor::Pos, Sensor::Vel, Sensor::Angle, Sensor::Gyro],
        duration: 12.0,
        pass: Pass::Settle { tol: 0.1 },
        cases: || {
            vec![
                hanging(81, 3.1, 0.0),
                hanging(82, -3.0, 0.5),
                hanging(83, 3.2, -0.5),
            ]
        },
        limit: Some(Goal::new(Metric::Time, 7.0)),
        goals: [
            Goal::new(Metric::Time, 4.0),
            Goal::new(Metric::Effort, 16.0),
        ],
        starter: "\
; the balancer from before | 之前的平衡控制器
MUL r0, angle, 40
MUL r1, gyro, 8
MUL r2, pos, 1
MUL r3, vel, 2
ADD r0, r0, r1
ADD r2, r2, r3
ADD motor, r0, r2
",
        reference: "\
; near the top: balance | 接近顶端：平衡
MUL r0, angle, 40
MUL r1, gyro, 8
ADD r0, r0, r1
MUL r1, pos, 1
ADD r0, r0, r1
MUL r1, vel, 2
ADD r0, r0, r1
; swing energy, zero at the top | 摆动能量，在顶端为零
COS r2, angle
MUL r3, gyro, gyro
MUL r3, r3, 0.034
ADD r3, r3, r2
SUB r3, r3, 1
; pump in step with the swing | 顺着摆动去推
MUL r4, gyro, r2
MUL r4, r4, r3
MUL r4, r4, 6
; keep the cart near the middle | 让小车留在中间附近
MUL r5, pos, 2
SUB r4, r4, r5
MUL r5, vel, 2
SUB r4, r4, r5
CLAMP r4, r4, 5
; balance when close, pump otherwise | 靠近就平衡，否则继续摆
ABS r5, angle
SUB r5, r5, 0.5
SEL motor, r5, r4, r0
",
        scope: &[
            Src::Sense(Sensor::Angle),
            Src::Sense(Sensor::Pos),
            Src::Out(Act::Motor),
        ],
        title: Tr::new("Swing Up", "摆起来"),
        brief: Tr::new(
            "Dr. Lina asked what happens when POLE falls over in the corridor. It lies there. So do I, if I fall: that is why I only have one arm.\nThis time POLE starts hanging straight down, free to swing all the way round. Swing it up and keep it balanced for the last 5 seconds.\nA balancer only works near the top. Pump energy into the swing first, like a child on a swing, then catch it. `angle` reads ±3.14 hanging down; `COS` is unlocked.",
            "莉娜医生问：POLE 要是在走廊里倒了怎么办？它就躺在那儿。我倒了也一样：所以我只有一条手臂。\n这次 POLE 一开始就垂直挂着，可以一直转一整圈。把它摆上去，并在最后 5 秒保持平衡。\n平衡控制器只在顶端附近管用。先像荡秋千一样往摆动里灌能量，再接住它。杆垂下时 `angle` 读数是 ±3.14；`COS` 已解锁。",
        ),
        predict: Predict {
            question: Tr::new(
                "You want the pole to swing a little higher each time. When should the cart push?",
                "你想让杆每次都摆得更高一点。小车应该什么时候推？",
            ),
            options: [
                Tr::new(
                    "Always the same way, as hard as it can.",
                    "一直往同一个方向，用尽全力。",
                ),
                Tr::new(
                    "In step with the swing, like pushing a child on a swing.",
                    "跟着摆动的节拍，就像推秋千上的孩子。",
                ),
                Tr::new(
                    "Only at the bottom, when the pole is fastest.",
                    "只在最低点推，那时杆最快。",
                ),
            ],
            answer: 1,
            why: Tr::new(
                "A push in step with the swing adds a little energy every time; a steady push just moves the cart off the track. Once the swing carries the pole near the top, a balancer can catch it.",
                "跟着节拍推，每次都能添一点能量；一直往一边推只会把小车推出轨道。等摆动把杆带到顶端附近，平衡控制器就能接住它。",
            ),
        },
        hints: [
            Tr::new(
                "The balancer fights the hanging pole and gets nowhere. Near the top it is the right tool; far from it you need another. `SEL` can switch between them on |angle|.",
                "平衡控制器跟垂着的杆较劲，毫无进展。在顶端附近它是对的工具；离得远时你需要另一个。`SEL` 可以根据 |angle| 在两者之间切换。",
            ),
            Tr::new(
                "Pump in proportion to `gyro`·cos(`angle`), clamped to a few newtons, and stop pumping once there is enough energy: E = 0.034·gyro² + cos(angle) − 1 is zero at the top. Keep the cart near the middle while you pump.",
                "按 `gyro`·cos(`angle`) 成比例地推，限幅在几牛顿以内；能量够了就停：E = 0.034·gyro² + cos(angle) − 1 在顶端为零。摆的时候让小车留在中间附近。",
            ),
            Tr::new(
                "Pump = clamp(6·gyro·cos(angle)·E − 2·pos − 2·vel, 5). Balance = 40·angle + 8·gyro + 1·pos + 2·vel. Use the balancer when |angle| < 0.5.",
                "摆动 = clamp(6·gyro·cos(angle)·E − 2·pos − 2·vel, 5)。平衡 = 40·angle + 8·gyro + 1·pos + 2·vel。|angle| < 0.5 时改用平衡控制器。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Swing up, then switch", "先摆起，再切换"),
            line: Tr::new(
                "Two controllers, each good in its own region, and a rule for handing over: far from the goal, pump energy; near it, balance.",
                "两个控制器，各管一片区域，再加一条交接规则：离目标远时灌能量，靠近时去平衡。",
            ),
            reality: Tr::new(
                "Gymnasts on the high bar pump the same way, and many real robots switch modes like this: walk, then stand; cruise, then dock.",
                "单杠上的体操运动员也是这样蓄力的，很多真实的机器人也这样切换模式：先走再站，先巡航再停靠。",
            ),
            myth: Tr::new(
                "A controller that is good at the top is not good everywhere. Linear feedback only works close to where it was designed.",
                "在顶端好用的控制器，不是哪里都好用。线性反馈只在设计点附近有效。",
            ),
            simplified: Tr::new(
                "Here the switch is one hard threshold. Real systems blend the hand-over and guard against flicking back and forth between modes.",
                "这里的切换只是一道硬阈值。真实系统会平滑地交接，并防止在两种模式之间来回跳。",
            ),
        },
        ..stub("swing-up", 5, 4)
    }
}

/// The first swing whose peak is taller than the swing before it (the one
/// the run starts in is skipped).
fn divergent_peak(s: &[crate::grade::Sample]) -> Option<f32> {
    let mut side = s.first()?.err > 0.0;
    let mut first = true;
    let mut last: Option<f32> = None;
    let (mut peak, mut at) = (0.0, 0.0);
    for x in s {
        if (x.err > 0.0) != side {
            if !first {
                if last.is_some_and(|l| peak > l) {
                    return Some(at);
                }
                last = Some(peak);
            }
            first = false;
            side = !side;
            peak = 0.0;
        }
        if x.err.abs() > peak {
            (peak, at) = (x.err.abs(), x.t);
        }
    }
    None
}

fn shivers() -> Task {
    Task {
        ops: CONTROL,
        sensors: &[Sensor::Pos, Sensor::Vel, Sensor::Angle, Sensor::Gyro],
        duration: 10.0,
        pass: Pass::Survive,
        cases: || {
            let r = |noise| Real {
                noise,
                lag: 1,
                ..Real::default()
            };
            vec![
                pole_at(91, 0.05, 0.0, r(0.01), &[(3.0, 0.5)]),
                pole_at(92, -0.1, 0.3, r(0.015), &[(5.0, -0.6)]),
                pole_at(93, 0.08, -0.3, r(0.02), &[(2.0, 0.4), (6.0, -0.4)]),
            ]
        },
        goals: [
            Goal::new(Metric::Error, 0.03),
            Goal::new(Metric::Effort, 15.0),
        ],
        starter: "\
; Lina's 'faster' tune | 莉娜调的“快一点”
MUL r0, angle, 200
DERIV r1, angle
MUL r1, r1, 6
MUL r2, pos, 1
MUL r3, vel, 2
ADD r0, r0, r1
ADD r2, r2, r3
ADD motor, r0, r2
",
        reference: "\
; a lean gain the delay can live with | 延迟承受得了的倾角增益
MUL r0, angle, 40
; fall rate straight from the gyro | 下落速度直接取自陀螺仪
MUL r1, gyro, 8
MUL r2, pos, 1
MUL r3, vel, 2
ADD r0, r0, r1
ADD r2, r2, r3
ADD motor, r0, r2
",
        scope: &[Src::Sense(Sensor::Angle), Src::Out(Act::Motor)],
        title: Tr::new("The Shivers", "打摆子"),
        brief: Tr::new(
            "Dr. Lina found POLE too slow, so she turned the lean gain up to 200 \"to make it react faster\". Now it shivers, worse and worse, and falls.\nOn the ward the sensors are noisy and every reading is one tick old.\nFind on the scope the first swing that is bigger than the one before it: from there the shivering feeds itself. Then fix it so POLE stands for 10 seconds.",
            "莉娜医生嫌 POLE 太慢，把倾角增益调到了 200，“好让它反应快一点”。现在它打起摆子，越抖越厉害，最后倒了。\n病房里的传感器有噪声，每个读数都晚一个 tick。\n在示波器上找到第一个比前一个更大的摆动：从那里起，抖动开始自己喂养自己。然后修好它，让 POLE 立满 10 秒。",
        ),
        predict: Predict {
            question: Tr::new(
                "Every reading arrives one tick late. What does that do to a very high gain?",
                "每个读数都晚到一个 tick。这对一个很高的增益意味着什么？",
            ),
            options: [
                Tr::new(
                    "Nothing much: one tick is only 20 ms.",
                    "没什么影响：一个 tick 才 20 毫秒。",
                ),
                Tr::new(
                    "The cart overcorrects on old news, and each swing comes back bigger.",
                    "小车按过时的消息过度纠正，每次摆回来都更大。",
                ),
                Tr::new(
                    "The robot reacts later, but more steadily.",
                    "机器人反应晚一点，但更稳。",
                ),
            ],
            answer: 1,
            why: Tr::new(
                "A delay turns a quick correction into a late one. With a huge gain, each late push overshoots, and the next one has more to undo: the swings grow until the pole falls.",
                "延迟把及时的纠正变成了迟到的纠正。增益太大时，每次迟到的推都会推过头，下一次要纠正的更多：摆动越来越大，直到杆倒下。",
            ),
        },
        hints: [
            Tr::new(
                "Find the first swing bigger than the one before. The angle gain is so high that every late correction overshoots.",
                "找到第一个比前一个更大的摆动。倾角增益太高，每次迟到的纠正都会过头。",
            ),
            Tr::new(
                "More `DERIV` only amplifies the noise. Bring the angle gain down to about 40, and take the fall rate from `gyro` instead of differentiating a noisy angle.",
                "再加 `DERIV` 只会放大噪声。把倾角增益降到 40 左右，下落速度直接用 `gyro`，别对带噪声的角度求导。",
            ),
            Tr::new(
                "40·angle + 8·gyro + 1·pos + 2·vel → motor.",
                "40·angle + 8·gyro + 1·pos + 2·vel → motor。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Gain margin", "增益裕度"),
            line: Tr::new(
                "Delay and noise put a ceiling on how hard a loop can push; past it, more gain means shaking, not precision.",
                "延迟和噪声给回路能推多狠设了上限；超过它，再加增益换来的不是精度，而是抖动。",
            ),
            reality: Tr::new(
                "Engineers check how much the gain could grow, or the delay lengthen, before a loop starts to ring, and keep that margin to spare.",
                "工程师会检查增益还能再大多少、延迟还能再长多少，回路才会开始振荡，并把这段余量留着。",
            ),
            myth: Tr::new(
                "When a controller shakes, the reflex is to add damping. With noisy, late sensors a bigger D just amplifies the noise.",
                "控制器一抖，第一反应是加阻尼。可传感器又吵又晚时，加大 D 只会放大噪声。",
            ),
            simplified: Tr::new(
                "Here the delay is exactly one tick. Real delays jitter from tick to tick, which eats into the margin further.",
                "这里的延迟正好一个 tick。真实的延迟每次都在抖动，还会进一步吃掉余量。",
            ),
        },
        clue: Some(Clue {
            signal: Src::Sense(Sensor::Angle),
            find: divergent_peak,
            within: 0.15,
            what: Tr::new("the first divergent peak", "第一个发散的峰"),
        }),
        ..stub("shivers", 5, 5)
    }
}

/// The tray run: POLE starts at `x0` and is sent to each stop in turn, every
/// 3.5 s, while six shoves land on the way.
fn walk(seed: u64, real: Real, x0: f32, stops: &[f32], kicks: [f32; 6]) -> Case {
    let mut p = pole(0.0, x0);
    p.target = x0;
    let at = [2.0, 3.6, 5.8, 8.2, 10.0, 13.0];
    Case {
        real,
        pushes: at.into_iter().zip(kicks).collect(),
        moves: (0..stops.len())
            .map(|i| (1.0 + 3.5 * i as f32, stops[i]))
            .collect(),
        ..case(seed, Rig::Pole(p))
    }
}

fn the_tray() -> Task {
    Task {
        ops: CONTROL,
        sensors: &[
            Sensor::Pos,
            Sensor::Vel,
            Sensor::Angle,
            Sensor::Gyro,
            Sensor::Target,
        ],
        duration: 18.0,
        pass: Pass::Arrive,
        cases: || {
            let r = |noise, lag, mass| Real {
                noise,
                lag,
                mass,
                ..Real::default()
            };
            vec![
                walk(121, r(0.01, 1, 1.0), -1.8, &[-0.6, 0.6, 1.8], [0.5, -0.6, 0.4, -0.5, 0.6, -0.4]),
                walk(122, r(0.02, 1, 1.4), 1.8, &[0.9, 0.0, -0.9, -1.8], [-0.6, 0.5, -0.5, 0.6, -0.6, 0.5]),
                walk(123, r(0.015, 2, 0.7), -1.5, &[0.0, 1.5, 0.5], [0.6, -0.5, 0.5, -0.6, 0.5, -0.6]),
                walk(124, r(0.02, 2, 1.2), 0.0, &[1.6, -1.6, 0.8], [-0.5, 0.6, -0.6, 0.5, -0.5, 0.6]),
            ]
        },
        goals: [
            Goal::new(Metric::Error, 0.042),
            Goal::new(Metric::Effort, 38.0),
        ],
        starter: "\
; Lina's corridor: stay up and follow target from bed to bed | 莉娜的走廊：立住，跟着 target 一张床一张床地走
; six shoves on the way, four robots that differ | 路上六次推扰，四台各不相同的机器人
; end upright and parked at the last bed | 最后要直立着停在最后一张床边
",
        reference: "\
; distance to the bed, at most 0.6 m of it at a time | 离病床的距离，一次最多看 0.6 m
SUB r2, pos, target
CLAMP r2, r2, 0.6
; a stiff balance | 硬一点的平衡
MUL r0, angle, 60
MUL r1, gyro, 8
; walk towards the bed | 朝病床走
MUL r2, r2, 2
MUL r3, vel, 3
ADD r0, r0, r1
ADD r2, r2, r3
ADD motor, r0, r2
",
        scope: &[
            Src::Sense(Sensor::Angle),
            Src::Sense(Sensor::Pos),
            Src::Sense(Sensor::Target),
            Src::Out(Act::Motor),
        ],
        live: Some(Live::Push),
        title: Tr::new("The Tray", "药盘"),
        brief: Tr::new(
            "Dr. Lina's job: carry the medicine tray down the corridor, stop at each bed, and do not spill a drop. She does not like mistakes.\n`target` moves to the next bed every 3.5 s. Patients, visitors and a trolley will bump POLE six times on the way. Four robots, all a little different: noisy, late, light or heavy.\nWhen time runs out, POLE must be upright and parked at the last bed.",
            "莉娜医生的委托：端着药盘走过走廊，在每张病床前停下，一滴都不能洒。她不喜欢出错。\n`target` 每 3.5 s 移到下一张床。路上病人、探视的人和一辆推车会撞 POLE 六次。四台机器人各有不同：有的吵，有的慢，有的轻，有的重。\n时间到时，POLE 必须直立着停在最后一张床边。",
        ),
        predict: Predict {
            question: Tr::new(
                "The next bed is 1.2 m away. What happens if the controller chases the whole distance at once?",
                "下一张床在 1.2 m 外。如果控制器一下子去追整段距离，会怎样？",
            ),
            options: [
                Tr::new("POLE gets there sooner.", "POLE 更快到达。"),
                Tr::new(
                    "POLE lunges with a big lean, and a shove on top of that can tip it or run it off the track.",
                    "POLE 带着大倾角猛冲，这时再挨一下推，就可能倒下或冲出轨道。",
                ),
                Tr::new(
                    "Nothing changes: the gains do not care how far it is.",
                    "没有区别：增益不在乎距离多远。",
                ),
            ],
            answer: 1,
            why: Tr::new(
                "A big position error asks for a big lean. Limit how much of the distance the controller sees at once, and POLE walks instead of lunging.",
                "位置误差大，要求的倾角就大。限制控制器一次看到的距离，POLE 就会走过去，而不是扑过去。",
            ),
        },
        hints: [
            Tr::new(
                "Start from the four gains, but measure position from the bed: use `pos − target` instead of `pos`.",
                "从四个增益出发，但位置要从病床量起：用 `pos − target` 代替 `pos`。",
            ),
            Tr::new(
                "Big jumps in `target` make POLE lunge. `CLAMP` the position error to about 0.6 m, and stiffen the gains for the heavy, late robots.",
                "`target` 一跳就让 POLE 猛冲。用 `CLAMP` 把位置误差限制在 0.6 m 左右，再为又重又慢的机器人把增益调硬一点。",
            ),
            Tr::new(
                "60·angle + 8·gyro + 2·clamp(pos − target, 0.6) + 3·vel → motor.",
                "60·angle + 8·gyro + 2·clamp(pos − target, 0.6) + 3·vel → motor。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Tracking under disturbance", "扰动下的跟踪"),
            line: Tr::new(
                "One loop doing three jobs at once: stay up, follow the goal, shrug off shoves. Each job borrows from the others.",
                "一个回路同时干三件事：立住、跟上目标、扛住推扰。每件事都在向另外两件借力。",
            ),
            reality: Tr::new(
                "Delivery robots in hospitals and hotels balance, follow a route and absorb bumps from people, all from feedback loops like this one.",
                "医院和酒店里的送物机器人要平衡、要走路线、还要扛住人的磕碰，靠的都是这样的反馈回路。",
            ),
            myth: Tr::new(
                "Chasing the goal harder does not get there sooner: for a balancing robot, a hard chase is exactly what tips it over.",
                "追得越狠并不会到得越快：对平衡机器人来说，猛追正是让它翻倒的原因。",
            ),
            simplified: Tr::new(
                "The corridor is a straight line and the shoves are short kicks. Real corridors have turns, slopes and people who lean on the tray.",
                "这里的走廊是一条直线，推扰只是短促的一下。真实的走廊有拐弯、有坡，还有人会靠在药盘上。",
            ),
        },
        ..stub("the-tray", 5, 6)
    }
}
