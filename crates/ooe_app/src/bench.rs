//! The bench: edit, run, watch, and get judged.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_base::{HintTier, Lang};
use oo_kit::audio::Cue;
use oo_kit::ui::{Actions, ButtonSpec, Tone, Ui};
use ooe_core::content;
use ooe_core::grade::{BuildError, Report, Verdict, Why, grade};
use ooe_core::lang::{ParseErrorKind, Src};
use ooe_core::save::SaveData;
use ooe_core::sim::{Fail, Rig};
use ooe_core::task::Pass;

use crate::state::{Act, Game, Play, SPEEDS, Screen, Sel, Session, Tab, t};
use crate::{Ctx, View, editor, hub, scope, stage};

/// Starts a fresh visit to task `i` with the player's saved work.
pub fn open(s: &mut Session, game: &Game, i: usize) {
    let task = &content::tasks()[i];
    *s = Session {
        task: i,
        work: Some(game.save.work_for(task)),
        cases: (task.cases)(),
        traces: task.scope.to_vec(),
        chapter: Some(task.chapter),
        settings_return: s.settings_return,
        play: Play::default(),
        ..default()
    };
}

pub fn leave(cx: &mut Ctx) {
    save_work(cx);
    cx.go(Screen::Hub);
}

fn save_work(cx: &mut Ctx) {
    let id = cx.s.task().id;
    if let Some(w) = cx.s.work.clone() {
        cx.game.save.work.insert(id.to_string(), w);
        cx.game.touch();
    }
}

pub fn build_error_text(e: &BuildError, l: Lang) -> String {
    let locked = |name: &str| {
        format!(
            "{name} {}",
            t(
                l,
                "is not available on this bench.",
                "在这个工作台上不可用。"
            )
        )
    };
    match e {
        BuildError::Parse(p) => {
            let what = match &p.kind {
                ParseErrorKind::UnknownOp(s) => {
                    format!("{} “{s}”", t(l, "unknown instruction", "未知指令"))
                }
                ParseErrorKind::BadSource(s) => {
                    format!("{} “{s}”", t(l, "cannot read", "无法读取"))
                }
                ParseErrorKind::BadDest(s) => {
                    format!("{} “{s}”", t(l, "cannot write to", "无法写入"))
                }
                ParseErrorKind::Arity { want, got } => {
                    oo_base::Tr::new("needs {} input(s), has {}", "需要 {} 个输入，实际 {} 个")
                        .fill(l, &[want, got])
                }
            };
            format!("{} {}: {what}", t(l, "Line", "第"), p.line + 1)
        }
        BuildError::LockedOp(op) => locked(op.name()),
        BuildError::LockedSensor(s) => locked(s.name()),
        BuildError::LockedAct(a) => locked(a.name()),
    }
}

/// Builds and grades the work, then replays the first failing case (or the
/// selected one when everything passes).
pub fn run(cx: &mut Ctx) {
    let task = cx.s.task();
    let l = cx.lang();
    cx.s.invalidate();
    cx.s.typing = false;
    match grade(task, cx.s.work()) {
        Err(e) => {
            cx.s.error = Some(build_error_text(&e, l));
            cx.cues.play(Cue::Error);
        }
        Ok(rep) => {
            cx.game.save.campaign.note_attempt(task.id);
            cx.game.touch();
            cx.s.case = match rep.verdict {
                Verdict::Miss { case, .. } => case,
                _ => cx.s.case.min(rep.runs.len() - 1),
            };
            cx.s.report = Some(rep);
            restart(&mut cx.s, true);
        }
    }
    cx.dirty();
}

/// Rewinds the replay of the selected case.
pub fn restart(s: &mut Session, playing: bool) {
    if s.report.is_none() {
        return;
    }
    s.play.step = 0;
    s.play.acc = 0.0;
    s.play.playing = playing;
    s.generation += 1;
}

/// Samples recorded for the selected case.
pub fn sample_len(s: &Session) -> usize {
    s.report
        .as_ref()
        .and_then(|r| r.runs.get(s.case))
        .map_or(0, |r| r.samples.len())
}

/// Moves the replay to sample `to` (clamped to the recording).
pub fn advance(s: &mut Session, to: usize) {
    let n = sample_len(s);
    if n > 0 {
        s.play.step = to.min(n - 1);
    }
}

fn at_end(s: &Session) -> bool {
    s.play.step + 1 >= sample_len(s)
}

