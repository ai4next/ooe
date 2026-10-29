//! Origin of Embodiment — the game shell.
//!
//! Boot → Title → (Intro) → Hub → Brief → Bench → Results → (Showcase)
//!
//! Screens are drawn immediate-style: any state change sets [`Redraw`] and
//! the current screen is rebuilt from `Game` + `Session`. Only animated
//! parts (playback, the live showcase) update entities in place.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

mod bench;
mod editor;
mod hub;
mod results;
mod scope;
mod stage;
mod state;
mod title;

use bevy::asset::AssetPlugin;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use oo_base::{Lang, Settings};
use oo_kit::audio::{Ambience, CueQueue, MasterGain, build_sfx, spawn_ambience};
use oo_kit::save::SaveSlot;
use oo_kit::scene::{BackdropStyle, ReduceMotion, spawn_backdrop, spawn_camera, spawn_fade};
use oo_kit::tex::VisualTextures;
use oo_kit::theme::Palette;
use oo_kit::toast::{ToastQueue, ToastsEnabled};
use oo_kit::ui::{Actions, ScreenEntity, Ui, UiAssets};
use oo_kit::{KitPlugin, KitSet, add_actions};
use ooe_core::content;
use ooe_core::save::SaveData;

use crate::state::{Act, Game, Redraw, Screen, Session, Tab};

/// Game systems run between button clicks and presentation.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameSet;

/// The resources most action handlers touch.
#[derive(SystemParam)]
pub struct Ctx<'w> {
    pub game: ResMut<'w, Game>,
    pub s: ResMut<'w, Session>,
    pub next: ResMut<'w, NextState<Screen>>,
    pub screen: Res<'w, State<Screen>>,
    pub cues: ResMut<'w, CueQueue>,
    pub toasts: ResMut<'w, ToastQueue>,
    pub redraw: ResMut<'w, Redraw>,
}

impl Ctx<'_> {
    pub fn go(&mut self, screen: Screen) {
        self.next.set(screen);
    }

    pub fn lang(&self) -> Lang {
        self.game.lang()
    }

    pub fn dirty(&mut self) {
        self.redraw.0 = true;
    }
}

fn main() {
    ooe_core::i18n::install();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "具身起源 · Origin of Embodiment".into(),
                    resolution: (1280, 720).into(),
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: if cfg!(debug_assertions) {
                    concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").to_string()
                } else {
                    "assets".to_string()
                },
                ..default()
            }),
    )
    .insert_resource(Palette::embodiment())
    .add_plugins(KitPlugin)
    .init_state::<Screen>()
    .init_resource::<Session>()
    .init_resource::<Redraw>()
    .insert_resource(Game {
        save: SaveData::default(),
        slot: None,
        dirty: false,
        since_save: 0.0,
    })
    .configure_sets(
        Update,
        GameSet.after(KitSet::Buttons).before(KitSet::Present),
    )
    .add_systems(Startup, setup)
    .add_systems(OnEnter(Screen::Boot), boot)
    .add_systems(
        Update,
        (
            global_keys,
            handle_actions,
            bench::keys.run_if(in_state(Screen::Bench)),
            bench::tick.run_if(in_state(Screen::Bench)),
            results::showcase_input.run_if(in_state(Screen::Showcase)),
            results::tick_showcase.run_if(in_state(Screen::Showcase)),
            redraw,
            (
                stage::sync_stage,
                scope::sync_cursor,
                results::sync_showcase,
            ),
            apply_settings,
            autosave,
        )
            .chain()
            .in_set(GameSet),
    );
    add_actions::<Act>(&mut app);
    oo_kit::debug::install_screenshot(&mut app, "OOE");
    app.run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut sources: ResMut<Assets<AudioSource>>,
    pal: Res<Palette>,
) {
    let tex = VisualTextures::build(&mut images);
    spawn_camera(&mut commands, &pal);
    spawn_backdrop(&mut commands, &tex, &pal, BackdropStyle::Blueprint);
    commands.insert_resource(UiAssets::load(&asset_server, tex, Lang::En));
    // G2: the workshop hums lower than the museum.
    commands.insert_resource(build_sfx(&mut sources, 196.0));
    spawn_ambience(&mut commands, &mut sources, &Ambience::workshop());
}

