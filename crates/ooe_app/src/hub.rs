//! The workshop hub (the island's eight chapters, each with its client)
//! and the briefing with its Predict question.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_base::Lang;
use oo_kit::ui::{ButtonSpec, Tone, Ui};
use oo_kit::ui::{TipArea, Tooltip};
use ooe_core::content;
use ooe_core::save::SaveData;
use ooe_core::task::{CHAPTERS, Chapter, Goal, Kind, Metric, Task};

use crate::state::{Act, Screen, t};
use crate::{Ctx, View, bench};

pub fn act(cx: &mut Ctx, a: Act) {
    match a {
        Act::Open(i) => {
            bench::open(&mut cx.s, &cx.game, i);
            cx.go(Screen::Brief);
        }
        Act::Predict(i) => {
            if cx.s.pick.is_none() {
                cx.s.pick = Some(i);
                let task = cx.s.task();
                SaveData::note_once(&mut cx.game.save.predicted, task.id);
                let cue = if i == task.predict.answer {
                    oo_kit::audio::Cue::Success
                } else {
                    oo_kit::audio::Cue::Error
                };
                cx.cues.play(cue);
                cx.game.touch();
                cx.dirty();
            }
        }
        Act::StartBench => cx.go(Screen::Bench),
        Act::Chapter(n) => {
            cx.s.chapter = Some(n);
            cx.dirty();
        }
        _ => {}
    }
}

pub fn kind_label(k: Kind, l: Lang) -> &'static str {
    ooe_core::task::kind_label(k).get(l)
}

/// `CHAPTER 3`, `第 3 章`, `3장`.
pub fn chapter_word(n: u8, l: Lang) -> String {
    match l {
        Lang::En => format!("CHAPTER {n}"),
        Lang::Zh | Lang::Ja => format!("第 {n} 章"),
        Lang::Ko => format!("{n}장"),
    }
}

/// `CHAPTER 3 · The School · 3-2 · PUZZLE`, for screen captions.
pub fn task_caption(task: &Task, l: Lang) -> String {
    format!(
        "{} · {} · {} · {}",
        chapter_word(task.chapter, l),
        task.chapter().place.get(l),
        task.code(),
        kind_label(task.kind(), l)
    )
}