/// Shows the verdict; scores the report the first time.
pub fn reveal(cx: &mut Ctx) {
    advance(&mut cx.s, usize::MAX);
    cx.s.play.playing = false;
    cx.s.play.revealed = true;
    if !cx.s.judged {
        cx.s.judged = true;
        judge(cx);
    }
    cx.dirty();
}

fn hint_ready(s: &Session) -> bool {
    s.hints.next().is_some_and(|n| s.hints.is_available(n))
}

fn judge(cx: &mut Ctx) {
    let task = cx.s.task();
    let l = cx.lang();
    let Some(rep) = &cx.s.report else { return };
    let Some(outcome) = rep.outcome(task) else {
        let had = hint_ready(&cx.s);
        cx.s.hints.fail();
        if hint_ready(&cx.s) && !had {
            cx.toasts.push(
                t(l, "KIT has a hint", "KIT 有提示"),
                t(
                    l,
                    "Open the Hints tab when you want it.",
                    "需要时打开“提示”标签。",
                ),
            );
        }
        cx.cues.play(Cue::Error);
        return;
    };
    save_work(cx);
    if !cx.game.save.has_evidence(task)
        && let Some(clue) = task.clue
    {
        cx.toasts.push(
            t(l, "Fixed — now show KIT why", "修好了——再给 KIT 指出原因"),
            format!(
                "{} {}",
                t(
                    l,
                    "Restore the starter, run it, and point at:",
                    "恢复初始程序，运行，并在示波器上指出："
                ),
                clue.what.get(l)
            ),
        );
        return;
    }
    let save = &mut cx.game.save;
    let before: Vec<bool> = content::tasks()
        .iter()
        .map(|t| save.is_unlocked(t))
        .collect();
    let assisted = cx.s.hints.used_walkthrough();
    cx.s.improved = save.campaign.record_solve(task.id, outcome, assisted);
    SaveData::note_once(&mut save.insights, task.id);
    cx.s.unlocked = content::tasks()
        .iter()
        .enumerate()
        .filter(|(i, t)| !before[*i] && save.is_unlocked(t))
        .map(|(i, _)| i)
        .collect();
    for &i in &cx.s.unlocked {
        let name = content::tasks()[i].title.get(l);
        cx.toasts.push(t(l, "Robot unlocked", "新机器人开放"), name);
    }
    cx.cues.play(Cue::Success);
    if cx.s.improved {
        cx.cues.play(Cue::Star);
    }
    if !cx.s.unlocked.is_empty() {
        cx.cues.play(Cue::Unlock);
    }
    cx.game.touch();
}

/// Where a fault's evidence sits in the replayed run, if that run failed.
pub fn clue_at(s: &Session) -> Option<f32> {
    let clue = s.task().clue?;
    let run = s.report.as_ref()?.runs.get(s.case)?;
    if run.ok() {
        return None;
    }
    (clue.find)(&run.samples)
}

/// The player's "this is the moment" at the playback cursor.
fn point(cx: &mut Ctx) {
    let task = cx.s.task();
    let l = cx.lang();
    let Some(clue) = task.clue else { return };
    let now =
        cx.s.report
            .as_ref()
            .and_then(|r| r.runs.get(cx.s.case))
            .and_then(|r| r.samples.get(cx.s.play.step))
            .map(|x| x.t);
    let (Some(now), at) = (now, clue_at(&cx.s)) else {
        return;
    };
    if at.is_some_and(|at| (now - at).abs() <= clue.within) {
        SaveData::note_once(&mut cx.game.save.evidence, task.id);
        cx.game.touch();
        cx.toasts
            .push(t(l, "Evidence found", "找到证据了"), clue.what.get(l));
        cx.cues.play(Cue::Success);
    } else {
        cx.toasts.push(
            t(l, "Not that moment", "不是这一刻"),
            format!(
                "{} {}",
                t(l, "Drag the scope cursor to:", "把示波器光标拖到："),
                clue.what.get(l)
            ),
        );
        cx.cues.play(Cue::Error);
    }
}