fn env(key: &str) -> Option<String> {
    if cfg!(debug_assertions) {
        std::env::var(key).ok()
    } else {
        None
    }
}

/// Loads the save, then honours debug overrides:
/// `OOE_FRESH`, `OOE_LANG=zh`, `OOE_TASK=<id>`, `OOE_REF`,
/// `OOE_SCREEN=hub|intro|settings|brief|bench|results|showcase`, `OOE_RUN`,
/// `OOE_TAB=brief|hints|learn`, `OOE_SEL=<line>[,<slot>]`, `OOE_STEP=<tick>`,
/// `OOE_REVEAL` (judge first), `OOE_EVIDENCE` (fault evidence found).
fn boot(mut cx: Ctx) {
    let slot = SaveSlot::new("ooe", "OriginOfEmbodiment");
    if env("OOE_FRESH").is_none()
        && let Some(save) = slot.load(|s| SaveData::from_json(s).ok_or("not a save"))
    {
        cx.game.save = save;
    }
    cx.game.slot = env("OOE_FRESH").is_none().then_some(slot);
    if let Some(l) = env("OOE_LANG") {
        cx.game.save.settings.lang = Lang::from_code(&l);
    }
    let task = env("OOE_TASK").and_then(|id| content::tasks().iter().position(|t| t.id == id));
    if let Some(i) = task {
        bench::open(&mut cx.s, &cx.game, i);
        if env("OOE_REF").is_some() {
            cx.s.work = Some(cx.s.task().reference.to_string());
        }
    }
    let screen = match env("OOE_SCREEN").as_deref() {
        Some("hub") => Screen::Hub,
        Some("intro") => Screen::Intro,
        Some("settings") => Screen::Settings,
        Some("brief") if task.is_some() => Screen::Brief,
        Some("bench") if task.is_some() => Screen::Bench,
        Some("results") if task.is_some() => {
            bench::run(&mut cx);
            bench::reveal(&mut cx);
            Screen::Results
        }
        Some("showcase") if task.is_some() => {
            results::start_showcase(&mut cx.s);
            Screen::Showcase
        }
        _ => Screen::Title,
    };
    if screen == Screen::Bench && env("OOE_RUN").is_some() {
        if env("OOE_EVIDENCE").is_some() {
            let id = cx.s.task().id;
            SaveData::note_once(&mut cx.game.save.evidence, id);
        }
        bench::run(&mut cx);
        if env("OOE_REVEAL").is_some() {
            bench::reveal(&mut cx);
        }
        if let Some(step) = env("OOE_STEP").and_then(|v| v.parse().ok()) {
            cx.s.play.playing = false;
            bench::advance(&mut cx.s, step);
        }
    }
    if let Some(sel) = env("OOE_SEL") {
        let n: Vec<usize> = sel.split(',').filter_map(|p| p.parse().ok()).collect();
        cx.s.sel = match n.as_slice() {
            [l] => state::Sel::Line(*l),
            [l, k] => state::Sel::Slot(*l, *k),
            _ => state::Sel::None,
        };
    }
    cx.s.tab = match env("OOE_TAB").as_deref() {
        Some("brief") => Tab::Brief,
        Some("hints") => Tab::Hints,
        Some("learn") => Tab::Learn,
        _ => cx.s.tab,
    };
    cx.go(screen);
}

fn global_keys(keys: Res<ButtonInput<KeyCode>>, mut actions: ResMut<Actions<Act>>) {
    if keys.just_pressed(KeyCode::Escape) {
        actions.push(Act::Back);
    }
}

