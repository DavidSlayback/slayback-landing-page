//! A trained policy, as the Python trainers export it (`ml/policy.py`): the actor MLP's layers from `policy.bin`
//! (`b"BOXP"`, version 1, the layer count, then per layer its sizes, weights `out x in` row-major and biases,
//! little-endian; tanh between layers). It plays greedily: the action with the highest logit.

use crate::{ACTIONS, Action, FLAT};
use game_core::agent::Obs;

struct Layer {
    inp: usize,
    out: usize,
    w: Vec<f32>,
    b: Vec<f32>,
}

/// The actor network, ready to act.
pub struct Policy {
    layers: Vec<Layer>,
}

impl Policy {
    /// Reads a `policy.bin`. It must take the flat observation ([`FLAT`]) and give one logit per action ([`ACTIONS`]).
    pub fn from_bytes(data: &[u8]) -> Result<Policy, String> {
        let mut at = 0usize;
        let mut take = |n: usize| -> Result<&[u8], String> {
            let s = data.get(at..at + n).ok_or("policy file cut short")?;
            at += n;
            Ok(s)
        };
        if take(4)? != b"BOXP" {
            return Err("not a policy file".into());
        }
        let u32_ = |b: &[u8]| u32::from_le_bytes(b.try_into().unwrap()) as usize;
        let version = u32_(take(4)?);
        if version != 1 {
            return Err(format!("policy file version {version}, not 1"));
        }
        let count = u32_(take(4)?);
        let mut layers = Vec::with_capacity(count);
        for _ in 0..count {
            let (inp, out) = (u32_(take(4)?), u32_(take(4)?));
            let floats = |b: &[u8]| b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect();
            let w = floats(take(4 * inp * out)?);
            let b = floats(take(4 * out)?);
            layers.push(Layer { inp, out, w, b });
        }
        let (first, last) = (layers.first().ok_or("no layers")?, layers.last().unwrap());
        if first.inp != FLAT || last.out != ACTIONS {
            return Err(format!("policy maps {} to {}, not {FLAT} to {ACTIONS}", first.inp, last.out));
        }
        Ok(Policy { layers })
    }

    pub fn load(path: &std::path::Path) -> Result<Policy, String> {
        Policy::from_bytes(&std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?)
    }

    /// The action logits for a flat observation.
    pub fn logits(&self, x: &[f32]) -> Vec<f32> {
        let mut x = x.to_vec();
        for (k, l) in self.layers.iter().enumerate() {
            let mut y = l.b.clone();
            for (o, yo) in y.iter_mut().enumerate() {
                let row = &l.w[o * l.inp..(o + 1) * l.inp];
                *yo += row.iter().zip(&x).map(|(w, v)| w * v).sum::<f32>();
            }
            if k + 1 < self.layers.len() {
                y.iter_mut().for_each(|v| *v = v.tanh());
            }
            debug_assert_eq!(y.len(), l.out);
            x = y;
        }
        x
    }

    /// The greedy action for `obs`.
    pub fn act(&self, obs: &Obs) -> Action {
        let logits = self.logits(&obs.flat());
        let best = logits.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map_or(0, |(i, _)| i);
        Action(best as u8)
    }
}
