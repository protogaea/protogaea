//! The canonical world state (spec §15). Only what affects the future belongs here; history
//! goes to reports and, later, the event log.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::genome::Genome;
use crate::ruleset::{Founder, Ruleset};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum Biome {
    DeepWater = 0,
    Shallows = 1,
    Forest = 2,
    Steppe = 3,
    Desert = 4,
    Mountains = 5,
    Swamp = 6,
}

impl Biome {
    pub const COUNT: usize = 7;
    pub const ALL: [Biome; Biome::COUNT] = [
        Biome::DeepWater,
        Biome::Shallows,
        Biome::Forest,
        Biome::Steppe,
        Biome::Desert,
        Biome::Mountains,
        Biome::Swamp,
    ];

    /// The land biome a `habitat` gene value refers to; `None` for a generalist.
    pub fn from_habitat(habitat: u8) -> Option<Biome> {
        match habitat {
            0 => Some(Biome::Forest),
            1 => Some(Biome::Steppe),
            2 => Some(Biome::Desert),
            3 => Some(Biome::Mountains),
            4 => Some(Biome::Swamp),
            _ => None,
        }
    }

    pub fn is_land(self) -> bool {
        !matches!(self, Biome::DeepWater | Biome::Shallows)
    }

    /// Land a flood can cover: everything but water and mountains.
    pub fn floods(self) -> bool {
        self.is_land() && self != Biome::Mountains
    }
}

/// The phase of a cell on a rift line (spec §10). Phases only move forward.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum RiftPhase {
    /// Not on a rift line.
    None = 0,
    /// A published line with no effect yet.
    Crack = 1,
    /// Food grows slower and steps cost more (`fault_growth_pct`, `fault_move_pct`).
    Fault = 2,
    /// The cell has become shallows.
    Shallows = 3,
    /// The cell has become deep water.
    Deep = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub biome: Biome,
    /// Tenths of a food unit.
    pub food: u32,
    /// Food units that turn into food over time.
    pub detritus: u32,
    /// 0–100 (spec §10).
    pub moisture: u8,
    pub rift: RiftPhase,
}

/// One cell of the rift schedule, fixed at genesis (spec §4, §10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rift {
    pub cell: u16,
    /// The land bridge this cell belongs to, from 1; 0 if none.
    pub bridge: u8,
    pub fault_epoch: u64,
    pub shallows_epoch: u64,
    pub deep_epoch: u64,
}

impl Rift {
    /// The phase this cell should be in at an epoch.
    pub fn phase_at(&self, epoch: u64) -> RiftPhase {
        if epoch >= self.deep_epoch {
            RiftPhase::Deep
        } else if epoch >= self.shallows_epoch {
            RiftPhase::Shallows
        } else if epoch >= self.fault_epoch {
            RiftPhase::Fault
        } else {
            RiftPhase::Crack
        }
    }
}

/// An extinct named clade, kept in the state so that `revive` can be checked (spec §12).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MuseumEntry {
    pub clade_id: u32,
    pub parent_id: u32,
    pub reference: Genome,
    pub founded_epoch: u64,
    pub extinct_epoch: u64,
    pub peak_living: u32,
}

/// A temporary effect of a natural event on an area (spec §10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum EffectKind {
    /// After a wildfire: food grows faster.
    Ash = 1,
    /// A great drought: food grows slower.
    Drought = 2,
    /// A flood: land acts as shallows.
    Flood = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effect {
    pub kind: EffectKind,
    pub center: u16,
    pub radius: u8,
    pub remaining_ticks: u32,
}