fn handle_actions(
    mut actions: ResMut<Actions<Act>>,
    mut cx: Ctx,
    mut exit: MessageWriter<AppExit>,
) {
    for a in std::mem::take(&mut actions.0) {
        let screen = *cx.screen.get();
        match a {
            Act::Goto(to) => {
                if to == Screen::Settings {
                    cx.s.settings_return = screen;
                }
                cx.go(to);
            }
            Act::Back => match screen {
                Screen::Hub => cx.go(Screen::Title),
                Screen::Brief | Screen::Results => cx.go(Screen::Hub),
                Screen::Bench => bench::leave(&mut cx),
                Screen::Showcase => cx.go(Screen::Results),
                Screen::Settings => {
                    let back = cx.s.settings_return;
                    cx.go(back);
                }
                Screen::Intro => title::finish_intro(&mut cx),
                Screen::Boot | Screen::Title => {}
            },
            Act::Quit => {
                flush(&mut cx.game);
                exit.write(AppExit::Success);
            }
            Act::ToggleLang => {
                let s = &mut cx.game.save.settings;
                s.lang = s.lang.toggled();
                cx.game.touch();
                cx.dirty();
            }
            a => match screen {
                Screen::Title | Screen::Intro | Screen::Settings => title::act(&mut cx, a),
                Screen::Hub | Screen::Brief => hub::act(&mut cx, a),
                Screen::Bench => bench::act(&mut cx, a),
                Screen::Results | Screen::Showcase => results::act(&mut cx, a),
                Screen::Boot => {}
            },
        }
        cx.cues.play(oo_kit::audio::Cue::Click);
    }
}

/// What a screen's draw function can read.
pub struct View<'a> {
    pub game: &'a Game,
    pub s: &'a Session,
    pub lang: Lang,
}

fn redraw(
    mut commands: Commands,
    old: Query<Entity, With<ScreenEntity>>,
    screen: Res<State<Screen>>,
    mut flag: ResMut<Redraw>,
    assets: Option<ResMut<UiAssets>>,
    pal: Res<Palette>,
    game: Res<Game>,
    s: Res<Session>,
    mut last: Local<Option<Screen>>,
) {
    let now = *screen.get();
    if *last != Some(now) {
        *last = Some(now);
        flag.0 = true;
        if now != Screen::Boot {
            spawn_fade(&mut commands, &pal);
        }
    }
    let Some(mut assets) = assets else { return };
    if !std::mem::take(&mut flag.0) {
        return;
    }
    assets.set_lang(game.lang());
    for e in &old {
        commands.entity(e).despawn();
    }
    let v = View {
        game: &game,
        s: &s,
        lang: game.lang(),
    };
    let mut ui = Ui::new(&mut commands, &assets, *pal);
    match now {
        Screen::Boot => {}
        Screen::Title => title::draw_title(&mut ui, &v),
        Screen::Intro => title::draw_intro(&mut ui, &v),
        Screen::Settings => title::draw_settings(&mut ui, &v),
        Screen::Hub => hub::draw_hub(&mut ui, &v),
        Screen::Brief => hub::draw_brief(&mut ui, &v),
        Screen::Bench => bench::draw(&mut ui, &v),
        Screen::Results => results::draw_results(&mut ui, &v),
        Screen::Showcase => results::draw_showcase(&mut ui, &v),
    }
}

fn apply_settings(
    game: Res<Game>,
    mut last: Local<Option<Settings>>,
    mut gain: ResMut<MasterGain>,
    mut reduce: ResMut<ReduceMotion>,
    mut toasts: ResMut<ToastsEnabled>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let s = &game.save.settings;
    if last.as_ref() == Some(s) {
        return;
    }
    *last = Some(s.clone());
    oo_kit::sync_settings(s, &mut gain, &mut reduce, &mut toasts);
    oo_kit::apply_window_mode(s.fullscreen, &mut windows);
}

pub fn flush(game: &mut Game) {
    game.dirty = false;
    game.since_save = 0.0;
    if let Some(slot) = &game.slot
        && let Err(e) = slot.write(&game.save.to_json())
    {
        eprintln!("save failed: {e}");
    }
}

/// Writes at most once a second while there are changes, and on exit.
fn autosave(time: Res<Time>, mut game: ResMut<Game>, mut exits: MessageReader<AppExit>) {
    if exits.read().count() > 0 && game.dirty {
        flush(&mut game);
        return;
    }
    if !game.dirty {
        return;
    }
    game.since_save += time.delta_secs();
    if game.since_save >= 1.0 {
        flush(&mut game);
    }
}
