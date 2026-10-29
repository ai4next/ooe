//! Title, intro and settings screens.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_base::Tr;
use oo_kit::ui::{ButtonSpec, Tone, Ui};
use ooe_core::content;

use crate::state::{Act, Screen, t};
use crate::{Ctx, View};

pub fn act(cx: &mut Ctx, a: Act) {
    let s = &mut cx.game.save.settings;
    match a {
        Act::NewGame => {
            let to = if cx.game.save.seen_intro {
                Screen::Hub
            } else {
                cx.s.page = 0;
                Screen::Intro
            };
            cx.go(to);
            return;
        }
        Act::Next => {
            cx.s.page += 1;
            if cx.s.page >= INTRO.len() {
                finish_intro(cx);
            } else {
                cx.dirty();
            }
            return;
        }
        Act::VolUp => s.volume_up(),
        Act::VolDown => s.volume_down(),
        Act::Mute => s.muted = !s.muted,
        Act::Fullscreen => s.fullscreen = !s.fullscreen,
        Act::Motion => s.reduce_motion = !s.reduce_motion,
        Act::Toasts => s.toasts = !s.toasts,
        _ => return,
    }
    cx.game.touch();
    cx.dirty();
}

pub fn finish_intro(cx: &mut Ctx) {
    cx.game.save.seen_intro = true;
    cx.game.touch();
    cx.go(Screen::Hub);
}

/// A two-link arm reaching for a dot: the title's emblem.
fn emblem(ui: &mut Ui, at: Vec2) {
    let pal = ui.pal;
    let base = at + Vec2::new(-70.0, -20.0);
    let elbow = base + Vec2::new(40.0, 62.0);
    let tip = elbow + Vec2::new(78.0, -12.0);
    let goal = tip + Vec2::new(26.0, -8.0);
    ui.rect(
        base + Vec2::new(0.0, -10.0),
        Vec2::new(64.0, 8.0),
        pal.line,
        6.0,
    );
    ui.line(base, elbow, 7.0, pal.primary, 6.2);
    ui.line(elbow, tip, 5.0, pal.primary, 6.2);
    for p in [base, elbow] {
        ui.dot(p, 16.0, pal.text, 6.4);
    }
    ui.dot(tip, 10.0, pal.primary_hot, 6.4);
    ui.glow(goal, 80.0, pal.secondary.with_alpha(0.22), 5.0);
    ui.dot(goal, 12.0, pal.secondary, 6.4);
    // A dashed reach line from tip to goal.
    for i in 0..3 {
        let a = tip.lerp(goal, 0.15 + i as f32 * 0.3);
        ui.dot(a, 3.0, pal.text_faint, 6.3);
    }
}

pub fn draw_title(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    emblem(ui, Vec2::new(0.0, 176.0));
    ui.text_spaced(
        "ORIGIN OF EMBODIMENT",
        Vec2::new(0.0, 96.0),
        54.0,
        pal.text,
        Anchor::CENTER,
        6.0,
    );
    ui.text(
        "具身起源",
        Vec2::new(0.0, 36.0),
        30.0,
        pal.primary,
        Anchor::CENTER,
    );
    ui.text(
        t(
            l,
            "A workshop full of robots that have never moved. Teach them.",
            "一间满是从没动过的机器人的工坊。教会它们。",
        ),
        Vec2::new(0.0, -8.0),
        16.0,
        pal.text_dim,
        Anchor::CENTER,
    );
    let started = v.game.save.seen_intro;
    let label = if started {
        t(l, "Continue", "继续")
    } else {
        t(l, "Open the workshop", "打开工坊")
    };
    ui.button(
        ButtonSpec::new(
            Vec2::new(0.0, -80.0),
            Vec2::new(280.0, 52.0),
            label,
            Act::NewGame,
        )
        .primary()
        .font(19.0),
    );
    let row = -146.0;
    ui.button(ButtonSpec::new(
        Vec2::new(-150.0, row),
        Vec2::new(136.0, 40.0),
        t(l, "Settings", "设置"),
        Act::Goto(Screen::Settings),
    ));
    ui.button(ButtonSpec::new(
        Vec2::new(0.0, row),
        Vec2::new(136.0, 40.0),
        l.switch_label(),
        Act::ToggleLang,
    ));
    ui.button(ButtonSpec::new(
        Vec2::new(150.0, row),
        Vec2::new(136.0, 40.0),
        t(l, "Quit", "退出"),
        Act::Quit,
    ));
    let c = &v.game.save.campaign;
    if c.solved_count() > 0 {
        let all = content::tasks().len();
        ui.text(
            format!(
                "{}  {} / {}   ·   {}  {} / {}",
                t(l, "Robots taught", "已教会的机器人"),
                c.solved_count(),
                all,
                t(l, "Stars", "星"),
                c.total_stars(),
                all * 3
            ),
            Vec2::new(0.0, -216.0),
            14.0,
            pal.text_faint,
            Anchor::CENTER,
        );
    }
    ui.caption(
        "SENSE · DECIDE · ACT · 50 TIMES A SECOND",
        Vec2::new(0.0, -320.0),
        pal.text_faint,
        Anchor::CENTER,
    );
}