pub fn act(cx: &mut Ctx, a: Act) {
    let l = cx.lang();
    let ran = cx.s.report.is_some();
    match a {
        Act::Run => run(cx),
        Act::Pause if ran => {
            if !cx.s.play.revealed {
                cx.s.play.playing = !cx.s.play.playing;
            } else {
                restart(&mut cx.s, true);
                cx.s.play.revealed = false;
            }
        }
        Act::Step if ran => {
            cx.s.play.playing = false;
            let n = cx.s.play.step + 1;
            advance(&mut cx.s, n);
            if at_end(&cx.s) {
                reveal(cx);
            }
        }
        Act::Rewind => {
            restart(&mut cx.s, false);
            cx.s.play.revealed = cx.s.judged;
        }
        Act::Skip if ran => reveal(cx),
        Act::Pause | Act::Step | Act::Skip => {}
        Act::Speed => cx.s.play.speed = (cx.s.play.speed + 1) % SPEEDS.len(),
        Act::Case(i) => {
            cx.s.case = i;
            if ran {
                restart(&mut cx.s, false);
                advance(&mut cx.s, usize::MAX);
            } else {
                cx.s.generation += 1;
            }
        }
        Act::Tab(tab) => cx.s.tab = tab,
        Act::Trace(src) => {
            if let Some(i) = cx.s.traces.iter().position(|s| *s == src) {
                cx.s.traces.remove(i);
            } else {
                if cx.s.traces.len() >= scope::MAX_TRACES {
                    cx.s.traces.remove(0);
                }
                cx.s.traces.push(src);
            }
        }
        Act::Point => point(cx),
        Act::Hint => {
            if let Some(tier) = cx.s.hints.reveal_next() {
                let id = cx.s.task().id;
                cx.game.save.campaign.note_hint(id, tier as u8);
                cx.game.touch();
                cx.s.tab = Tab::Hints;
                if tier == HintTier::Walkthrough {
                    cx.toasts.push(
                        t(l, "Walkthrough opened", "已打开完整解法"),
                        t(
                            l,
                            "This robot can now earn at most two stars.",
                            "这个机器人最多只能获得两颗星。",
                        ),
                    );
                }
            }
        }
        Act::Undo => {
            if let Some(w) = cx.s.undo.pop() {
                cx.s.work = Some(w);
                cx.s.invalidate();
                cx.s.sel = Sel::None;
            }
        }
        Act::Restore => {
            *cx.s.edit() = cx.s.task().starter.to_string();
            cx.s.sel = Sel::None;
            cx.s.scroll = 0;
        }
        Act::Next => {
            if cx.s.report.as_ref().is_some_and(Report::passed)
                && cx.game.save.has_evidence(cx.s.task())
            {
                cx.go(Screen::Results);
            }
        }
        a => editor::act(cx, a),
    }
    cx.dirty();
}

const DIGITS: [KeyCode; 10] = [
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

pub fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: ResMut<Actions<Act>>,
    mut cx: Ctx,
    scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
) {
    let ctrl = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    let number = selected_number(&cx.s);
    let mut push = |a| actions.push(a);
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::F5) {
        push(Act::Run);
    }
    if keys.just_pressed(KeyCode::Space) {
        push(Act::Pause);
    }
    if ctrl && keys.just_pressed(KeyCode::KeyZ) {
        push(Act::Undo);
    }
    // With a number selected, the period key types a decimal point.
    if keys.just_pressed(KeyCode::Period) && number.is_none() {
        push(Act::Step);
    }
    if keys.just_pressed(KeyCode::Delete) || (keys.just_pressed(KeyCode::Backspace) && !cx.s.typing)
    {
        push(Act::DeleteSel);
    }
    for (k, up) in [(KeyCode::ArrowUp, true), (KeyCode::ArrowDown, false)] {
        if keys.just_pressed(k) {
            push(if alt {
                Act::MoveLine(up)
            } else {
                Act::Scroll(if up { -1 } else { 1 })
            });
        }
    }
    let wheel = scroll.delta.y;
    if wheel.abs() > 0.5 {
        push(Act::Scroll(if wheel > 0.0 { -1 } else { 1 }));
    }
    let Some(cur) = number else {
        cx.s.typing = false;
        return;
    };
    // Typing replaces the selected number: digits, one point, minus, backspace.
    let mut entry = if cx.s.typing {
        cx.s.entry.clone()
    } else {
        String::new()
    };
    let before = entry.clone();
    for (d, k) in DIGITS.into_iter().enumerate() {
        if keys.just_pressed(k) && entry.len() < 7 {
            entry.push(char::from(b'0' + d as u8));
        }
    }
    if (keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::NumpadDecimal))
        && !entry.contains('.')
    {
        if entry.trim_start_matches('-').is_empty() {
            entry.push('0');
        }
        entry.push('.');
    }
    if keys.just_pressed(KeyCode::Minus) {
        if !cx.s.typing {
            push(Act::Negate);
            return;
        }
        entry = match entry.strip_prefix('-') {
            Some(rest) => rest.to_string(),
            None => format!("-{entry}"),
        };
    }
    if cx.s.typing && keys.just_pressed(KeyCode::Backspace) {
        entry.pop();
    }
    if entry == before {
        return;
    }
    let v: f32 = entry.trim_end_matches('.').parse().unwrap_or(0.0);
    cx.s.typing = true;
    cx.s.entry = entry;
    if (v - cur).abs() > 1e-6 {
        push(Act::SetSlot(Src::Num(v.clamp(-999.0, 999.0))));
    }
}

