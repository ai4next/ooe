//! The block editor for PATCH: one row per instruction, its destination and
//! inputs as chips, a palette of the bench's blocks and a picker for the
//! selected slot.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_base::Lang;
use oo_kit::audio::Cue;
use oo_kit::ui::{ButtonSpec, Tone, Ui};
use ooe_core::doc::{Doc, note_text};
use ooe_core::lang::{Op, REGS, Src, num};

use crate::state::{Act, Sel, Session, t};
use crate::{Ctx, View};

/// Visible rows of the listing.
pub const ROWS: usize = 14;
const ROW_H: f32 = 22.0;
const TOP: f32 = 262.0;
const X0: f32 = -584.0;
const OP_W: f32 = 58.0;
const CHIP: f32 = 58.0;
/// Centre of the palette and picker column.
const PX: f32 = 40.0;

pub fn row_y(r: usize) -> f32 {
    TOP - r as f32 * ROW_H
}

/// A display row: a note above an instruction, or the instruction itself.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Row {
    Note(usize),
    Line(usize),
}

fn rows(doc: &Doc) -> Vec<Row> {
    let mut out = Vec::new();
    for i in 0..doc.code.len() {
        if doc.notes.get(i).is_some_and(Option::is_some) {
            out.push(Row::Note(i));
        }
        out.push(Row::Line(i));
    }
    out
}

pub fn doc(s: &Session) -> Option<Doc> {
    Doc::parse(s.work()).ok()
}

/// The selected slot's number, for typing digits.
pub fn number_at(s: &Session, line: usize, k: usize) -> Option<f32> {
    match doc(s)?.slot(line, k)? {
        Src::Num(v) => Some(v),
        _ => None,
    }
}

fn sel_line(s: &Session) -> Option<usize> {
    match s.sel {
        Sel::Line(i) | Sel::Slot(i, _) => Some(i),
        Sel::None => None,
    }
}

fn commit(cx: &mut Ctx, d: &Doc) {
    *cx.s.edit() = d.source();
}

/// Scrolls so instruction `line` is visible.
fn show_line(s: &mut Session, d: &Doc, line: usize) {
    let Some(r) = rows(d).iter().position(|r| *r == Row::Line(line)) else {
        return;
    };
    if r < s.scroll {
        s.scroll = r.saturating_sub(1);
    } else if r >= s.scroll + ROWS {
        s.scroll = r + 1 - ROWS;
    }
}

pub fn act(cx: &mut Ctx, a: Act) {
    let Some(mut d) = doc(&cx.s) else { return };
    let line = sel_line(&cx.s);
    match a {
        Act::AddOp(op) => {
            let after = line.or(d.code.len().checked_sub(1));
            let at = d.insert(after, op);
            commit(cx, &d);
            cx.s.sel = Sel::Line(at);
            show_line(&mut cx.s, &d, at);
            cx.cues.play(Cue::Place);
        }
        Act::SetOp(op) => {
            if let Some(i) = line
                && d.code.get(i).is_some_and(|ins| ins.op != op)
                && d.set_op(i, op)
            {
                commit(cx, &d);
                cx.s.sel = Sel::Line(i);
                cx.cues.play(Cue::Place);
            }
        }
        Act::SelLine(i) => {
            cx.s.sel = if cx.s.sel == Sel::Line(i) {
                Sel::None
            } else {
                Sel::Line(i)
            };
            cx.s.typing = false;
        }
        Act::SelSlot(i, k) => {
            let s = Sel::Slot(i, k);
            cx.s.sel = if cx.s.sel == s { Sel::Line(i) } else { s };
            cx.s.typing = false;
        }
        Act::SetSlot(v) => {
            if let Sel::Slot(i, k) = cx.s.sel
                && d.slot(i, k) != Some(v)
                && d.set_slot(i, k, v)
            {
                commit(cx, &d);
            }
            // Typed digits arrive as numbers; anything picked ends the entry.
            if !matches!(v, Src::Num(_)) {
                cx.s.typing = false;
            }
        }
        Act::Bump(delta) => set_number(cx, &mut d, |v| v + delta as f32 / 100.0),
        Act::Negate => set_number(cx, &mut d, |v| -v),
        Act::DeleteSel => {
            if let Some(i) = line.filter(|i| *i < d.code.len()) {
                d.remove(i);
                commit(cx, &d);
                cx.s.sel = Sel::None;
                let max = rows(&d).len().saturating_sub(ROWS);
                cx.s.scroll = cx.s.scroll.min(max);
            }
        }
        Act::MoveLine(up) => {
            if let Some(i) = line
                && let Some(j) = d.shift(i, up)
            {
                commit(cx, &d);
                cx.s.sel = match cx.s.sel {
                    Sel::Slot(_, k) => Sel::Slot(j, k),
                    _ => Sel::Line(j),
                };
                show_line(&mut cx.s, &d, j);
            }
        }
        Act::Scroll(dy) => {
            let max = rows(&d).len().saturating_sub(ROWS) as i32;
            cx.s.scroll = (cx.s.scroll as i32 + dy).clamp(0, max) as usize;
        }
        _ => {}
    }
}