impl Effect {
    /// Whether the effect covers a cell. Ash covers a disc; a drought covers a square; a flood
    /// covers the land in a disc, except mountains.
    pub fn covers(&self, world: &World, cell: usize) -> bool {
        let (cx, cy) = world.coords(usize::from(self.center));
        let (x, y) = world.coords(cell);
        let (dx, dy, r) = ((x - cx).abs(), (y - cy).abs(), i32::from(self.radius));
        match self.kind {
            EffectKind::Ash => dx * dx + dy * dy <= r * r,
            EffectKind::Drought => dx <= r && dy <= r,
            EffectKind::Flood => dx * dx + dy * dy <= r * r && world.cells[cell].biome.floods(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organism {
    pub id: u64,
    /// 0 for founders.
    pub parent_id: u64,
    pub lineage_id: u32,
    pub clade_id: u32,
    pub cell: u16,
    pub age: u32,
    /// Hundredths of a unit.
    pub energy: i32,
    pub genome: Genome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clade {
    pub id: u32,
    /// 0 for founder clades.
    pub parent_id: u32,
    pub reference: Genome,
    pub founded_epoch: u64,
    pub living: u32,
    /// The most organisms alive at once. It decides whether the clade is named (spec §12).
    pub peak_living: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct World {
    pub world_id: [u8; 16],
    pub ruleset_id: [u8; 32],
    pub width: u16,
    pub height: u16,
    /// The next epoch to run.
    pub epoch: u64,
    pub cells: Vec<Cell>,
    /// Living organisms, ordered by id.
    pub organisms: Vec<Organism>,
    /// Clades with living members, by id.
    pub clades: BTreeMap<u32, Clade>,
    /// Active effects of natural events, in the order they started.
    pub effects: Vec<Effect>,
    /// The rift schedule, by cell index (spec §4). Empty in a world without rifts.
    pub rifts: Vec<Rift>,
    /// The most recently extinct named clades, oldest first (spec §12).
    pub museum: Vec<MuseumEntry>,
    /// The genomes the spore bank can revive, each with the biome it starts in (spec §12).
    pub spore_bank: Vec<Founder>,
    /// The epochs at which the spore bank revived the world.
    pub revivals: Vec<u64>,
    /// The season ended by extinction (spec §12).
    pub ended: bool,
    pub next_organism_id: u64,
    pub next_clade_id: u32,
}

impl World {
    /// The index of the cell at `(x, y)`, or `None` outside the map.
    pub fn index(&self, x: i32, y: i32) -> Option<usize> {
        let (w, h) = (i32::from(self.width), i32::from(self.height));
        if x < 0 || y < 0 || x >= w || y >= h {
            return None;
        }
        Some((y * w + x) as usize)
    }

    /// The `(x, y)` coordinates of a cell index.
    pub fn coords(&self, index: usize) -> (i32, i32) {
        let w = usize::from(self.width);
        ((index % w) as i32, (index / w) as i32)
    }

    /// Whether nothing more can happen: the season ended by extinction, or everything died and
    /// the spore bank cannot help.
    pub fn finished(&self, rules: &Ruleset) -> bool {
        self.ended
            || (self.organisms.is_empty()
                && (self.spore_bank.is_empty() || rules.revival.per_genome == 0))
    }

    /// The canonical encoding of the state.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            128 + self.cells.len() * 10 + self.organisms.len() * 50 + self.rifts.len() * 27,
        );
        out.extend_from_slice(b"PROTOGAEA/STATE/A2.2");
        out.extend_from_slice(&self.world_id);
        out.extend_from_slice(&self.ruleset_id);
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&self.epoch.to_le_bytes());
        out.extend_from_slice(&self.next_organism_id.to_le_bytes());
        out.extend_from_slice(&self.next_clade_id.to_le_bytes());
        for c in &self.cells {
            out.push(c.biome as u8);
            out.extend_from_slice(&c.food.to_le_bytes());
            out.extend_from_slice(&c.detritus.to_le_bytes());
            out.push(c.moisture);
            out.push(c.rift as u8);
        }
        out.extend_from_slice(&(self.effects.len() as u64).to_le_bytes());
        for e in &self.effects {
            out.push(e.kind as u8);
            out.extend_from_slice(&e.center.to_le_bytes());
            out.push(e.radius);
            out.extend_from_slice(&e.remaining_ticks.to_le_bytes());
        }
        out.extend_from_slice(&(self.organisms.len() as u64).to_le_bytes());
        for o in &self.organisms {
            out.extend_from_slice(&o.id.to_le_bytes());
            out.extend_from_slice(&o.parent_id.to_le_bytes());
            out.extend_from_slice(&o.lineage_id.to_le_bytes());
            out.extend_from_slice(&o.clade_id.to_le_bytes());
            out.extend_from_slice(&o.cell.to_le_bytes());
            out.extend_from_slice(&o.age.to_le_bytes());
            out.extend_from_slice(&o.energy.to_le_bytes());
            push_genome(&mut out, &o.genome);
        }
        out.extend_from_slice(&(self.clades.len() as u64).to_le_bytes());
        for c in self.clades.values() {
            out.extend_from_slice(&c.id.to_le_bytes());
            out.extend_from_slice(&c.parent_id.to_le_bytes());
            push_genome(&mut out, &c.reference);
            out.extend_from_slice(&c.founded_epoch.to_le_bytes());
            out.extend_from_slice(&c.living.to_le_bytes());
            out.extend_from_slice(&c.peak_living.to_le_bytes());
        }
        out.extend_from_slice(&(self.rifts.len() as u64).to_le_bytes());
        for r in &self.rifts {
            out.extend_from_slice(&r.cell.to_le_bytes());
            out.push(r.bridge);
            out.extend_from_slice(&r.fault_epoch.to_le_bytes());
            out.extend_from_slice(&r.shallows_epoch.to_le_bytes());
            out.extend_from_slice(&r.deep_epoch.to_le_bytes());
        }
        out.extend_from_slice(&(self.museum.len() as u64).to_le_bytes());
        for m in &self.museum {
            out.extend_from_slice(&m.clade_id.to_le_bytes());
            out.extend_from_slice(&m.parent_id.to_le_bytes());
            push_genome(&mut out, &m.reference);
            out.extend_from_slice(&m.founded_epoch.to_le_bytes());
            out.extend_from_slice(&m.extinct_epoch.to_le_bytes());
            out.extend_from_slice(&m.peak_living.to_le_bytes());
        }
        out.extend_from_slice(&(self.spore_bank.len() as u64).to_le_bytes());
        for s in &self.spore_bank {
            push_genome(&mut out, &s.genome);
            out.push(s.biome as u8);
        }
        out.extend_from_slice(&(self.revivals.len() as u64).to_le_bytes());
        for e in &self.revivals {
            out.extend_from_slice(&e.to_le_bytes());
        }
        out.push(u8::from(self.ended));
        out
    }

    /// Stage A2: a flat BLAKE3 hash of the canonical encoding. The Merkle `state_root` of
    /// spec §15 replaces it later in stage A.
    pub fn state_hash(&self) -> [u8; 32] {
        *blake3::hash(&self.canonical_bytes()).as_bytes()
    }

    /// Checks the invariants the core must maintain (`docs/ruleset.md`, §4).
    pub fn check_invariants(&self, rules: &Ruleset) -> Result<(), String> {
        let expected_cells = usize::from(rules.width) * usize::from(rules.height);
        if self.cells.len() != expected_cells
            || (self.width, self.height) != (rules.width, rules.height)
        {
            return Err("the map size does not match the ruleset".into());
        }
        for (i, c) in self.cells.iter().enumerate() {
            if c.food > rules.biomes[c.biome as usize].food_max {
                return Err(format!("cell {i} has more food than its biome allows"));
            }
            if c.moisture > 100 {
                return Err(format!("cell {i} has moisture above 100"));
            }
        }
        if self.organisms.len() > rules.max_organisms as usize {
            return Err("more organisms than max_organisms".into());
        }
        let mut per_cell = vec![0u8; self.cells.len()];
        let mut per_clade: BTreeMap<u32, u32> = BTreeMap::new();
        let mut previous = 0u64;
        for o in &self.organisms {
            if o.id <= previous || o.id >= self.next_organism_id {
                return Err(format!(
                    "organism {} is out of order or beyond next_organism_id",
                    o.id
                ));
            }
            previous = o.id;
            if !o.genome.is_valid(rules.trait_budget) {
                return Err(format!("organism {} has an invalid genome", o.id));
            }
            if o.energy <= 0 || o.energy > rules.energy_max {
                return Err(format!("organism {} has energy {}", o.id, o.energy));
            }
            let cell = usize::from(o.cell);
            let Some(c) = self.cells.get(cell) else {
                return Err(format!("organism {} is outside the map", o.id));
            };
            if !rules.biomes[c.biome as usize].passable {
                return Err(format!("organism {} stands in an impassable cell", o.id));
            }
            per_cell[cell] += 1;
            if per_cell[cell] > rules.max_per_cell {
                return Err(format!(
                    "cell {cell} holds more than max_per_cell organisms"
                ));
            }
            *per_clade.entry(o.clade_id).or_insert(0) += 1;
        }
        for (id, clade) in &self.clades {
            if clade.id != *id || *id >= self.next_clade_id {
                return Err(format!("clade {id} has an inconsistent id"));
            }
            if clade.living == 0 || per_clade.get(id) != Some(&clade.living) {
                return Err(format!("clade {id} has a wrong living count"));
            }
            if clade.peak_living < clade.living {
                return Err(format!("clade {id} has peak_living below living"));
            }
        }
        if let Some(id) = per_clade.keys().find(|id| !self.clades.contains_key(id)) {
            return Err(format!("organisms refer to a missing clade {id}"));
        }
        let mut on_rift = vec![false; self.cells.len()];
        let mut previous: Option<u16> = None;
        for r in &self.rifts {
            if previous.is_some_and(|p| p >= r.cell) || usize::from(r.cell) >= self.cells.len() {
                return Err(format!("rift cell {} is out of order", r.cell));
            }
            previous = Some(r.cell);
            if !(r.fault_epoch <= r.shallows_epoch && r.shallows_epoch <= r.deep_epoch) {
                return Err(format!("rift cell {} has phases out of order", r.cell));
            }
            on_rift[usize::from(r.cell)] = true;
        }
        for (i, c) in self.cells.iter().enumerate() {
            if on_rift[i] != (c.rift != RiftPhase::None) {
                return Err(format!(
                    "cell {i} has a rift phase that does not match the schedule"
                ));
            }
        }
        if self.museum.len() > rules.museum_capacity as usize {
            return Err("the museum holds more than museum_capacity clades".into());
        }
        if self.revivals.windows(2).any(|w| w[0] >= w[1]) {
            return Err("revivals are out of order".into());
        }
        Ok(())
    }
}

fn push_genome(out: &mut Vec<u8>, g: &Genome) {
    out.extend_from_slice(&g.traits);
    out.push(g.habitat);
    out.push(g.dispersal);
    out.push(g.boldness);
    out.extend_from_slice(&g.hue.to_le_bytes());
}
