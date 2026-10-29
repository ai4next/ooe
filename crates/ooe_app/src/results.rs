//! After a solve: the verdict, the insight card, and the Showcase where the
//! player's controller runs the robot live while the player interferes.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::audio::{Cue, CueQueue};
use oo_kit::ui::{ButtonSpec, ScreenEntity, Tone, Ui};
use ooe_core::content;
use ooe_core::grade::{self, Brain, Report};
use ooe_core::lang::DT;
use ooe_core::sim::{Rig, World};
use ooe_core::task::{Live, Task};

use crate::state::{Act, Screen, Session, t};
use crate::{Ctx, View, bench, hub, scope, stage};

pub fn act(cx: &mut Ctx, a: Act) {
    match a {
        Act::Next => {
            let save = &cx.game.save;
            let tasks = content::tasks();
            let open =
                |i: &usize| save.is_unlocked(&tasks[*i]) && !save.campaign.is_solved(tasks[*i].id);
            let after = (cx.s.task + 1..tasks.len()).find(open);
            match after.or_else(|| hub::next_task(save)) {
                Some(i) => {
                    bench::open(&mut cx.s, &cx.game, i);
                    cx.go(Screen::Brief);
                }
                None => cx.go(Screen::Hub),
            }
        }
        Act::Showcase if cx.s.task().live.is_some() => {
            start_showcase(&mut cx.s);
            cx.go(Screen::Showcase);
        }
        Act::Rewind => start_showcase(&mut cx.s),
        Act::Poke(dx, dy) => poke(&mut cx.s, dx as f32, dy as f32),
        _ => {}
    }
    cx.dirty();
}

/// The controller the showcase runs: the graded one (with its trained
/// table), or the work rebuilt.
fn fresh_brain(s: &Session) -> Option<Brain> {
    let task = s.task();
    match &s.report {
        Some(Report { code, table, .. }) => Some(Brain::new(code, table.clone())),
        None => {
            let code = grade::build(task, s.work.as_deref()?).ok()?;
            Some(grade::brain(task, &code))
        }
    }
}

/// The showcase's world: the first test case, without its scripted pushes,
/// with the player's moved target applied.
fn fresh_world(s: &Session) -> Option<World> {
    let case = s.cases.first()?;
    let mut w = World::new(case.rig.clone(), case.real, Vec::new(), case.seed);
    apply_shift(&mut w, &case.rig, s.live.shift);
    Some(w)
}

fn apply_shift(w: &mut World, base: &Rig, shift: [f32; 2]) {
    match (&mut w.rig, base) {
        (Rig::Cart(c), Rig::Cart(b)) => c.target = b.target + shift[0],
        (Rig::Arm(a), Rig::Arm(b)) => {
            a.nudge = [b.nudge[0] + shift[0], b.nudge[1] + shift[1]];
        }
        _ => {}
    }
}

/// Puts the robot back at the start of the first case with a fresh controller.
pub fn start_showcase(s: &mut Session) {
    if s.cases.is_empty() {
        s.cases = (s.task().cases)();
    }
    s.live.shift = [0.0; 2];
    s.live.pushes = 0;
    s.live.best = 0.0;
    reset_live(s);
}

fn reset_live(s: &mut Session) {
    s.live.world = fresh_world(s);
    s.live.brain = fresh_brain(s);
    if let Some(w) = &mut s.live.world {
        s.live.senses = w.senses();
    }
    s.live.outs = Default::default();
    s.live.acc = 0.0;
    s.live.trail.clear();
    s.live.down = 0.0;
    s.live.up = 0.0;
}

/// Whether the target moves up and down (the lift) rather than sideways.
fn vertical(task: &Task, s: &Session) -> bool {
    task.live == Some(Live::Target)
        && matches!(s.cases.first().map(|c| &c.rig), Some(Rig::Cart(c)) if c.gravity)
}