/// Rewrites the selected number through `f`, rounded to hundredths.
fn set_number(cx: &mut Ctx, d: &mut Doc, f: impl Fn(f32) -> f32) {
    cx.s.typing = false;
    let Sel::Slot(i, k) = cx.s.sel else { return };
    let Some(Src::Num(v)) = d.slot(i, k) else {
        return;
    };
    let next = ((f(v) * 100.0).round() / 100.0).clamp(-999.0, 999.0);
    if next != v && d.set_slot(i, k, Src::Num(next)) {
        commit(cx, d);
    }
}

fn op_color(ui: &Ui, op: Op) -> Color {
    let pal = ui.pal;
    match op {
        Op::Mov | Op::Add | Op::Sub | Op::Mul | Op::Div => pal.primary,
        Op::Min | Op::Max | Op::Abs | Op::Clamp | Op::Sel => pal.tertiary,
        Op::Sqrt | Op::Sin | Op::Cos | Op::Atan2 | Op::Acos => pal.secondary,
        Op::Lpf | Op::Deriv | Op::Integ | Op::Pid => pal.warn,
    }
}

pub fn op_tip(op: Op, l: Lang) -> &'static str {
    match op {
        Op::Mov => t(
            l,
            "MOV d, a — copy a into d.",
            "MOV d, a —— 把 a 复制到 d。",
        ),
        Op::Add => t(l, "ADD d, a, b — d = a + b.", "ADD d, a, b —— d = a + b。"),
        Op::Sub => t(l, "SUB d, a, b — d = a − b.", "SUB d, a, b —— d = a − b。"),
        Op::Mul => t(l, "MUL d, a, b — d = a × b.", "MUL d, a, b —— d = a × b。"),
        Op::Div => t(
            l,
            "DIV d, a, b — d = a ÷ b (0 when b is 0).",
            "DIV d, a, b —— d = a ÷ b（b 为 0 时得 0）。",
        ),
        Op::Min => t(
            l,
            "MIN d, a, b — the smaller one.",
            "MIN d, a, b —— 取较小者。",
        ),
        Op::Max => t(
            l,
            "MAX d, a, b — the larger one.",
            "MAX d, a, b —— 取较大者。",
        ),
        Op::Abs => t(l, "ABS d, a — drop the sign.", "ABS d, a —— 去掉符号。"),
        Op::Clamp => t(
            l,
            "CLAMP d, x, lim — keep x between −lim and +lim.",
            "CLAMP d, x, lim —— 把 x 限制在 −lim 到 +lim 之间。",
        ),
        Op::Sel => t(
            l,
            "SEL d, c, a, b — a if c > 0, otherwise b.",
            "SEL d, c, a, b —— c > 0 时取 a，否则取 b。",
        ),
        Op::Sqrt => t(l, "SQRT d, a — square root.", "SQRT d, a —— 平方根。"),
        Op::Sin => t(
            l,
            "SIN d, a — sine (radians).",
            "SIN d, a —— 正弦（弧度）。",
        ),
        Op::Cos => t(
            l,
            "COS d, a — cosine (radians).",
            "COS d, a —— 余弦（弧度）。",
        ),
        Op::Atan2 => t(
            l,
            "ATAN2 d, y, x — the angle pointing at (x, y).",
            "ATAN2 d, y, x —— 指向 (x, y) 的角度。",
        ),
        Op::Acos => t(
            l,
            "ACOS d, a — the angle whose cosine is a.",
            "ACOS d, a —— 余弦为 a 的角度。",
        ),
        Op::Lpf => t(
            l,
            "LPF d, x, k — smooth x: move k of the way towards it each tick. Remembers.",
            "LPF d, x, k —— 平滑 x：每个周期向它靠近 k 的比例。有记忆。",
        ),
        Op::Deriv => t(
            l,
            "DERIV d, x — how fast x is changing, per second. Remembers.",
            "DERIV d, x —— x 每秒变化多快。有记忆。",
        ),
        Op::Integ => t(
            l,
            "INTEG d, x — x added up over time. Remembers.",
            "INTEG d, x —— x 随时间的累加。有记忆。",
        ),
        Op::Pid => t(
            l,
            "PID d, e, kp, ki, kd — kp·e + ki·∫e + kd·de/dt. Remembers.",
            "PID d, e, kp, ki, kd —— kp·e + ki·∫e + kd·de/dt。有记忆。",
        ),
    }
}