fn hex(c: u32) -> Color {
    Color::srgb_u8((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// The chapter the hub shows: the one picked, else where KIT points next.
fn shown_chapter(v: &View) -> u8 {
    v.s.chapter
        .or_else(|| next_task(&v.game.save).map(|i| content::tasks()[i].chapter))
        .unwrap_or(1)
}

/// A chapter opens with its first level.
fn chapter_open(n: u8, save: &SaveData) -> bool {
    content::in_chapter(n)
        .next()
        .is_some_and(|t| save.is_unlocked(t))
}

pub fn goal_text(g: Goal, l: Lang) -> String {
    format!("≤ {} {}", g.metric.show(g.max), g.metric.label().get(l))
}

/// What a metric measures, for goal tooltips.
pub fn metric_tip(m: Metric, l: Lang) -> &'static str {
    match m {
        Metric::Time => t(
            l,
            "Seconds until it settles (or finishes), worst test case.",
            "直到稳定（或到达终点）的秒数，取最差测试用例。",
        ),
        Metric::Overshoot => t(
            l,
            "How far past the target it went, worst test case.",
            "冲过目标的距离，取最差测试用例。",
        ),
        Metric::Error => t(
            l,
            "RMS of the error after the first second, worst test case.",
            "第一秒之后误差的均方根，取最差测试用例。",
        ),
        Metric::Effort => t(
            l,
            "Total motor effort spent, worst test case.",
            "电机消耗的总能量，取最差测试用例。",
        ),
        Metric::Size => t(l, "Rows in the program.", "程序的行数。"),
    }
}

/// The first unlocked task without a solve: where KIT points next.
pub fn next_task(save: &SaveData) -> Option<usize> {
    content::tasks()
        .iter()
        .position(|t| save.is_unlocked(t) && !save.campaign.is_solved(t.id))
}

fn requirement_names(task: &Task, save: &SaveData, l: Lang) -> String {
    task.requires
        .iter()
        .filter(|r| !save.campaign.is_solved(r))
        .filter_map(|r| content::task(r))
        .map(|t| t.title.get(l))
        .collect::<Vec<_>>()
        .join(" · ")
}

pub fn draw_hub(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    let save = &v.game.save;
    ui.text_spaced(
        t(l, "KIT'S WORKSHOP", "KIT 的工坊"),
        Vec2::new(-600.0, 318.0),
        24.0,
        pal.text,
        Anchor::CENTER_LEFT,
        3.0,
    );
    let all = content::tasks().len() as u32;
    ui.pill(
        Vec2::new(236.0, 318.0),
        170.0,
        t(l, "TAUGHT", "已教会"),
        format!("{} / {}", save.campaign.solved_count(), all),
        pal.good,
    );
    ui.pill(
        Vec2::new(414.0, 318.0),
        150.0,
        t(l, "STARS", "星"),
        format!("{} / {}", save.campaign.total_stars(), all * 3),
        pal.secondary,
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(536.0, 318.0),
            Vec2::new(76.0, 34.0),
            l.switch_label(),
            Act::ToggleLang,
        )
        .font(14.0),
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(610.0, 318.0),
            Vec2::new(56.0, 34.0),
            "≡",
            Act::Goto(Screen::Settings),
        )
        .font(16.0)
        .tip(t(l, "Settings", "设置").to_string()),
    );
    let next = next_task(save);
    let tasks = content::tasks();
    let shown = shown_chapter(v);
    // Left rail: the island's eight places.
    for ch in &CHAPTERS {
        draw_chapter_tab(ui, ch, ch.n == shown, save, l);
    }
    // Right: the chapter's client and its levels.
    let ch = ooe_core::task::chapter(shown);
    let accent = hex(ch.client.color());
    let center = Vec2::new(160.0, -18.0);
    let size = Vec2::new(900.0, 560.0);
    ui.card(center, size, accent);
    let top = center.y + size.y / 2.0;
    let left = center.x - size.x / 2.0 + 28.0;
    ui.caption(
        format!("{} · {}", chapter_word(ch.n, l), ch.place.get(l)),
        Vec2::new(left, top - 30.0),
        accent,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        ch.theme.get(l),
        Vec2::new(left, top - 62.0),
        24.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    ui.dot(Vec2::new(left + 6.0, top - 100.0), 10.0, accent, 13.0);
    ui.text(
        format!("{} · {}", ch.client.name().get(l), ch.client.role().get(l)),
        Vec2::new(left + 22.0, top - 100.0),
        15.0,
        accent,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        ch.need.get(l),
        Vec2::new(left, top - 118.0),
        size.x - 56.0,
        14.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
    let row_w = size.x - 40.0;
    for slot in 1..=6u8 {
        let y = top - 196.0 - (slot - 1) as f32 * 58.0;
        let found = tasks
            .iter()
            .enumerate()
            .find(|(_, t)| t.chapter == ch.n && t.slot == slot);
        let kind = Kind::of_slot(slot - 1);
        let code = format!("{}-{}", ch.n, slot);
        let Some((i, task)) = found else {
            ui.well(Vec2::new(center.x, y), Vec2::new(row_w, 50.0));
            ui.text(
                code,
                Vec2::new(left + 6.0, y),
                14.0,
                pal.text_faint,
                Anchor::CENTER_LEFT,
            );
            ui.caption(
                format!(
                    "{} · {}",
                    kind_label(kind, l),
                    t(l, "COMING SOON", "尚未开放")
                ),
                Vec2::new(left + 66.0, y),
                pal.text_faint,
                Anchor::CENTER_LEFT,
            );
            continue;
        };
        let open = save.is_unlocked(task);
        let tone = if Some(i) == next {
            Tone::Primary
        } else {
            Tone::Secondary
        };
        let mut b = ButtonSpec::new(
            Vec2::new(center.x, y),
            Vec2::new(row_w, 50.0),
            "",
            Act::Open(i),
        )
        .tone(tone)
        .enabled(open);
        if !open {
            b = b.tip(format!(
                "{} {}",
                t(l, "Requires", "需要先完成"),
                requirement_names(task, save, l)
            ));
        }
        ui.button(b);
        let dim = if open { pal.text_dim } else { pal.text_faint };
        ui.text(
            code,
            Vec2::new(left + 6.0, y),
            14.0,
            dim,
            Anchor::CENTER_LEFT,
        );
        ui.text(
            task.title.get(l),
            Vec2::new(left + 66.0, y + 8.0),
            16.0,
            if open { pal.text } else { pal.text_faint },
            Anchor::CENTER_LEFT,
        );
        let sub = if open {
            kind_label(kind, l)
        } else {
            t(l, "LOCKED", "未开放")
        };
        let sub_color = match kind {
            _ if !open => pal.text_faint,
            Kind::Fault => pal.warn,
            Kind::Commission => accent,
            Kind::Puzzle => pal.text_faint,
        };
        ui.caption(
            sub,
            Vec2::new(left + 66.0, y - 12.0),
            sub_color,
            Anchor::CENTER_LEFT,
        );
        if open {
            ui.stars(
                Vec2::new(center.x + row_w / 2.0 - 52.0, y),
                save.campaign.stars(task.id),
                9.0,
            );
        }
    }
    let line = match next {
        Some(i) => format!(
            "KIT · {} {} “{}”",
            t(l, "Next on the bench:", "下一个："),
            tasks[i].code(),
            tasks[i].title.get(l)
        ),
        None => t(
            l,
            "KIT · Every robot can move now. Go back and chase the stars you missed.",
            "KIT · 每个机器人都学会动了。回去把漏掉的星补上吧。",
        )
        .to_string(),
    };
    ui.text(
        line,
        Vec2::new(-600.0, -330.0),
        14.0,
        pal.text_dim,
        Anchor::CENTER_LEFT,
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(560.0, -330.0),
            Vec2::new(120.0, 34.0),
            t(l, "Title", "标题"),
            Act::Back,
        )
        .tone(Tone::Ghost)
        .font(14.0),
    );
}

/// One chapter on the hub's left rail: number, place, client and progress.
fn draw_chapter_tab(ui: &mut Ui, ch: &Chapter, shown: bool, save: &SaveData, l: Lang) {
    let pal = ui.pal;
    let y = 250.0 - (ch.n - 1) as f32 * 68.0;
    let x = -470.0;
    let w = 300.0;
    let open = chapter_open(ch.n, save);
    let tasks: Vec<&Task> = content::in_chapter(ch.n).collect();
    let solved = tasks
        .iter()
        .filter(|t| save.campaign.is_solved(t.id))
        .count();
    let stars: u32 = tasks.iter().map(|t| save.campaign.stars(t.id) as u32).sum();
    let accent = hex(ch.client.color());
    let mut b = ButtonSpec::new(Vec2::new(x, y), Vec2::new(w, 60.0), "", Act::Chapter(ch.n))
        .tone(if shown {
            Tone::Selected
        } else {
            Tone::Secondary
        })
        .enabled(open);
    if !open {
        b = b.tip(t(l, "Finish the chapter before it first.", "先完成上一章。").to_string());
    }
    ui.button(b);
    let left = x - w / 2.0;
    ui.text(
        ch.n.to_string(),
        Vec2::new(left + 26.0, y + 2.0),
        24.0,
        if open { accent } else { pal.text_faint },
        Anchor::CENTER,
    );
    ui.text(
        ch.place.get(l),
        Vec2::new(left + 52.0, y + 10.0),
        16.0,
        if open { pal.text } else { pal.text_faint },
        Anchor::CENTER_LEFT,
    );
    ui.caption(
        ch.client.name().get(l),
        Vec2::new(left + 52.0, y - 11.0),
        if open { accent } else { pal.text_faint },
        Anchor::CENTER_LEFT,
    );
    if open {
        ui.caption(
            format!("★ {stars}"),
            Vec2::new(x + w / 2.0 - 16.0, y + 10.0),
            pal.secondary,
            Anchor::CENTER_RIGHT,
        );
        ui.progress(
            Vec2::new(x + w / 2.0 - 54.0, y - 11.0),
            Vec2::new(76.0, 4.0),
            solved as f32 / tasks.len().max(1) as f32,
            accent,
        );
    }
}

/// Goal rows: the hard limit (if any) then the two star goals.
pub fn draw_goals(ui: &mut Ui, task: &Task, at: Vec2, l: Lang, met: Option<[bool; 2]>) {
    let pal = ui.pal;
    let mut y = at.y;
    let mut row = |ui: &mut Ui, label: String, goal: Goal, color: Color, lit: bool| {
        let e = ui.text(
            format!("{label}  {}", goal_text(goal, l)),
            Vec2::new(at.x + 20.0, y),
            15.0,
            if lit { pal.text } else { pal.text_dim },
            Anchor::CENTER_LEFT,
        );
        ui.commands.entity(e).insert((
            Tooltip(metric_tip(goal.metric, l).into()),
            TipArea(Vec2::new(160.0, 10.0)),
        ));
        ui.dot(
            Vec2::new(at.x + 6.0, y),
            8.0,
            if lit { color } else { pal.text_faint },
            13.0,
        );
        y -= 26.0;
    };
    if let Some(g) = task.limit {
        row(ui, t(l, "Must run", "必须达到").into(), g, pal.warn, true);
    }
    for (i, g) in task.goals.into_iter().enumerate() {
        let lit = met.is_none_or(|m| m[i]);
        row(ui, format!("★{}", i + 2), g, pal.secondary, lit);
    }
}

pub fn draw_brief(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    let task = v.s.task();
    let save = &v.game.save;
    // Left: the robot's brief.
    let left = Vec2::new(-300.0, 10.0);
    ui.card(left, Vec2::new(600.0, 560.0), pal.primary);
    let x0 = left.x - 270.0;
    ui.caption(
        task_caption(task, l),
        Vec2::new(x0, 262.0),
        pal.primary,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        task.title.get(l),
        Vec2::new(x0, 228.0),
        30.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        task.brief.get(l),
        Vec2::new(x0, 190.0),
        540.0,
        16.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
    ui.caption(
        t(l, "TEST STANDARDS", "测试标准"),
        Vec2::new(x0, -110.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    draw_goals(ui, task, Vec2::new(x0, -140.0), l, None);
    let rec = save.campaign.get(task.id);
    if let Some(r) = rec.filter(|r| r.solved) {
        ui.stars(Vec2::new(left.x + 220.0, 228.0), r.stars, 11.0);
    }
    // Right: Predict.
    let right = Vec2::new(330.0, 10.0);
    ui.card(right, Vec2::new(560.0, 560.0), pal.secondary);
    let rx = right.x - 250.0;
    ui.caption(
        t(l, "PREDICT · BEFORE YOU BUILD", "预测 · 先想再做"),
        Vec2::new(rx, 262.0),
        pal.secondary,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        task.predict.question.get(l),
        Vec2::new(rx, 236.0),
        500.0,
        17.0,
        pal.text,
        Anchor::TOP_LEFT,
    );
    let answered = v.s.pick.or(save
        .predicted
        .iter()
        .any(|p| p == task.id)
        .then_some(usize::MAX));
    for (i, opt) in task.predict.options.iter().enumerate() {
        let y = 110.0 - i as f32 * 64.0;
        let mut b = ButtonSpec::new(
            Vec2::new(right.x, y),
            Vec2::new(500.0, 54.0),
            "",
            Act::Predict(i),
        )
        .enabled(answered.is_none());
        if answered.is_some() && i == task.predict.answer {
            b = b.tone(Tone::Selected);
        }
        ui.button(b);
        let mark = match answered {
            Some(_) if i == task.predict.answer => ("✓", pal.good),
            Some(p) if p == i => ("×", pal.bad),
            _ => (["A", "B", "C"][i], pal.text_faint),
        };
        ui.text(
            mark.0,
            Vec2::new(right.x - 228.0, y),
            16.0,
            mark.1,
            Anchor::CENTER,
        );
        let e = ui.wrapped(
            opt.get(l),
            Vec2::new(right.x - 206.0, y),
            440.0,
            15.0,
            pal.text,
            Anchor::CENTER_LEFT,
        );
        ui.commands
            .entity(e)
            .insert(Transform::from_xyz(right.x - 206.0, y, 11.0));
    }
    if let Some(p) = answered {
        let (head, color) = match p {
            usize::MAX => (
                t(l, "You predicted this before.", "你之前预测过这一题。"),
                pal.text_dim,
            ),
            p if p == task.predict.answer => (t(l, "Correct!", "答对了。"), pal.good),
            _ => (
                t(
                    l,
                    "Not quite — and that is what the bench is for.",
                    "不太对——这正是动手要验证的。",
                ),
                pal.warn,
            ),
        };
        ui.text(
            head,
            Vec2::new(rx, -100.0),
            16.0,
            color,
            Anchor::CENTER_LEFT,
        );
        ui.wrapped(
            task.predict.why.get(l),
            Vec2::new(rx, -122.0),
            500.0,
            14.0,
            pal.text_dim,
            Anchor::TOP_LEFT,
        );
    } else {
        ui.text(
            t(
                l,
                "Commit to an answer. You will see why when you run it.",
                "先选定一个答案，运行时你会看到原因。",
            ),
            Vec2::new(rx, -100.0),
            14.0,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
    }
    let start = if rec.is_some_and(|r| r.solved) {
        t(l, "Back to the bench", "回到工作台")
    } else {
        t(l, "To the bench", "去工作台")
    };
    ui.button(
        ButtonSpec::new(
            Vec2::new(right.x + 120.0, -232.0),
            Vec2::new(240.0, 50.0),
            start,
            Act::StartBench,
        )
        .primary()
        .enabled(answered.is_some())
        .font(17.0),
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(right.x - 160.0, -232.0),
            Vec2::new(150.0, 40.0),
            t(l, "Workshop", "工坊"),
            Act::Back,
        )
        .tone(Tone::Ghost),
    );
}