/// The value of the selected number slot, if any.
fn selected_number(s: &Session) -> Option<f32> {
    match s.sel {
        Sel::Slot(line, k) => editor::number_at(s, line, k),
        _ => None,
    }
}

pub fn tick(time: Res<Time>, mut cx: Ctx) {
    let dt = time.delta_secs();
    let before = hint_ready(&cx.s);
    let wait = cx.s.hints.wait_for_next().map(|w| w.ceil() as i32);
    cx.s.hints.tick(dt);
    let wait_now = cx.s.hints.wait_for_next().map(|w| w.ceil() as i32);
    if before != hint_ready(&cx.s) || (cx.s.tab == Tab::Hints && wait != wait_now) {
        cx.dirty();
    }
    if !cx.s.play.playing || cx.s.report.is_none() {
        return;
    }
    cx.s.play.acc += dt * SPEEDS[cx.s.play.speed];
    let n = cx.s.play.acc.floor() as usize;
    if n > 0 {
        cx.s.play.acc -= n as f32;
        let to = cx.s.play.step + n;
        advance(&mut cx.s, to);
    }
    if at_end(&cx.s) {
        reveal(&mut cx);
    }
}

pub const LEFT: (Vec2, Vec2) = (Vec2::new(-216.0, 120.0), Vec2::new(832.0, 372.0));
pub const RIGHT: (Vec2, Vec2) = (Vec2::new(421.0, 120.0), Vec2::new(422.0, 372.0));
pub const STRIP: (Vec2, Vec2) = (Vec2::new(0.0, -216.0), Vec2::new(1264.0, 264.0));
const TAB_Y: f32 = -106.0;

pub fn draw(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    draw_top(ui, v);
    ui.card(LEFT.0, LEFT.1, pal.primary);
    ui.card(RIGHT.0, RIGHT.1, pal.secondary);
    ui.card(STRIP.0, STRIP.1, pal.tertiary);
    editor::draw(ui, v);
    stage::draw_bench_stage(ui, v);
    draw_cases(ui, v);
    draw_transport(ui, v);
    let mut tabs = vec![
        (Tab::Scope, t(l, "Scope", "示波器")),
        (Tab::Brief, t(l, "Brief", "任务")),
        (Tab::Hints, t(l, "Hints", "提示")),
    ];
    if task.learn {
        tabs.push((Tab::Learn, t(l, "Training", "训练")));
    }
    for (i, (tab, label)) in tabs.into_iter().enumerate() {
        let p = Vec2::new(-560.0 + i as f32 * 118.0, TAB_Y);
        let mut b = ButtonSpec::new(p, Vec2::new(112.0, 28.0), label, Act::Tab(tab))
            .font(13.0)
            .selected(s.tab == tab);
        if tab != s.tab {
            b = b.tone(Tone::Ghost);
        }
        ui.button(b);
        if tab == Tab::Hints
            && s.hints
                .next()
                .is_some_and(|n| s.hints.is_available(n) && n != HintTier::Direction)
        {
            ui.dot(p + Vec2::new(48.0, 9.0), 8.0, pal.secondary, 12.0);
        }
    }
    draw_verdict(ui, v);
    let y = 288.0;
    ui.button(
        ButtonSpec::new(
            Vec2::new(-236.0, y),
            Vec2::new(52.0, 20.0),
            t(l, "Undo", "撤销"),
            Act::Undo,
        )
        .font(11.0)
        .tone(Tone::Ghost)
        .enabled(!s.undo.is_empty())
        .tip("Ctrl+Z"),
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(-180.0, y),
            Vec2::new(52.0, 20.0),
            t(l, "Reset", "还原"),
            Act::Restore,
        )
        .font(11.0)
        .tone(Tone::Ghost)
        .tip(t(
            l,
            "Put the starter program back. Undo brings yours back.",
            "恢复初始程序，可以撤销。",
        )),
    );
    match s.tab {
        Tab::Scope => scope::draw(ui, v),
        Tab::Brief => draw_brief_tab(ui, v),
        Tab::Hints => draw_hints(ui, v),
        Tab::Learn => draw_learn(ui, v),
    }
}