/// What slot `k` of an `op` row means (0 is the destination).
fn slot_name(op: Op, k: usize, l: Lang) -> &'static str {
    if k == 0 {
        return t(l, "WRITES TO", "写入");
    }
    match (op, k) {
        (Op::Sel, 1) => t(l, "IF THIS > 0", "如果它 > 0"),
        (Op::Sel, 2) => t(l, "THEN", "就取"),
        (Op::Sel, 3) => t(l, "ELSE", "否则取"),
        (Op::Clamp, 2) => t(l, "LIMIT", "上限"),
        (Op::Lpf, 2) => t(l, "SMOOTHING k (0–1)", "平滑系数 k（0–1）"),
        (Op::Atan2, 1) => t(l, "Y", "Y"),
        (Op::Atan2, 2) => t(l, "X", "X"),
        (Op::Pid, 1) => t(l, "ERROR", "误差"),
        (Op::Pid, 2) => t(l, "kp · PROPORTIONAL", "kp · 比例"),
        (Op::Pid, 3) => t(l, "ki · INTEGRAL", "ki · 积分"),
        (Op::Pid, 4) => t(l, "kd · DERIVATIVE", "kd · 微分"),
        _ => t(l, "INPUT", "输入"),
    }
}

/// Shows a row's current value while a run replays; set by `scope::sync_cursor`.
#[derive(Component)]
pub struct RowValue(pub usize);

