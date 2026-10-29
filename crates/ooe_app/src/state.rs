//! Screens, actions and the resources every screen reads.

use bevy::prelude::*;
use oo_base::{HintGate, Lang};
use oo_kit::save::SaveSlot;
use ooe_core::content;
use ooe_core::grade::Report;
use ooe_core::lang::{Op, Src};
use ooe_core::save::SaveData;
use ooe_core::task::{Case, Task};

#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Screen {
    #[default]
    Boot,
    Title,
    Intro,
    Hub,
    Brief,
    Bench,
    Results,
    Showcase,
    Settings,
}

/// Bottom-strip tabs on the bench.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Scope,
    Brief,
    Hints,
    /// Learning levels: the training curve.
    Learn,
}

/// Every button in the game pushes one of these.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Goto(Screen),
    Back,
    Quit,
    ToggleLang,
    // Title / settings
    NewGame,
    VolUp,
    VolDown,
    Mute,
    Fullscreen,
    Motion,
    Toasts,
    // Hub / brief
    /// Shows a chapter's levels on the hub.
    Chapter(u8),
    Open(usize),
    Predict(usize),
    StartBench,
    // Bench: run
    Run,
    Pause,
    Step,
    Rewind,
    Skip,
    Speed,
    Case(usize),
    Tab(Tab),
    Hint,
    Undo,
    Restore,
    /// Toggles a signal on the scope.
    Trace(Src),
    /// Fault levels: "this is the moment" at the playback cursor.
    Point,
    // Bench: editor
    AddOp(Op),
    SetOp(Op),
    SelLine(usize),
    SelSlot(usize, usize),
    SetSlot(Src),
    /// Adds to the selected number, in hundredths.
    Bump(i32),
    Negate,
    DeleteSel,
    MoveLine(bool),
    Scroll(i32),
    // Results
    Next,
    Showcase,
    /// Showcase: push the robot, or move its target, one step this way.
    Poke(i8, i8),
}

/// Persistent progress plus the autosave clock.
#[derive(Resource)]
pub struct Game {
    pub save: SaveData,
    pub slot: Option<SaveSlot>,
    pub dirty: bool,
    pub since_save: f32,
}

impl Game {
    pub fn lang(&self) -> Lang {
        self.save.settings.lang
    }

    pub fn touch(&mut self) {
        self.dirty = true;
    }
}

/// What is selected in the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sel {
    #[default]
    None,
    Line(usize),
    /// A row's slot: 0 is the destination, 1.. the inputs.
    Slot(usize, usize),
}

/// Playback speeds, in control ticks per second (50 is real time).
pub const SPEEDS: [f32; 4] = [50.0, 100.0, 200.0, 400.0];

/// Replay of one case's samples.
#[derive(Clone, Debug, Default)]
pub struct Play {
    /// Index of the sample on screen.
    pub step: usize,
    pub playing: bool,
    pub speed: usize,
    pub acc: f32,
    /// The verdict has been revealed for the current report.
    pub revealed: bool,
}

/// The live Showcase: the player's program driving the boss rig.
#[derive(Default)]
pub struct Live {
    pub world: Option<ooe_core::sim::World>,
    pub brain: Option<ooe_core::grade::Brain>,
    pub senses: ooe_core::ctl::Senses,
    /// The last tick's motor commands, for the stage's indicators.
    pub outs: ooe_core::ctl::Outs,
    pub acc: f32,
    /// Moved target (Target) or arm nudge (Nudge).
    pub shift: [f32; 2],
    pub pushes: u32,
    /// Recent error, for the live trace.
    pub trail: Vec<f32>,
    /// Seconds left before a fallen robot is set back up.
    pub down: f32,
    /// Seconds since the last reset, and the longest stretch so far.
    pub up: f32,
    pub best: f32,
}

/// One visit to a task: the work in progress and everything the bench shows.
#[derive(Resource, Default)]
pub struct Session {
    pub task: usize,
    /// The chapter the hub shows (None: follow KIT's next level).
    pub chapter: Option<u8>,
    pub work: Option<String>,
    pub undo: Vec<String>,
    pub hints: HintGate,
    pub sel: Sel,
    pub tab: Tab,
    pub report: Option<Report>,
    /// Build error shown instead of a report.
    pub error: Option<String>,
    pub case: usize,
    pub play: Play,
    pub pick: Option<usize>,
    pub scroll: usize,
    /// Signals on the scope.
    pub traces: Vec<Src>,
    /// Bumped whenever the report changes, so views know to repaint.
    pub generation: u64,
    /// The current report has been scored (once, when first revealed).
    pub judged: bool,
    /// Stars gained by the last solve (for the results screen).
    pub improved: bool,
    pub unlocked: Vec<usize>,
    pub settings_return: Screen,
    /// Page of the intro.
    pub page: usize,
    pub live: Live,
    /// The task's test cases (also inside a report, once there is one).
    pub cases: Vec<Case>,
    /// Typing a number into the selected slot replaces it on the first digit.
    pub typing: bool,
    /// The digits typed so far.
    pub entry: String,
}

impl Session {
    pub fn task(&self) -> &'static Task {
        &content::tasks()[self.task]
    }

    pub fn work(&self) -> &str {
        self.work.as_deref().unwrap_or("")
    }

    /// Records the current work for undo, then returns it for editing.
    pub fn edit(&mut self) -> &mut String {
        if let Some(w) = &self.work {
            self.undo.push(w.clone());
            if self.undo.len() > 64 {
                self.undo.remove(0);
            }
        }
        self.invalidate();
        self.work.get_or_insert_default()
    }

    /// Edits drop the last run: it no longer describes the work.
    pub fn invalidate(&mut self) {
        self.report = None;
        self.error = None;
        self.judged = false;
        self.generation += 1;
        self.play = Play {
            speed: self.play.speed,
            ..default()
        };
    }
}

/// Set to redraw the current screen from state at the end of the frame.
#[derive(Resource, Default)]
pub struct Redraw(pub bool);

/// Picks the language variant (ja / ko come from the `i18n/` catalogs).
pub use oo_base::t;