fn draw_brief_tab(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    ui.wrapped(
        task.brief.get(l),
        Vec2::new(-612.0, -134.0),
        700.0,
        15.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
    let met = s
        .report
        .as_ref()
        .filter(|r| r.passed())
        .map(|r| r.goals_met);
    ui.caption(
        t(l, "TEST STANDARDS", "测试标准"),
        Vec2::new(150.0, -140.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    hub::draw_goals(ui, task, Vec2::new(150.0, -168.0), l, met);
    ui.caption(
        t(l, "WHY IT HAPPENS", "背后的原因"),
        Vec2::new(420.0, -140.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        task.predict.why.get(l),
        Vec2::new(420.0, -158.0),
        196.0,
        12.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
}

fn draw_top(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    let y = 332.0;
    ui.button(
        ButtonSpec::new(
            Vec2::new(-586.0, y),
            Vec2::new(96.0, 28.0),
            t(l, "‹ Workshop", "‹ 工坊"),
            Act::Back,
        )
        .tone(Tone::Ghost)
        .font(13.0),
    );
    ui.caption(
        format!(
            "{} · {} · {}",
            task.code(),
            task.chapter().place.get(l),
            hub::kind_label(task.kind(), l)
        ),
        Vec2::new(-520.0, y + 7.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        task.title.get(l),
        Vec2::new(-520.0, y - 8.0),
        17.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    let measure = s
        .report
        .as_ref()
        .filter(|_| s.play.revealed)
        .map(|r| r.measure);
    let show =
        |g: ooe_core::task::Goal| measure.map_or("—".into(), |m| g.metric.show(m.get(g.metric)));
    for (i, g) in task.goals.into_iter().enumerate() {
        let met = measure.is_some_and(|m| m.meets(g));
        let accent = if met { pal.good } else { pal.text_dim };
        let x = 130.0 + i as f32 * 236.0;
        let label = format!("★{} {}", i + 2, hub::goal_text(g, l));
        ui.pill(Vec2::new(x, y), 228.0, &label, show(g), accent);
    }
    if let Some(g) = task.limit {
        let label = format!("{} {}", t(l, "LIMIT", "上限"), hub::goal_text(g, l));
        ui.pill(Vec2::new(-114.0, y), 228.0, &label, show(g), pal.warn);
    }
    ui.button(
        ButtonSpec::new(
            Vec2::new(520.0, y),
            Vec2::new(72.0, 28.0),
            l.switch_label(),
            Act::ToggleLang,
        )
        .font(13.0),
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(592.0, y),
            Vec2::new(52.0, 28.0),
            "≡",
            Act::Goto(Screen::Settings),
        )
        .font(15.0),
    );
}

/// Case buttons; after the verdict each shows whether it passed.
fn draw_cases(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let at = Vec2::new(232.0, -2.0);
    let n = s.report.as_ref().map_or(s.cases.len(), |r| r.runs.len());
    ui.caption(
        t(l, "TEST", "测试"),
        at,
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    for i in 0..n {
        let p = at + Vec2::new(58.0 + i as f32 * 34.0, 0.0);
        let on = s.case == i;
        let b = ButtonSpec::new(p, Vec2::new(30.0, 24.0), (i + 1).to_string(), Act::Case(i))
            .font(12.0)
            .selected(on)
            .tone(if on { Tone::Selected } else { Tone::Ghost });
        ui.button(b);
        if let Some(r) = s.report.as_ref().filter(|_| s.play.revealed) {
            let ok = r.runs.get(i).is_some_and(|c| c.ok());
            ui.dot(
                p + Vec2::new(11.0, 9.0),
                6.0,
                if ok { pal.good } else { pal.bad },
                12.0,
            );
        }
    }
    let e = ui.text(
        "",
        Vec2::new(612.0, -2.0),
        13.0,
        pal.text_dim,
        Anchor::CENTER_RIGHT,
    );
    ui.commands.entity(e).insert(stage::Clock);
}

/// Run and the playback controls.
fn draw_transport(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let s = v.s;
    let ran = s.report.is_some();
    let y = -38.0;
    ui.button(
        ButtonSpec::new(
            Vec2::new(282.0, y),
            Vec2::new(118.0, 34.0),
            t(l, "Run", "运行"),
            Act::Run,
        )
        .primary()
        .font(15.0)
        .tip(t(
            l,
            "Build and test every case. Enter / F5",
            "构建并测试所有用例。Enter / F5",
        )),
    );
    let play = if s.play.playing {
        t(l, "Pause", "暂停")
    } else if s.play.revealed {
        t(l, "Replay", "重放")
    } else {
        t(l, "Play", "播放")
    };
    let controls = [
        (play, Act::Pause, t(l, "Space", "空格")),
        (
            t(l, "Step", "单步"),
            Act::Step,
            t(
                l,
                "One control tick (20 ms). Period key",
                "前进一个控制周期（20 毫秒）。句号键",
            ),
        ),
        (
            t(l, "Rewind", "倒回"),
            Act::Rewind,
            t(l, "Back to the start", "回到开头"),
        ),
        (
            t(l, "Skip", "跳过"),
            Act::Skip,
            t(l, "Jump to the verdict", "直接看结果"),
        ),
    ];
    for (i, (label, act, tip)) in controls.into_iter().enumerate() {
        let p = Vec2::new(372.0 + i as f32 * 58.0, y);
        ui.button(
            ButtonSpec::new(p, Vec2::new(54.0, 30.0), label, act)
                .font(12.0)
                .enabled(ran)
                .tip(tip),
        );
    }
    let speed = format!("×{}", (SPEEDS[s.play.speed] / SPEEDS[0]) as i32);
    ui.button(
        ButtonSpec::new(
            Vec2::new(604.0, y),
            Vec2::new(46.0, 30.0),
            speed,
            Act::Speed,
        )
        .font(12.0)
        .tone(Tone::Ghost)
        .tip(t(
            l,
            "Playback speed (×1 is real time)",
            "回放速度（×1 为实时）",
        )),
    );
}

/// Why a case failed, in words.
pub fn why_text(why: Why, rig: &Rig, l: Lang) -> String {
    match why {
        Why::Failed(Fail::Crash) => match rig {
            Rig::Cart(c) if c.gravity => t(l, "hit the ceiling", "撞到了天花板"),
            _ => t(l, "hit the wall", "撞墙了"),
        }
        .into(),
        Why::Failed(Fail::OffTrack) => t(l, "ran off the end of the track", "冲出了轨道").into(),
        Why::Failed(Fail::Fell) => t(l, "fell over", "摔倒了").into(),
        Why::Failed(Fail::Lost) => t(l, "lost the line", "丢了线").into(),
        Why::Unsettled => t(
            l,
            "never came to rest on the target",
            "始终没有稳稳停在目标上",
        )
        .into(),
        Why::Late => t(l, "did not get there in time", "没能及时到达").into(),
        Why::Drift { rms, max } => format!(
            "{} {} > {}",
            t(l, "error too large:", "误差太大："),
            ooe_core::lang::num(rms),
            ooe_core::lang::num(max)
        ),
    }
}

/// What passing means for this task, in words.
fn pass_text(p: Pass, l: Lang) -> &'static str {
    match p {
        Pass::Settle { .. } => t(
            l,
            "Every test settles on target.",
            "每个测试都稳稳停在目标上。",
        ),
        Pass::Track { .. } => t(l, "Every test tracks closely.", "每个测试都跟得很紧。"),
        Pass::Survive => t(l, "It stays up in every test.", "每个测试都撑住了。"),
        Pass::Finish => t(l, "It finishes every test.", "每个测试都到达了终点。"),
        Pass::Arrive => t(
            l,
            "It arrives docked in every test.",
            "每个测试都稳稳到达终点。",
        ),
    }
}

/// One line on the tab row: build error, replaying, or the verdict.
fn draw_verdict(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let at = Vec2::new(-112.0, TAB_Y);
    if let Some(e) = &s.error {
        ui.dot(at, 9.0, pal.bad, 12.0);
        ui.text(
            format!("{}  {e}", t(l, "Does not build.", "无法构建。")),
            at + Vec2::new(14.0, 0.0),
            14.0,
            pal.bad,
            Anchor::CENTER_LEFT,
        );
        return;
    }
    let Some(rep) = &s.report else {
        ui.text(
            t(
                l,
                "Not run yet — press Run to test every case.",
                "还没有运行——按“运行”测试所有用例。",
            ),
            at,
            14.0,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        return;
    };
    if !s.play.revealed {
        let msg = format!(
            "{} {} / {}…",
            t(l, "Replaying test", "正在回放测试"),
            s.case + 1,
            rep.runs.len()
        );
        ui.text(msg, at, 14.0, pal.text_dim, Anchor::CENTER_LEFT);
        return;
    }
    let (msg, color) = match rep.verdict {
        Verdict::Pass => (format!("✓ {}", pass_text(s.task().pass, l)), pal.good),
        Verdict::Miss { case, why } => (
            format!(
                "× {} {} {}",
                t(l, "Test", "测试"),
                case + 1,
                why_text(why, &rep.runs[case].case.rig, l)
            ),
            pal.bad,
        ),
        Verdict::OverLimit { goal, got } => (
            format!(
                "△ {} {} · {} {}",
                t(l, "Works, but over the limit:", "能用，但超出上限："),
                goal.metric.show(got),
                t(l, "needs", "要求"),
                hub::goal_text(goal, l)
            ),
            pal.warn,
        ),
    };
    ui.text(msg, at, 14.0, color, Anchor::CENTER_LEFT);
    let task = s.task();
    if let Some(clue) = task.clue
        && !v.game.save.has_evidence(task)
        && clue_at(s).is_some()
    {
        // Fault levels: find the moment before the fix counts.
        ui.text(
            format!("{} {}", t(l, "Evidence:", "证据："), clue.what.get(l)),
            Vec2::new(476.0, TAB_Y),
            12.0,
            pal.warn,
            Anchor::CENTER_RIGHT,
        );
        ui.button(
            ButtonSpec::new(
                Vec2::new(548.0, TAB_Y),
                Vec2::new(128.0, 30.0),
                t(l, "This is the moment", "就是这里"),
                Act::Point,
            )
            .primary()
            .font(12.0),
        );
    }
    if rep.passed() && !v.game.save.has_evidence(task) {
        ui.text(
            t(l, "Point at the evidence first", "先指出证据"),
            Vec2::new(608.0, TAB_Y),
            13.0,
            pal.warn,
            Anchor::CENTER_RIGHT,
        );
    } else if rep.passed() {
        ui.stars(Vec2::new(430.0, TAB_Y), rep.stars(), 9.0);
        ui.button(
            ButtonSpec::new(
                Vec2::new(548.0, TAB_Y),
                Vec2::new(120.0, 30.0),
                t(l, "Results ›", "结果 ›"),
                Act::Next,
            )
            .primary()
            .font(14.0),
        );
    }
}

fn draw_hints(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    let names = [
        t(l, "WHERE TO LOOK", "往哪里看"),
        t(l, "WHICH MECHANISM", "用什么机制"),
        t(l, "WALKTHROUGH · CAPS AT ★★", "完整解法 · 最多 ★★"),
    ];
    for (i, tier) in HintTier::ALL.into_iter().enumerate() {
        let y = -140.0 - i as f32 * 66.0;
        let open = s.hints.revealed >= tier as u8;
        let color = if open { pal.secondary } else { pal.text_faint };
        ui.dot(Vec2::new(-604.0, y - 2.0), 8.0, color, 12.0);
        ui.caption(
            format!("{} · {}", i + 1, names[i]),
            Vec2::new(-592.0, y - 2.0),
            color,
            Anchor::CENTER_LEFT,
        );
        let body = Vec2::new(-592.0, y - 14.0);
        if open {
            ui.wrapped(
                task.hints[i].get(l),
                body,
                1180.0,
                14.0,
                pal.text,
                Anchor::TOP_LEFT,
            );
            continue;
        }
        if s.hints.next() != Some(tier) {
            continue;
        }
        if s.hints.is_available(tier) {
            let label = if tier == HintTier::Walkthrough {
                t(l, "Show the walkthrough", "显示完整解法")
            } else {
                t(l, "Show this hint", "显示这条提示")
            };
            ui.button(
                ButtonSpec::new(
                    body + Vec2::new(90.0, -12.0),
                    Vec2::new(180.0, 28.0),
                    label,
                    Act::Hint,
                )
                .font(13.0),
            );
        } else {
            let (runs, secs) = if tier == HintTier::Mechanism {
                (1, 45)
            } else {
                (3, 150)
            };
            let left = s.hints.wait_for_next().unwrap_or(0.0).ceil() as i32;
            let need = runs - s.hints.failures.min(runs);
            let msg = oo_base::Tr::new(
                "Opens after {} more failed run(s), or in {} s ({} runs / {} s).",
                "再失败 {} 次，或再过 {} 秒后开放（共 {} 次 / {} 秒）。",
            )
            .fill(l, &[&need, &left, &runs, &secs]);
            ui.text(
                msg,
                body + Vec2::new(0.0, -10.0),
                13.0,
                pal.text_faint,
                Anchor::CENTER_LEFT,
            );
        }
    }
}

/// Learning levels: how often training episodes ended docked.
fn draw_learn(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    ui.caption(
        t(
            l,
            "TRAINING · SHARE OF EPISODES THAT DOCKED (MOVING AVERAGE)",
            "训练 · 成功停靠的回合比例（滑动平均）",
        ),
        Vec2::new(-612.0, -136.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let at = Vec2::new(-300.0, -232.0);
    let size = Vec2::new(620.0, 150.0);
    ui.well(at, size + Vec2::new(16.0, 16.0));
    let Some(table) = s.report.as_ref().and_then(|r| r.table.as_ref()) else {
        ui.wrapped(
            t(
                l,
                "Write a reward and press Run. The robot then practises on its own, episode after episode, and drives with what it learned. This chart shows how often practice ended docked.",
                "写好奖励后按“运行”。机器人会一回合接一回合地自己练习，然后用学到的东西来开车。这张图显示练习中有多少回合成功停靠。",
            ),
            at + Vec2::new(-size.x / 2.0 + 10.0, 40.0),
            size.x - 20.0,
            14.0,
            pal.text_dim,
            Anchor::TOP_LEFT,
        );
        return;
    };
    let curve = &table.curve;
    let (x0, y0) = (at.x - size.x / 2.0, at.y - size.y / 2.0);
    for f in [0.0, 0.5, 1.0] {
        let y = y0 + f * size.y;
        ui.rect(Vec2::new(at.x, y), Vec2::new(size.x, 1.0), pal.line, 6.0);
        ui.caption(
            format!("{:.0}%", f * 100.0),
            Vec2::new(x0 - 8.0, y),
            pal.text_faint,
            Anchor::CENTER_RIGHT,
        );
    }
    const WINDOW: usize = 50;
    const POINTS: usize = 80;
    let n = curve.len();
    if n > 1 {
        let pt = |i: usize| {
            let end = (i * (n - 1) / (POINTS - 1)) + 1;
            let tail = &curve[end.saturating_sub(WINDOW)..end];
            let avg = tail.iter().sum::<f32>() / tail.len() as f32;
            Vec2::new(
                x0 + i as f32 / (POINTS - 1) as f32 * size.x,
                y0 + avg * size.y,
            )
        };
        let mut prev = pt(0);
        for i in 1..POINTS {
            let p = pt(i);
            ui.line(prev, p, 2.5, pal.secondary, 7.0);
            prev = p;
        }
    }
    let x = 60.0;
    let late = table.recent_success(200);
    let early = {
        let head = &curve[..curve.len().min(200)];
        if head.is_empty() {
            0.0
        } else {
            head.iter().sum::<f32>() / head.len() as f32
        }
    };
    let stats = [
        (t(l, "EPISODES", "回合"), n.to_string()),
        (
            t(l, "FIRST 200", "前 200 回合"),
            format!("{:.0}%", early * 100.0),
        ),
        (
            t(l, "LAST 200", "最后 200 回合"),
            format!("{:.0}%", late * 100.0),
        ),
    ];
    for (i, (label, value)) in stats.into_iter().enumerate() {
        ui.pill(
            Vec2::new(x + 90.0 + i as f32 * 186.0, -170.0),
            176.0,
            label,
            value,
            if i == 2 { pal.good } else { pal.primary },
        );
    }
    ui.wrapped(
        t(
            l,
            "The program you wrote is not driving: it only says what is good. The learner tries actions, keeps what your reward pays for, and the test runs use what it kept. If the curve stays low, the reward is not pointing it towards the dock.",
            "你写的程序并不直接开车：它只负责说什么是好的。学习器尝试各种动作，保留奖励鼓励的那些，测试时用的就是它保留下来的。如果曲线一直很低，说明奖励没有把它引向停靠点。",
        ),
        Vec2::new(x, -204.0),
        560.0,
        13.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
}
