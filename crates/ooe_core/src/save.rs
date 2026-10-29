//! What the game remembers between sessions.

use std::collections::BTreeMap;

use oo_base::{Campaign, Settings};
use serde::{Deserialize, Serialize};

use crate::content;
use crate::task::Task;

/// 2: chapters; v1 level ids are renamed on load (`content::RENAMED`).
pub const SAVE_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SaveData {
    pub version: u32,
    pub campaign: Campaign,
    pub settings: Settings,
    /// The player's latest program per level id.
    pub work: BTreeMap<String, String>,
    /// Level ids whose Predict question has been answered (right or wrong).
    pub predicted: Vec<String>,
    /// Insight cards collected, by level id.
    pub insights: Vec<String>,
    /// Fault level ids whose evidence the player has pointed at.
    pub evidence: Vec<String>,
    pub seen_intro: bool,
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            campaign: Campaign::default(),
            settings: Settings::default(),
            work: BTreeMap::new(),
            predicted: Vec::new(),
            insights: Vec::new(),
            evidence: Vec::new(),
            seen_intro: false,
        }
    }
}

impl SaveData {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("save data serialises")
    }

    /// Parses a save; unknown level ids are dropped so content changes never
    /// brick a save.
    pub fn from_json(text: &str) -> Option<Self> {
        let mut s: SaveData = serde_json::from_str(text).ok()?;
        if s.version < 2 {
            s.rename_v1();
        }
        s.work.retain(|id, _| content::task(id).is_some());
        s.campaign
            .records
            .retain(|id, _| content::task(id).is_some());
        s.version = SAVE_VERSION;
        Some(s)
    }

    /// Moves v1 progress onto the levels those ids became. Runs before any
    /// new level can reuse an old id (`where-am-i`).
    fn rename_v1(&mut self) {
        let map = |id: &str| {
            content::RENAMED
                .iter()
                .find(|(old, _)| *old == id)
                .map_or(id.to_string(), |(_, new)| new.to_string())
        };
        self.work = std::mem::take(&mut self.work)
            .into_iter()
            .map(|(k, v)| (map(&k), v))
            .collect();
        self.campaign.records = std::mem::take(&mut self.campaign.records)
            .into_iter()
            .map(|(k, v)| (map(&k), v))
            .collect();
        for list in [&mut self.predicted, &mut self.insights] {
            for id in list.iter_mut() {
                *id = map(id);
            }
        }
    }

    /// The player's program, or the starter if they have not touched it.
    pub fn work_for(&self, task: &Task) -> String {
        self.work
            .get(task.id)
            .cloned()
            .unwrap_or_else(|| task.starter.to_string())
    }

    pub fn is_unlocked(&self, task: &Task) -> bool {
        task.requires.iter().all(|r| self.campaign.is_solved(r))
    }

    /// Fault levels count as solved only once their evidence is found.
    pub fn has_evidence(&self, task: &Task) -> bool {
        task.clue.is_none() || self.evidence.iter().any(|x| x == task.id)
    }

    pub fn note_once(list: &mut Vec<String>, id: &str) -> bool {
        if list.iter().any(|x| x == id) {
            return false;
        }
        list.push(id.to_string());
        true
    }
}

#[cfg(test)]
mod tests {
    use oo_base::Outcome;

    use super::*;

    fn win(s: &mut SaveData, id: &str) {
        s.campaign.record_solve(
            id,
            Outcome {
                stars: 3,
                metric: 1.0,
                alt: 1.0,
            },
            false,
        );
    }

    #[test]
    fn round_trips_and_drops_unknown_levels() {
        let mut s = SaveData::default();
        let t = &content::tasks()[0];
        s.work.insert(t.id.into(), t.reference.into());
        s.work.insert("gone".into(), "MOV motor, 1".into());
        win(&mut s, t.id);
        win(&mut s, "gone");
        let back = SaveData::from_json(&s.to_json()).unwrap();
        assert_eq!(back.work_for(t), t.reference);
        assert!(!back.work.contains_key("gone"));
        assert_eq!(back.campaign.total_stars(), 3);
    }

    #[test]
    fn v1_saves_move_to_their_new_levels() {
        let v1 = r#"{"version": 1, "work": {"where-am-i": "MOV est, 0", "wake-up": "MOV motor, 1"},
            "campaign": {"records": {"lift-off": {"solved": true, "stars": 2}}},
            "predicted": ["balance"], "insights": ["sim-to-real"]}"#;
        let s = SaveData::from_json(v1).unwrap();
        assert_eq!(s.version, SAVE_VERSION);
        assert_eq!(s.work.get("how-far").map(String::as_str), Some("MOV est, 0"));
        assert_eq!(s.work.get("eyes-open").map(String::as_str), Some("MOV motor, 1"));
        assert!(!s.work.contains_key("where-am-i"));
        assert_eq!(s.campaign.stars("sag"), 2);
        assert_eq!(s.predicted, ["four-gains"]);
        assert_eq!(s.insights, ["it-worked-in-sim"]);
        // Already-current saves keep their ids.
        let again = SaveData::from_json(&s.to_json()).unwrap();
        assert_eq!(again, s);
    }

    #[test]
    fn unlocks_follow_requirements() {
        let mut s = SaveData::default();
        let ts = content::tasks();
        assert!(s.is_unlocked(&ts[0]));
        assert!(!s.is_unlocked(&ts[1]));
        win(&mut s, ts[0].id);
        assert!(s.is_unlocked(&ts[1]));
    }

    #[test]
    fn faults_need_their_evidence() {
        let mut s = SaveData::default();
        let fault = content::task("it-worked-in-sim").unwrap();
        assert!(s.has_evidence(content::task("eyes-open").unwrap()));
        assert!(!s.has_evidence(fault));
        SaveData::note_once(&mut s.evidence, fault.id);
        let back = SaveData::from_json(&s.to_json()).unwrap();
        assert!(back.has_evidence(fault));
    }

    #[test]
    fn tolerates_partial_and_garbage_saves() {
        assert!(SaveData::from_json("not json").is_none());
        let s = SaveData::from_json("{\"seen_intro\": true}").unwrap();
        assert!(s.seen_intro);
        assert_eq!(s.settings, Settings::default());
    }
}
