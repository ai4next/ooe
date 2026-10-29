//! A program with the comments attached to its rows, and the structured
//! edits the block editor makes.

use crate::lang::{self, Dst, Instr, Op, ParseError, Src};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Doc {
    pub code: Vec<Instr>,
    pub notes: Vec<Option<String>>,
}

/// Bilingual notes are written `english | 中文`; several comment lines that
/// belong to one row keep their halves apart when merged.
fn join_notes(parts: &[String]) -> String {
    if !parts.iter().any(|p| p.contains(" | ")) {
        return parts.join(" ");
    }
    let half = |zh: bool| {
        parts
            .iter()
            .map(|p| note_text(p, zh))
            .collect::<Vec<_>>()
            .join(" ")
    };
    format!("{} | {}", half(false), half(true))
}

/// The half of a note for the chosen language.
pub fn note_text(note: &str, zh: bool) -> &str {
    match note.split_once(" | ") {
        Some((en, cn)) => {
            if zh {
                cn
            } else {
                en
            }
        }
        None => note,
    }
}

impl Doc {
    pub fn parse(src: &str) -> Result<Self, ParseError> {
        let mut doc = Doc::default();
        let mut pending: Vec<String> = Vec::new();
        for (line, raw) in src.lines().enumerate() {
            let (body, note) = lang::split_comment(raw);
            let ins = lang::parse_line(body).map_err(|kind| ParseError { line, kind })?;
            if let Some(n) = note.filter(|n| !n.is_empty()) {
                pending.push(n.to_string());
            }
            if let Some(i) = ins {
                doc.code.push(i);
                doc.notes
                    .push((!pending.is_empty()).then(|| join_notes(&pending)));
                pending.clear();
            }
        }
        Ok(doc)
    }

    /// Source text: each note as a comment line above its row.
    pub fn source(&self) -> String {
        let mut out = String::new();
        for (i, ins) in self.code.iter().enumerate() {
            if let Some(Some(n)) = self.notes.get(i) {
                out.push_str(&format!("; {n}\n"));
            }
            out.push_str(&format!("{ins}\n"));
        }
        out
    }

    /// Inserts a fresh row after `after` (or at the top). Returns its index.
    pub fn insert(&mut self, after: Option<usize>, op: Op) -> usize {
        let at = after.map_or(0, |a| (a + 1).min(self.code.len()));
        self.code.insert(at, op.template());
        self.notes.resize(self.code.len() - 1, None);
        self.notes.insert(at, None);
        at
    }

    pub fn remove(&mut self, line: usize) -> bool {
        if line >= self.code.len() {
            return false;
        }
        self.code.remove(line);
        if line < self.notes.len() {
            self.notes.remove(line);
        }
        true
    }

    /// Swaps a row with its neighbour. Returns the row's new index.
    pub fn shift(&mut self, line: usize, up: bool) -> Option<usize> {
        let other = if up { line.checked_sub(1)? } else { line + 1 };
        if other >= self.code.len() || line >= self.code.len() {
            return None;
        }
        self.code.swap(line, other);
        self.notes.resize(self.code.len(), None);
        self.notes.swap(line, other);
        Some(other)
    }

    /// Changes a row's operation, keeping as many operands as still fit.
    pub fn set_op(&mut self, line: usize, op: Op) -> bool {
        let Some(ins) = self.code.get_mut(line) else {
            return false;
        };
        let old = *ins;
        *ins = op.template();
        ins.d = old.d;
        let keep = old.op.arity().min(op.arity());
        ins.args[..keep].copy_from_slice(&old.args[..keep]);
        true
    }

    /// Sets slot `slot` of a row: 0 is the destination, 1.. the sources.
    pub fn set_slot(&mut self, line: usize, slot: usize, v: Src) -> bool {
        let Some(ins) = self.code.get_mut(line) else {
            return false;
        };
        if slot == 0 {
            let d = match v {
                Src::Reg(r) => Dst::Reg(r),
                Src::Out(a) => Dst::Out(a),
                _ => return false,
            };
            ins.d = d;
            return true;
        }
        if slot > ins.op.arity() {
            return false;
        }
        ins.args[slot - 1] = v;
        true
    }

    /// The value in a slot (0 = destination).
    pub fn slot(&self, line: usize, slot: usize) -> Option<Src> {
        let ins = self.code.get(line)?;
        if slot == 0 {
            return Some(ins.d.into());
        }
        ins.srcs().get(slot - 1).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::{Act, Sensor};

    #[test]
    fn notes_stay_on_their_rows() {
        let src = "; error | 误差\nSUB r0, target, pos\n\nMUL motor, r0, 2 ; gain\n";
        let mut d = Doc::parse(src).unwrap();
        assert_eq!(d.code.len(), 2);
        assert_eq!(note_text(d.notes[0].as_deref().unwrap(), true), "误差");
        assert_eq!(d.notes[1].as_deref(), Some("gain"));
        assert_eq!(Doc::parse(&d.source()).unwrap(), d);
        assert_eq!(d.shift(0, false), Some(1));
        assert_eq!(d.notes[0].as_deref(), Some("gain"));
        assert_eq!(d.insert(Some(0), Op::Pid), 1);
        assert_eq!((d.code.len(), d.notes.len()), (3, 3));
        assert!(d.remove(1));
        assert!(!d.remove(9));
        assert_eq!(d.shift(0, true), None);
    }

    #[test]
    fn slots_check_their_kind() {
        let mut d = Doc::parse("ADD r0, pos, 1").unwrap();
        assert!(!d.set_slot(0, 0, Src::Sense(Sensor::Pos)));
        assert!(d.set_slot(0, 0, Src::Out(Act::Motor)));
        assert!(d.set_slot(0, 2, Src::Num(0.5)));
        assert!(!d.set_slot(0, 3, Src::Num(0.5)));
        assert_eq!(d.code[0].to_string(), "ADD motor, pos, 0.5");
        assert!(d.set_op(0, Op::Abs));
        assert_eq!(d.code[0].to_string(), "ABS motor, pos");
        assert!(d.set_op(0, Op::Pid));
        assert_eq!(d.code[0].to_string(), "PID motor, pos, 1, 0, 0");
        assert_eq!(d.slot(0, 1), Some(Src::Sense(Sensor::Pos)));
        assert_eq!(d.slot(0, 5), None);
    }
}