/// One arrow press: a push, or a step of the target.
fn poke(s: &mut Session, dx: f32, dy: f32) {
    let task = s.task();
    let Some(live) = task.live else { return };
    let Some(base) = s.cases.first().map(|c| c.rig.clone()) else {
        return;
    };
    let lift = vertical(task, s);
    let Some(w) = &mut s.live.world else { return };
    match (live, &base) {
        (Live::Push, _) => {
            if dx != 0.0 && w.fail.is_none() {
                w.pushes.push((w.t, dx * 0.6));
                s.live.pushes += 1;
            }
        }
        (Live::Target, Rig::Cart(c)) => {
            let step = if lift { dy } else { dx } * 0.25;
            let (lo, hi) = if c.gravity {
                (0.2, c.rail - 0.4)
            } else {
                let hi = c.wall.map_or(c.rail - 0.4, |wall| wall - CART_ROOM);
                (-c.rail + 0.4, hi)
            };
            let want = (c.target + s.live.shift[0] + step).clamp(lo, hi);
            s.live.shift[0] = want - c.target;
        }
        (Live::Nudge, Rig::Arm(_)) => {
            s.live.shift[0] = (s.live.shift[0] + dx * 0.1).clamp(-0.6, 0.6);
            s.live.shift[1] = (s.live.shift[1] + dy * 0.1).clamp(-0.6, 0.6);
        }
        _ => {}
    }
    apply_shift(w, &base, s.live.shift);
}

/// A target must stay this far short of a wall.
const CART_ROOM: f32 = 0.5;

/// Steps the live world at the control rate; a fallen robot is set back up.
pub fn tick_showcase(time: Res<Time>, mut s: ResMut<Session>, mut cues: ResMut<CueQueue>) {
    let dt = time.delta_secs().min(0.1);
    let s = &mut *s;
    if s.live.world.is_none() {
        return;
    }
    if s.live.world.as_ref().is_some_and(|w| w.fail.is_some()) {
        s.live.down -= dt;
        if s.live.down <= 0.0 {
            reset_live(s);
        }
        return;
    }
    s.live.acc += dt / DT;
    let n = (s.live.acc.floor() as usize).min(10);
    s.live.acc -= s.live.acc.floor();
    let (Some(w), Some(brain)) = (&mut s.live.world, &mut s.live.brain) else {
        return;
    };
    for _ in 0..n {
        let outs = brain.tick(&s.live.senses);
        w.step(outs);
        s.live.senses = w.senses();
        s.live.outs = outs;
        s.live.trail.push(w.error());
        s.live.up += DT;
        if w.fail.is_some() {
            s.live.down = 1.6;
            cues.play(Cue::Error);
            break;
        }
    }
    s.live.best = s.live.best.max(s.live.up);
    let keep = TRAIL;
    if s.live.trail.len() > keep {
        let cut = s.live.trail.len() - keep;
        s.live.trail.drain(..cut);
    }
}

/// Arrow keys interfere; R resets.
pub fn showcase_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: ResMut<oo_kit::ui::Actions<Act>>,
) {
    for (k, d) in [
        (KeyCode::ArrowLeft, (-1, 0)),
        (KeyCode::ArrowRight, (1, 0)),
        (KeyCode::ArrowUp, (0, 1)),
        (KeyCode::ArrowDown, (0, -1)),
    ] {
        if keys.just_pressed(k) {
            actions.push(Act::Poke(d.0, d.1));
        }
    }
    if keys.just_pressed(KeyCode::KeyR) {
        actions.push(Act::Rewind);
    }
}

