//! Learning to dock by trial and error. The player writes the reward; a
//! tabular Q-learner does the rest. What it learns is only as good as what
//! the reward asks for.

use oo_base::Rng;

use crate::ctl::{Controller, Senses};
use crate::lang::{Act, DT, Instr, Sensor};
use crate::sim::{Rig, World};

/// The learner's choices: target wheel speed, m/s.
pub const ACTIONS: [f32; 5] = [-0.8, -0.2, 0.0, 0.2, 0.8];
pub const N_ACT: usize = ACTIONS.len();
const E_EDGES: [f32; 8] = [-1.0, -0.4, -0.15, -0.04, 0.04, 0.15, 0.4, 1.0];
const V_EDGES: [f32; 6] = [-0.5, -0.15, -0.03, 0.03, 0.15, 0.5];
pub const E_BINS: usize = E_EDGES.len() + 1;
pub const V_BINS: usize = V_EDGES.len() + 1;
pub const STATES: usize = E_BINS * V_BINS;

pub const EPISODES: usize = 1500;
const ALPHA_MIN: f32 = 0.05;
const GAMMA: f32 = 0.95;
const EPISODE_SECS: f32 = 5.0;
const REWARD_CLIP: f32 = 100.0;

fn bin(edges: &[f32], v: f32) -> usize {
    edges.iter().filter(|e| v > **e).count()
}

/// The table's view of the world: distance to go and speed, bucketed.
pub fn state(s: &Senses) -> usize {
    let e = s[Sensor::Target as usize] - s[Sensor::Pos as usize];
    bin(&E_EDGES, e) * V_BINS + bin(&V_EDGES, s[Sensor::Vel as usize])
}

#[derive(Clone, Debug, PartialEq)]
pub struct QTable {
    pub q: Vec<[f32; N_ACT]>,
    /// Per episode: 1 if it ended docked.
    pub curve: Vec<f32>,
}

impl QTable {
    /// The greedy action; ties go to the lowest index.
    pub fn best(&self, state: usize) -> usize {
        let q = self.q[state];
        let mut b = 0;
        for a in 1..N_ACT {
            if q[a] > q[b] {
                b = a;
            }
        }
        b
    }

    /// Fraction of the last `n` episodes that docked.
    pub fn recent_success(&self, n: usize) -> f32 {
        let tail = &self.curve[self.curve.len().saturating_sub(n)..];
        if tail.is_empty() {
            return 0.0;
        }
        tail.iter().sum::<f32>() / tail.len() as f32
    }
}

/// A training episode's starting world: the template rig with a random target.
fn episode(template: &Rig, rng: &mut Rng) -> World {
    let mut rig = template.clone();
    if let Rig::Cart(c) = &mut rig {
        let side = if rng.chance(0.5) { 1.0 } else { -1.0 };
        c.target = side * rng.range(0.4, 2.0) as f32;
        c.x = rng.range(-0.3, 0.3) as f32;
        c.v = 0.0;
    }
    World::new(rig, Default::default(), Vec::new(), rng.next_u64())
}

/// Trains a table on `reward` (a program that writes `reward`).
pub fn train(reward: &[Instr], template: &Rig, duration: f32, seed: u64) -> QTable {
    let mut rng = Rng::new(seed);
    let mut table = QTable {
        q: vec![[0.0; N_ACT]; STATES],
        curve: Vec::with_capacity(EPISODES),
    };
    let ticks = (duration.min(EPISODE_SECS) / DT).round() as usize;
    // Visits per state-action: each estimate averages its own samples, so
    // rarely seen states learn fast and well-known ones settle down.
    let mut visits = vec![[0u32; N_ACT]; STATES];
    for ep in 0..EPISODES {
        let eps = 0.3 - 0.25 * ep as f32 / EPISODES as f32;
        let mut w = episode(template, &mut rng);
        let mut ctl = Controller::new(reward.to_vec());
        let mut st = state(&w.senses());
        for _ in 0..ticks {
            let a = if rng.chance(eps as f64) {
                rng.below(N_ACT as u64) as usize
            } else {
                table.best(st)
            };
            let mut cmd = [0.0; Act::COUNT];
            cmd[Act::Motor as usize] = ACTIONS[a];
            w.step(cmd);
            let s2 = w.senses();
            let r = ctl.tick(&s2)[Act::Reward as usize].clamp(-REWARD_CLIP, REWARD_CLIP);
            let st2 = state(&s2);
            // Leaving the track ends the episode but is not an escape: the
            // value still looks ahead from where the robot ended up.
            let next = table.q[st2].iter().copied().fold(f32::MIN, f32::max);
            let n = &mut visits[st][a];
            *n += 1;
            let alpha = (1.0 / *n as f32).max(ALPHA_MIN);
            let q = &mut table.q[st][a];
            *q += alpha * (r + GAMMA * next - *q);
            if w.fail.is_some() {
                break;
            }
            st = st2;
        }
        table.curve.push(f32::from(w.fail.is_none() && w.docked()));
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_cover_the_table() {
        let mut s = [0.0; Sensor::COUNT];
        assert_eq!(state(&s), (E_BINS / 2) * V_BINS + V_BINS / 2);
        s[Sensor::Target as usize] = 50.0;
        s[Sensor::Vel as usize] = 50.0;
        assert_eq!(state(&s), STATES - 1);
        s[Sensor::Target as usize] = -50.0;
        s[Sensor::Vel as usize] = -50.0;
        assert_eq!(state(&s), 0);
    }
}