/// KIT's welcome, one card per page.
const INTRO: [Tr; 4] = [
    Tr::new(
        "Power's on. Hello!\nI am KIT, the workshop assistant. Every robot on these benches has motors and sensors, and not one of them knows how to use them.",
        "通电了。你好！\n我是 KIT，工坊助手。这些工作台上的每个机器人都有电机和传感器，可没有一个知道该怎么用。",
    ),
    Tr::new(
        "A robot is a loop. Fifty times a second it reads its sensors, runs a tiny program, and sets its motors. Nothing more. Everything clever comes from what that program does with what it reads.",
        "机器人就是一个循环。每秒五十次：读传感器，跑一小段程序，设定电机。仅此而已。所有聪明之处，都来自这段程序如何对待它读到的东西。",
    ),
    Tr::new(
        "You write that program in PATCH, one row at a time. Before you build, I will ask you to predict what happens. Then run it and watch the scope: every signal, every tick.",
        "你用 PATCH 一行一行写这段程序。动手之前，我会先请你预测会发生什么。然后运行它，看示波器：每个信号，每个周期。",
    ),
    Tr::new(
        "First make it work on every test. Then make it smooth. The stars measure how close you came to how real engineers tune real machines.\nPIP is waiting on the first bench.",
        "先让它通过每个测试，再让它更平稳。星星衡量的是你离真实工程师调校真实机器的方式有多近。\nPIP 在第一张工作台上等你。",
    ),
];

pub fn draw_intro(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    let page = v.s.page.min(INTRO.len() - 1);
    ui.card(Vec2::new(0.0, 10.0), Vec2::new(720.0, 300.0), pal.primary);
    let head = Vec2::new(-300.0, 120.0);
    ui.chip(head, Vec2::new(24.0, 18.0), pal.primary, 8.0);
    ui.dot(head + Vec2::new(-5.0, 1.0), 4.0, pal.bg_deep, 8.2);
    ui.dot(head + Vec2::new(5.0, 1.0), 4.0, pal.bg_deep, 8.2);
    ui.glow(head, 90.0, pal.primary.with_alpha(0.3), 7.0);
    ui.caption(
        t(l, "KIT · WORKSHOP ASSISTANT", "KIT · 工坊助手"),
        Vec2::new(-276.0, 120.0),
        pal.primary,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        INTRO[page].get(l),
        Vec2::new(-310.0, 84.0),
        620.0,
        19.0,
        pal.text,
        Anchor::TOP_LEFT,
    );
    for i in 0..INTRO.len() {
        let c = if i == page {
            pal.primary
        } else {
            pal.text_faint
        };
        ui.dot(Vec2::new(-30.0 + i as f32 * 20.0, -110.0), 8.0, c, 8.0);
    }
    let last = page + 1 == INTRO.len();
    let label = if last {
        t(l, "Into the workshop", "进入工坊")
    } else {
        t(l, "Next", "继续")
    };
    ui.button(
        ButtonSpec::new(
            Vec2::new(0.0, -200.0),
            Vec2::new(220.0, 48.0),
            label,
            Act::Next,
        )
        .primary(),
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(0.0, -260.0),
            Vec2::new(120.0, 34.0),
            t(l, "Skip", "跳过"),
            Act::Back,
        )
        .tone(Tone::Ghost)
        .font(14.0),
    );
}
pub fn draw_settings(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    let s = &v.game.save.settings;
    ui.card(Vec2::new(0.0, 0.0), Vec2::new(560.0, 440.0), pal.secondary);
    ui.text(
        t(l, "Settings", "设置"),
        Vec2::new(-240.0, 180.0),
        26.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    let on = |b: bool| {
        if b {
            t(l, "On", "开")
        } else {
            t(l, "Off", "关")
        }
    };
    let rows: [(&str, String); 5] = [
        (t(l, "Volume", "音量"), format!("{} / 10", s.volume)),
        (t(l, "Mute", "静音"), on(s.muted).into()),
        (t(l, "Fullscreen", "全屏"), on(s.fullscreen).into()),
        (
            t(l, "Reduce motion", "减少动态"),
            on(s.reduce_motion).into(),
        ),
        (t(l, "Notifications", "通知"), on(s.toasts).into()),
    ];
    for (i, (label, value)) in rows.into_iter().enumerate() {
        let y = 110.0 - i as f32 * 56.0;
        ui.text(
            label,
            Vec2::new(-230.0, y),
            17.0,
            pal.text_dim,
            Anchor::CENTER_LEFT,
        );
        if i == 0 {
            ui.button(ButtonSpec::new(
                Vec2::new(70.0, y),
                Vec2::new(44.0, 36.0),
                "−",
                Act::VolDown,
            ));
            ui.text(value, Vec2::new(140.0, y), 17.0, pal.text, Anchor::CENTER);
            ui.button(ButtonSpec::new(
                Vec2::new(210.0, y),
                Vec2::new(44.0, 36.0),
                "+",
                Act::VolUp,
            ));
        } else {
            let act = [Act::Mute, Act::Fullscreen, Act::Motion, Act::Toasts][i - 1];
            let lit = [s.muted, s.fullscreen, s.reduce_motion, s.toasts][i - 1];
            ui.button(
                ButtonSpec::new(Vec2::new(160.0, y), Vec2::new(140.0, 36.0), value, act)
                    .selected(lit),
            );
        }
    }
    let y = 110.0 - 5.0 * 56.0;
    ui.text(
        t(l, "Language", "语言"),
        Vec2::new(-230.0, y),
        17.0,
        pal.text_dim,
        Anchor::CENTER_LEFT,
    );
    ui.button(ButtonSpec::new(
        Vec2::new(160.0, y),
        Vec2::new(140.0, 36.0),
        l.switch_label(),
        Act::ToggleLang,
    ));
    ui.button(
        ButtonSpec::new(
            Vec2::new(0.0, -270.0),
            Vec2::new(180.0, 44.0),
            t(l, "Done", "完成"),
            Act::Back,
        )
        .primary(),
    );
}