pub fn draw_results(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    let rec = v
        .game
        .save
        .campaign
        .get(task.id)
        .cloned()
        .unwrap_or_default();
    // Left: what the run achieved.
    let left = Vec2::new(-330.0, 20.0);
    ui.card(left, Vec2::new(560.0, 540.0), pal.good);
    let x0 = left.x - 250.0;
    ui.caption(
        format!(
            "{} · {} · {}",
            task.code(),
            task.chapter().place.get(l),
            t(l, "ROBOT TUNED", "调校完成")
        ),
        Vec2::new(x0, 262.0),
        pal.good,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        task.title.get(l),
        Vec2::new(x0, 228.0),
        30.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    let stars = s.report.as_ref().map_or(0, |r| r.stars());
    ui.stars(Vec2::new(left.x, 168.0), stars, 22.0);
    let (msg, color) = if s.improved {
        (
            t(l, "A new best for this robot.", "这台机器人的新纪录。"),
            pal.secondary,
        )
    } else if rec.stars > stars {
        (
            t(
                l,
                "Your earlier controller earned more stars — that record stays.",
                "你之前的控制器星更多——纪录保留。",
            ),
            pal.text_dim,
        )
    } else {
        (t(l, "It works.", "它能用了。"), pal.text_dim)
    };
    ui.text(msg, Vec2::new(left.x, 126.0), 15.0, color, Anchor::CENTER);
    if rec.assisted {
        ui.text(
            t(
                l,
                "Solved with the walkthrough: at most ★★ here.",
                "看过完整解法：这里最多 ★★。",
            ),
            Vec2::new(left.x, 104.0),
            12.0,
            pal.text_faint,
            Anchor::CENTER,
        );
    }
    if let Some(rep) = &s.report {
        draw_rows(ui, v, rep, Vec2::new(left.x, 70.0), x0);
        draw_errors(ui, v, rep, Vec2::new(left.x, -132.0));
    }
    if !s.unlocked.is_empty() {
        let names: Vec<&str> = s
            .unlocked
            .iter()
            .map(|i| content::tasks()[*i].title.get(l))
            .collect();
        ui.text(
            format!("{}  {}", t(l, "NOW OPEN", "新开放"), names.join(" · ")),
            Vec2::new(x0, -222.0),
            13.0,
            pal.secondary,
            Anchor::CENTER_LEFT,
        );
    }
    draw_insight(ui, task, l);
    let y = -290.0;
    ui.button(
        ButtonSpec::new(
            Vec2::new(500.0, y),
            Vec2::new(220.0, 48.0),
            t(l, "Next robot ›", "下一台机器人 ›"),
            Act::Next,
        )
        .primary()
        .font(17.0),
    );
    if task.live.is_some() {
        ui.button(
            ButtonSpec::new(
                Vec2::new(272.0, y),
                Vec2::new(200.0, 48.0),
                t(l, "Showcase ▶", "展演 ▶"),
                Act::Showcase,
            )
            .tone(Tone::Secondary)
            .font(16.0)
            .tip(t(
                l,
                "Your controller drives the robot live — and you get to shove it.",
                "你的控制器实时驱动机器人——你可以去推它。",
            )),
        );
    }
    ui.button(
        ButtonSpec::new(
            Vec2::new(-470.0, y),
            Vec2::new(160.0, 40.0),
            t(l, "‹ Bench", "‹ 工作台"),
            Act::Goto(Screen::Bench),
        )
        .tone(Tone::Ghost)
        .tip(t(l, "Chase the stars you missed.", "去拿漏掉的星。")),
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(-300.0, y),
            Vec2::new(160.0, 40.0),
            t(l, "Workshop", "工坊"),
            Act::Back,
        )
        .tone(Tone::Ghost),
    );
}