/// Listing, palette and picker inside the editor card.
pub fn draw(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    let Some(d) = doc(s) else {
        ui.text(
            t(
                l,
                "This program cannot be shown as blocks.",
                "这段程序无法显示为积木。",
            ),
            Vec2::new(-600.0, 240.0),
            15.0,
            pal.bad,
            Anchor::CENTER_LEFT,
        );
        ui.button(ButtonSpec::new(
            Vec2::new(-500.0, 190.0),
            Vec2::new(200.0, 34.0),
            t(l, "Restore the starter", "恢复初始程序"),
            Act::Restore,
        ));
        return;
    };
    ui.caption(
        format!(
            "{}  {}",
            t(l, "PROGRAM · ROWS", "程序 · 行数"),
            d.code.len()
        ),
        Vec2::new(-612.0, 288.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let all = rows(&d);
    let selected = sel_line(s);
    if d.code.is_empty() {
        ui.text(
            t(
                l,
                "Empty. Pick a block on the right to start.",
                "空的。从右边选一个积木开始。",
            ),
            Vec2::new(-604.0, row_y(0)),
            13.0,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
    }
    for (r, row) in all.iter().enumerate().skip(s.scroll).take(ROWS) {
        let y = row_y(r - s.scroll);
        let i = match *row {
            Row::Note(i) => {
                let note = note_text(d.notes[i].as_deref().unwrap_or(""), l == Lang::Zh);
                ui.text(
                    format!("; {note}"),
                    Vec2::new(X0, y),
                    12.0,
                    pal.text_faint,
                    Anchor::CENTER_LEFT,
                );
                continue;
            }
            Row::Line(i) => i,
        };
        let ins = &d.code[i];
        if selected == Some(i) {
            ui.rect(
                Vec2::new(-385.0, y),
                Vec2::new(470.0, ROW_H - 2.0),
                pal.text.with_alpha(0.06),
                5.4,
            );
        }
        ui.text(
            format!("{:>2}", i + 1),
            Vec2::new(-604.0, y),
            11.0,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        let on = selected == Some(i) && s.sel == Sel::Line(i);
        ui.button(
            ButtonSpec::new(
                Vec2::new(X0 + OP_W / 2.0, y),
                Vec2::new(OP_W, ROW_H - 4.0),
                ins.op.name(),
                Act::SelLine(i),
            )
            .font(12.0)
            .selected(on)
            .tone(if on { Tone::Selected } else { Tone::Ghost })
            .tip(op_tip(ins.op, l)),
        );
        ui.rect(
            Vec2::new(X0 + 1.5, y),
            Vec2::new(3.0, ROW_H - 8.0),
            op_color(ui, ins.op),
            11.5,
        );
        let slots = std::iter::once(Src::from(ins.d)).chain(ins.srcs().iter().copied());
        for (k, src) in slots.enumerate() {
            // A gap after the destination reads as "d ← inputs".
            let gap = if k > 0 { 14.0 } else { 0.0 };
            let x = X0 + OP_W + 6.0 + CHIP / 2.0 + k as f32 * (CHIP + 4.0) + gap;
            let on = s.sel == Sel::Slot(i, k);
            ui.button(
                ButtonSpec::new(
                    Vec2::new(x, y),
                    Vec2::new(CHIP, ROW_H - 4.0),
                    src.to_string(),
                    Act::SelSlot(i, k),
                )
                .font(12.0)
                .tone(if on { Tone::Selected } else { Tone::Secondary }),
            );
            if k == 0 {
                ui.text(
                    "←",
                    Vec2::new(x + CHIP / 2.0 + 9.0, y),
                    11.0,
                    pal.text_faint,
                    Anchor::CENTER,
                );
            }
        }
        let e = ui.text(
            "",
            Vec2::new(-134.0, y),
            11.0,
            pal.secondary,
            Anchor::CENTER_RIGHT,
        );
        ui.commands.entity(e).insert(RowValue(i));
    }
    if all.len() > ROWS {
        let h = ROWS as f32 * ROW_H;
        let frac = ROWS as f32 / all.len() as f32;
        let top = TOP + ROW_H / 2.0;
        let off = s.scroll as f32 / all.len() as f32 * h;
        ui.rect(
            Vec2::new(-626.0, top - h / 2.0),
            Vec2::new(3.0, h),
            pal.line,
            6.0,
        );
        ui.rect(
            Vec2::new(-626.0, top - off - frac * h / 2.0),
            Vec2::new(3.0, frac * h),
            pal.primary,
            6.1,
        );
    }
    draw_palette(ui, task.ops, l);
    draw_side(ui, v, &d);
}

fn draw_palette(ui: &mut Ui, ops: &[Op], l: Lang) {
    let pal = ui.pal;
    ui.caption(
        t(l, "ADD A BLOCK", "添加积木"),
        Vec2::new(PX - 158.0, 288.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    // Five rows fit above the side panel; wider benches get a fifth column.
    let cols = if ops.len() > 16 { 5 } else { 4 };
    let pitch = 320.0 / cols as f32;
    let w = pitch - 4.0;
    for (n, &op) in ops.iter().enumerate() {
        let (c, r) = ((n % cols) as f32, (n / cols) as f32);
        let p = Vec2::new(PX - 160.0 + pitch / 2.0 + c * pitch, 262.0 - r * 27.0);
        ui.button(
            ButtonSpec::new(p, Vec2::new(w, 24.0), op.name(), Act::AddOp(op))
                .font(12.0)
                .tone(Tone::Ghost)
                .tip(op_tip(op, l)),
        );
        let color = op_color(ui, op);
        ui.rect(
            p + Vec2::new(3.0 - w / 2.0, 0.0),
            Vec2::new(3.0, 14.0),
            color,
            11.5,
        );
    }
}

const PICK_W: f32 = 74.0;

/// One choice in a grid of `cols` columns below `top`.
fn pick(ui: &mut Ui, n: usize, cols: usize, top: f32, label: String, act: Act, on: bool) {
    let (c, r) = ((n % cols) as f32, (n / cols) as f32);
    let w = (4.0 * (PICK_W + 4.0)) / cols as f32 - 4.0;
    let x = PX - 2.0 * (PICK_W + 4.0) + w / 2.0 + 2.0 + c * (w + 4.0);
    ui.button(
        ButtonSpec::new(Vec2::new(x, top - r * 24.0), Vec2::new(w, 21.0), label, act)
            .font(12.0)
            .selected(on),
    );
}

fn bumps(ui: &mut Ui, y: f32, value: f32, typing: bool) {
    let pal = ui.pal;
    for (n, (d, label)) in [(-10, "−0.1"), (-1, "−.01"), (1, "+.01"), (10, "+0.1")]
        .into_iter()
        .enumerate()
    {
        let x = PX + [-132.0, -82.0, 82.0, 132.0][n];
        ui.button(
            ButtonSpec::new(Vec2::new(x, y), Vec2::new(46.0, 21.0), label, Act::Bump(d)).font(11.0),
        );
    }
    for (n, (d, label)) in [(-100, "−1"), (100, "+1")].into_iter().enumerate() {
        let x = PX + [-132.0, 132.0][n];
        ui.button(
            ButtonSpec::new(
                Vec2::new(x, y - 25.0),
                Vec2::new(46.0, 21.0),
                label,
                Act::Bump(d),
            )
            .font(11.0),
        );
    }
    ui.button(
        ButtonSpec::new(
            Vec2::new(PX, y - 25.0),
            Vec2::new(60.0, 21.0),
            "±",
            Act::Negate,
        )
        .font(12.0),
    );
    let color = if typing { pal.primary } else { pal.secondary };
    ui.text(num(value), Vec2::new(PX, y), 15.0, color, Anchor::CENTER);
}

/// The picker for the selected slot, or line actions for a selected line.
fn draw_side(ui: &mut Ui, v: &View, d: &Doc) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    let top = 104.0;
    let head = Vec2::new(PX - 158.0, 128.0);
    let (line, k) = match s.sel {
        Sel::Slot(i, k) => (i, Some(k)),
        Sel::Line(i) => (i, None),
        Sel::None => {
            ui.caption(
                t(l, "EDIT", "编辑"),
                head,
                pal.text_faint,
                Anchor::CENTER_LEFT,
            );
            ui.wrapped(
                t(
                    l,
                    "Pick a block above to add it. Click a row to select it, a chip to change it.\nEvery tick the rows run top to bottom; registers r0–r7 keep their value until written.\nDigits type a number · Alt+↑/↓ moves a row · Delete removes it.",
                    "点上方的积木把它加进程序。点一行选中它，点一个格子修改它。\n每个周期从上到下执行每一行；寄存器 r0–r7 在被改写前保持原值。\n数字键输入数值 · Alt+↑/↓ 移动一行 · Delete 删除。",
                ),
                Vec2::new(PX - 158.0, top + 6.0),
                310.0,
                13.0,
                pal.text_dim,
                Anchor::TOP_LEFT,
            );
            return;
        }
    };
    let Some(ins) = d.code.get(line) else { return };
    let Some(k) = k else {
        ui.caption(
            format!("{} {} · {}", t(l, "ROW", "行"), line + 1, ins.op.name()),
            head,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        ui.wrapped(
            op_tip(ins.op, l),
            Vec2::new(PX - 158.0, top + 6.0),
            310.0,
            13.0,
            pal.text_dim,
            Anchor::TOP_LEFT,
        );
        let y = top - 44.0;
        let moves = [
            (t(l, "↑ Up", "↑ 上移"), Act::MoveLine(true), -112.0, 86.0),
            (t(l, "↓ Down", "↓ 下移"), Act::MoveLine(false), -18.0, 86.0),
            (t(l, "Delete", "删除"), Act::DeleteSel, 90.0, 110.0),
        ];
        for (label, act, dx, w) in moves {
            ui.button(
                ButtonSpec::new(Vec2::new(PX + dx, y), Vec2::new(w, 26.0), label, act).font(13.0),
            );
        }
        ui.caption(
            t(l, "CHANGE THE BLOCK", "换成别的积木"),
            Vec2::new(PX - 158.0, y - 24.0),
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        let cols = if task.ops.len() > 15 { 6 } else { 5 };
        for (n, &op) in task.ops.iter().enumerate() {
            pick(
                ui,
                n,
                cols,
                y - 42.0,
                op.name().into(),
                Act::SetOp(op),
                op == ins.op,
            );
        }
        return;
    };
    let Some(cur) = d.slot(line, k) else { return };
    ui.caption(
        format!(
            "{} {} · {}",
            t(l, "ROW", "行"),
            line + 1,
            slot_name(ins.op, k, l)
        ),
        head,
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let mut n = 0;
    let add = |ui: &mut Ui, n: &mut usize, src: Src| {
        pick(
            ui,
            *n,
            4,
            top,
            src.to_string(),
            Act::SetSlot(src),
            cur == src,
        );
        *n += 1;
    };
    for r in 0..REGS {
        add(ui, &mut n, Src::Reg(r));
    }
    // Outputs start on a fresh row, sensors after them.
    let outs: Vec<Src> = task.acts.iter().map(|&a| Src::Out(a)).collect();
    let sensors: Vec<Src> = task.sensors.iter().map(|&x| Src::Sense(x)).collect();
    for src in outs {
        add(ui, &mut n, src);
    }
    if k > 0 {
        n = n.div_ceil(4) * 4;
        for src in sensors {
            add(ui, &mut n, src);
        }
    }
    if k == 0 {
        ui.wrapped(
            t(
                l,
                "A register keeps a value for later rows. An output drives the robot.",
                "寄存器把值留给后面的行用；输出直接驱动机器人。",
            ),
            Vec2::new(PX - 158.0, top - n.div_ceil(4) as f32 * 24.0 - 4.0),
            310.0,
            12.0,
            pal.text_faint,
            Anchor::TOP_LEFT,
        );
        return;
    }
    let y = top - n.div_ceil(4) as f32 * 24.0 - 8.0;
    match cur {
        Src::Num(v) => bumps(ui, y, v, s.typing),
        _ => {
            ui.button(
                ButtonSpec::new(
                    Vec2::new(PX, y),
                    Vec2::new(PICK_W * 2.0, 21.0),
                    t(l, "0-9 a number", "0-9 一个数"),
                    Act::SetSlot(Src::Num(0.0)),
                )
                .font(12.0),
            );
        }
    }
}