/// The limit and star goals against this run's worst case.
fn draw_rows(ui: &mut Ui, v: &View, rep: &Report, at: Vec2, x0: f32) {
    let pal = ui.pal;
    let l = v.lang;
    let task = v.s.task();
    ui.caption(
        t(l, "THIS RUN · WORST TEST", "本次运行 · 最差测试"),
        Vec2::new(x0, at.y),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let rows = task
        .limit
        .into_iter()
        .map(|g| (t(l, "LIMIT", "上限").to_string(), g))
        .chain(
            task.goals
                .into_iter()
                .enumerate()
                .map(|(i, g)| (format!("★{}", i + 2), g)),
        );
    for (i, (label, g)) in rows.enumerate() {
        let y = at.y - 28.0 - i as f32 * 36.0;
        let got = rep.measure.get(g.metric);
        let met = rep.measure.meets(g);
        let accent = if met { pal.good } else { pal.warn };
        ui.chip(
            Vec2::new(at.x, y),
            Vec2::new(500.0, 30.0),
            pal.glass.with_alpha(0.8),
            8.0,
        );
        ui.dot(Vec2::new(x0 + 12.0, y), 8.0, accent, 9.0);
        ui.text(
            format!("{label}  {}", hub::goal_text(g, l)),
            Vec2::new(x0 + 26.0, y),
            14.0,
            pal.text,
            Anchor::CENTER_LEFT,
        );
        ui.text(
            g.metric.show(got),
            Vec2::new(at.x + 236.0, y),
            17.0,
            accent,
            Anchor::CENTER_RIGHT,
        );
    }
}

/// Every test's error over time, overlaid: the shape of the solve.
fn draw_errors(ui: &mut Ui, v: &View, rep: &Report, at: Vec2) {
    let pal = ui.pal;
    let l = v.lang;
    let size = Vec2::new(492.0, 92.0);
    ui.well(at, size + Vec2::new(8.0, 8.0));
    ui.caption(
        t(l, "ERROR OVER TIME · EVERY TEST", "误差随时间 · 所有测试"),
        Vec2::new(at.x - size.x / 2.0, at.y + size.y / 2.0 + 14.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let duration = v.s.task().duration.max(DT);
    let peak = rep
        .runs
        .iter()
        .flat_map(|r| r.samples.iter().map(|x| x.err.abs()))
        .filter(|e| e.is_finite())
        .fold(1e-3f32, f32::max);
    let (x0, mid) = (at.x - size.x / 2.0 + 6.0, at.y);
    // Keep strokes off the well's rounded corners.
    let inner = size - Vec2::new(12.0, 12.0);
    ui.rect(
        at,
        Vec2::new(size.x, 1.0),
        pal.text_faint.with_alpha(0.4),
        5.6,
    );
    let colors = [pal.secondary, pal.primary, pal.tertiary, pal.warn];
    for (k, run) in rep.runs.iter().enumerate() {
        let n = run.samples.len();
        if n < 2 {
            continue;
        }
        let color = colors[k % colors.len()].with_alpha(0.85);
        let pt = |i: usize| {
            let x = &run.samples[i];
            Vec2::new(
                x0 + (x.t / duration).min(1.0) * inner.x,
                mid + (x.err / peak).clamp(-1.0, 1.0) * inner.y / 2.0,
            )
        };
        let step = (n / 160).max(1);
        let mut prev = pt(0);
        let mut i = step;
        while i < n + step {
            let p = pt(i.min(n - 1));
            ui.line(prev, p, 1.6, color, 6.0 + k as f32 * 0.05);
            prev = p;
            i += step;
        }
    }
    ui.text(
        format!("±{}", scope::short(peak)),
        Vec2::new(at.x + size.x / 2.0 - 4.0, at.y + size.y / 2.0 - 8.0),
        10.0,
        pal.text_faint,
        Anchor::CENTER_RIGHT,
    );
}

/// The card the solve earns: the idea, real robots, a myth, and what the
/// simulator leaves out.
fn draw_insight(ui: &mut Ui, task: &Task, l: oo_base::Lang) {
    let pal = ui.pal;
    let i = &task.insight;
    let at = Vec2::new(300.0, 20.0);
    ui.card(at, Vec2::new(640.0, 540.0), pal.secondary);
    let x0 = at.x - 290.0;
    ui.caption(
        t(l, "INSIGHT CARD", "洞见卡"),
        Vec2::new(x0, 262.0),
        pal.secondary,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        i.title.get(l),
        Vec2::new(x0, 230.0),
        24.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        i.line.get(l),
        Vec2::new(x0, 204.0),
        580.0,
        16.0,
        pal.text,
        Anchor::TOP_LEFT,
    );
    let parts = [
        (
            t(l, "ON A REAL ROBOT", "真实机器人上"),
            i.reality,
            pal.primary,
        ),
        (t(l, "A COMMON MYTH", "常见误解"), i.myth, pal.warn),
        (
            t(l, "WHAT THE SIMULATOR SIMPLIFIES", "仿真简化了什么"),
            i.simplified,
            pal.text_faint,
        ),
    ];
    for (n, (head, body, color)) in parts.into_iter().enumerate() {
        let y = 110.0 - n as f32 * 112.0;
        ui.rect(
            Vec2::new(x0 + 1.5, y - 38.0),
            Vec2::new(3.0, 86.0),
            color,
            8.0,
        );
        ui.caption(head, Vec2::new(x0 + 14.0, y), color, Anchor::CENTER_LEFT);
        ui.wrapped(
            body.get(l),
            Vec2::new(x0 + 14.0, y - 14.0),
            566.0,
            14.0,
            pal.text_dim,
            Anchor::TOP_LEFT,
        );
    }
}

/// Where the live stage sits.
const SHOW: (Vec2, Vec2) = (Vec2::new(-170.0, 52.0), Vec2::new(860.0, 452.0));
/// The live error strip under the stage.
const STRIP: (Vec2, Vec2) = (Vec2::new(-170.0, -248.0), Vec2::new(860.0, 92.0));
/// Ticks of error kept for the strip (4 s).
const TRAIL: usize = 200;

/// Live readouts beside the stage.
#[derive(Component)]
pub struct ShowStat;

/// One segment of the live error strip.
#[derive(Component)]
pub struct TrailSeg(usize);

/// Stand-in shown over a fallen robot until it is set back up.
#[derive(Component)]
pub struct DownBanner;

pub fn draw_showcase(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    let (c, size) = SHOW;
    ui.card(c, size + Vec2::new(24.0, 24.0), pal.secondary);
    ui.well(c, size);
    stage::draw_stage(ui, v, c, size, true);
    let (sc, ss) = STRIP;
    ui.well(sc, ss);
    ui.rect(
        sc,
        Vec2::new(ss.x, 1.0),
        pal.text_faint.with_alpha(0.35),
        5.6,
    );
    ui.caption(
        t(l, "ERROR · LAST 4 SECONDS", "误差 · 最近 4 秒"),
        Vec2::new(sc.x - ss.x / 2.0, sc.y + ss.y / 2.0 + 12.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    for i in 0..TRAIL / 2 {
        ui.commands.spawn((
            Sprite::from_color(scope::PHOSPHOR, Vec2::new(1.0, 2.0)),
            Transform::from_xyz(sc.x, sc.y, 6.2),
            TrailSeg(i),
            ScreenEntity,
        ));
    }
    let e = ui.text("", c + Vec2::new(0.0, 40.0), 26.0, pal.bad, Anchor::CENTER);
    ui.commands.entity(e).insert(DownBanner);
    // Right column.
    let x0 = 290.0;
    ui.caption(
        t(l, "SHOWCASE · LIVE", "展演 · 实时"),
        Vec2::new(x0, 262.0),
        pal.secondary,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        task.title.get(l),
        Vec2::new(x0, 248.0),
        330.0,
        26.0,
        pal.text,
        Anchor::TOP_LEFT,
    );
    let lift = vertical(task, s);
    let body = match task.live {
        Some(Live::Push) => t(
            l,
            "Your controller is in charge now. Shove the robot with ← → and see whether it recovers. If it falls, it is set back up.",
            "现在由你的控制器接管。用 ← → 推机器人，看它能不能稳住。摔倒了会被重新扶起。",
        ),
        Some(Live::Target) if lift => t(
            l,
            "Move the target with ↑ ↓. Your program reads the new target every tick and drives there on its own.",
            "用 ↑ ↓ 移动目标。你的程序每个周期都读到新目标，自己开过去。",
        ),
        Some(Live::Target) => t(
            l,
            "Move the target with ← →. Your program reads the new target every tick and drives there on its own.",
            "用 ← → 移动目标。你的程序每个周期都读到新目标，自己开过去。",
        ),
        Some(Live::Nudge) => t(
            l,
            "Nudge the arm's target with the arrow keys. Your program sees the new target and follows it.",
            "用方向键微调手臂的目标。你的程序会看到新目标并跟上去。",
        ),
        None => "",
    };
    ui.wrapped(
        body,
        Vec2::new(x0, 176.0),
        330.0,
        14.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
    let e = ui.wrapped(
        "",
        Vec2::new(x0, 76.0),
        330.0,
        15.0,
        pal.secondary,
        Anchor::TOP_LEFT,
    );
    ui.commands.entity(e).insert(ShowStat);
    // Arrow buttons for mouse players.
    let pad = Vec2::new(x0 + 165.0, -78.0);
    let mut arrows = vec![
        ("◀", -1, 0, Vec2::new(-54.0, 0.0)),
        ("▶", 1, 0, Vec2::new(54.0, 0.0)),
    ];
    if lift {
        arrows = vec![
            ("▼", 0, -1, Vec2::new(-54.0, 0.0)),
            ("▲", 0, 1, Vec2::new(54.0, 0.0)),
        ];
    } else if task.live == Some(Live::Nudge) {
        arrows.push(("▲", 0, 1, Vec2::new(0.0, 40.0)));
        arrows.push(("▼", 0, -1, Vec2::new(0.0, -40.0)));
    }
    let tip = if task.live == Some(Live::Push) {
        t(l, "Push", "推")
    } else {
        t(l, "Move the target", "移动目标")
    };
    for (label, dx, dy, off) in arrows {
        ui.button(
            ButtonSpec::new(pad + off, Vec2::new(48.0, 36.0), label, Act::Poke(dx, dy))
                .font(16.0)
                .tip(tip),
        );
    }
    ui.button(
        ButtonSpec::new(
            pad + Vec2::new(0.0, -86.0),
            Vec2::new(150.0, 32.0),
            t(l, "Reset  R", "重置  R"),
            Act::Rewind,
        )
        .tone(Tone::Ghost)
        .font(13.0),
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(x0 + 165.0, -280.0),
            Vec2::new(240.0, 46.0),
            t(l, "‹ Back to results", "‹ 回到结果"),
            Act::Back,
        )
        .tone(Tone::Ghost)
        .font(15.0),
    );
}

/// Updates the readouts and the live error strip.
pub fn sync_showcase(
    s: Res<Session>,
    game: Res<crate::state::Game>,
    mut stat: Query<&mut Text2d, (With<ShowStat>, Without<DownBanner>)>,
    mut banner: Query<&mut Text2d, (With<DownBanner>, Without<ShowStat>)>,
    mut segs: Query<(&TrailSeg, &mut Transform, &mut Sprite)>,
) {
    let Some(w) = &s.live.world else { return };
    let l = game.lang();
    for mut text in &mut stat {
        let mut lines = vec![
            format!("{}  {:.1} s", t(l, "time", "时间"), w.t),
            format!("{}  {}", t(l, "error", "误差"), scope::short(w.error())),
        ];
        if s.task().live == Some(Live::Push) {
            lines.push(format!("{}  {}", t(l, "pushes", "推了"), s.live.pushes));
            lines.push(format!(
                "{}  {:.1} s",
                t(l, "longest standing", "最长站立"),
                s.live.best
            ));
        } else {
            let stop = if w.docked() { "✓" } else { "…" };
            lines.push(format!("{}  {stop}", t(l, "on target", "到位")));
        }
        let want = lines.join("\n");
        if text.0 != want {
            text.0 = want;
        }
    }
    for mut text in &mut banner {
        let want = if w.fail.is_some() {
            t(l, "Down — setting it back up…", "倒了——正在扶起……")
        } else {
            ""
        };
        if text.0 != want {
            text.0 = want.into();
        }
    }
    let trail = &s.live.trail;
    let (c, size) = STRIP;
    let peak = trail.iter().map(|e| e.abs()).fold(0.05f32, f32::max);
    // Screen slot `k` shows trail entry `k - off`: the newest is at the right.
    let off = TRAIL.saturating_sub(trail.len());
    let pt = |k: usize| {
        // Inset so the stroke's caps stay inside the well.
        let w = size.x - 16.0;
        let x = c.x - w / 2.0 + k as f32 / (TRAIL - 1) as f32 * w;
        let y = c.y + (trail[k - off] / peak).clamp(-1.0, 1.0) * (size.y / 2.0 - 4.0);
        Vec2::new(x, y)
    };
    for (seg, mut tf, mut sp) in &mut segs {
        let (a, b) = (seg.0 * 2, (seg.0 * 2 + 2).min(TRAIL - 1));
        if a < off || b <= a {
            sp.custom_size = Some(Vec2::ZERO);
            continue;
        }
        let (p, q) = (pt(a), pt(b));
        let d = q - p;
        tf.translation.x = (p.x + q.x) / 2.0;
        tf.translation.y = (p.y + q.y) / 2.0;
        tf.rotation = Quat::from_rotation_z(d.y.atan2(d.x));
        sp.custom_size = Some(Vec2::new(d.length() + 0.5, 2.0));
    }
}
