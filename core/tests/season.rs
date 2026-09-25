//! The Breaking of Pangea, floods, the museum and the spore bank (spec §4, §10, §12).

use std::collections::{BTreeMap, VecDeque};

use protogaea_core::rifts::Plan;
use protogaea_core::rng::derive;
use protogaea_core::{
    epoch_seed, genesis, step_epoch, world_plan, Biome, EffectKind, EpochReport, RiftPhase,
    Ruleset, World,
};

fn seed(k: u64) -> [u8; 32] {
    derive(b"season-test", &[&k.to_le_bytes()])
}

fn step(world: &mut World, rules: &Ruleset) -> EpochReport {
    let seed = epoch_seed(&world.world_id, world.epoch, &[7; 32], &world.state_hash());
    let report = step_epoch(world, rules, &seed);
    world.check_invariants(rules).unwrap();
    report
}

/// Connected groups of passable cells (8-neighborhood) in a terrain, as a label per cell.
fn components(rules: &Ruleset, biomes: &[Biome]) -> Vec<Option<usize>> {
    let (w, h) = (i64::from(rules.width), i64::from(rules.height));
    let passable = |i: usize| rules.biomes[biomes[i] as usize].passable;
    let mut label = vec![None; biomes.len()];
    let mut next = 0;
    for start in 0..biomes.len() {
        if !passable(start) || label[start].is_some() {
            continue;
        }
        label[start] = Some(next);
        let mut queue = VecDeque::from([start]);
        while let Some(i) = queue.pop_front() {
            let (x, y) = (i as i64 % w, i as i64 / w);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    let j = (ny * w + nx) as usize;
                    if passable(j) && label[j].is_none() {
                        label[j] = Some(next);
                        queue.push_back(j);
                    }
                }
            }
        }
        next += 1;
    }
    label
}

/// The terrain once every rift cell has reached its phase at `epoch`.
fn terrain_at(biomes: &[Biome], plan: &Plan, epoch: u64) -> Vec<Biome> {
    let mut out = biomes.to_vec();
    for r in &plan.rifts {
        let cell = usize::from(r.cell);
        match r.phase_at(epoch) {
            RiftPhase::Deep => out[cell] = Biome::DeepWater,
            RiftPhase::Shallows if out[cell].is_land() => out[cell] = Biome::Shallows,
            _ => {}
        }
    }
    out
}

#[test]
fn the_plan_has_plates_rifts_and_bridges_on_schedule() {
    let rules = Ruleset::default();
    let day = u64::from(rules.epochs_per_day);
    let r = &rules.rifts;
    for k in 0..12 {
        let (_, plan) = world_plan(&rules, &seed(k));
        assert!((3..=4).contains(&plan.plate_count), "seed {k}");
        assert!(!plan.rifts.is_empty(), "seed {k}");
        assert!(
            plan.bridges.len() + 1 >= usize::from(plan.plate_count),
            "seed {k}: {} bridges for {} plates",
            plan.bridges.len(),
            plan.plate_count
        );
        let mut closings: Vec<u64> = plan.bridges.iter().map(|b| b.close_epoch).collect();
        closings.sort_unstable();
        closings.dedup();
        assert_eq!(
            closings.len(),
            plan.bridges.len(),
            "bridges close one by one"
        );
        for rift in &plan.rifts {
            assert_eq!(rift.fault_epoch, u64::from(r.fault_day) * day);
            if rift.bridge > 0 {
                assert_eq!(rift.shallows_epoch, rift.deep_epoch);
                assert!(
                    (u64::from(r.bridges_from_day) * day..u64::from(r.bridges_to_day) * day)
                        .contains(&rift.deep_epoch)
                );
            } else {
                assert!(
                    (u64::from(r.shallows_from_day) * day..u64::from(r.deep_from_day) * day)
                        .contains(&rift.shallows_epoch)
                );
                assert!(
                    (u64::from(r.deep_from_day) * day..u64::from(r.bridges_from_day) * day)
                        .contains(&rift.deep_epoch)
                );
            }
        }
    }
}

#[test]
fn bridges_join_the_plates_until_they_close_and_then_the_plates_are_isolated() {
    let rules = Ruleset::default();
    let day = u64::from(rules.epochs_per_day);
    for k in 0..12 {
        let (biomes, plan) = world_plan(&rules, &seed(k));

        // Phase IV: every rift cell but the bridges is deep water, and the land still holds
        // together.
        let straits = terrain_at(
            &biomes,
            &plan,
            u64::from(rules.rifts.bridges_from_day) * day - 1,
        );
        let land = straits.iter().filter(|b| b.is_land()).count();
        let label = components(&rules, &straits);
        let mut land_per_group: BTreeMap<usize, usize> = BTreeMap::new();
        for (i, b) in straits.iter().enumerate() {
            if b.is_land() {
                *land_per_group.entry(label[i].unwrap()).or_default() += 1;
            }
        }
        let largest = land_per_group.values().max().copied().unwrap_or(0);
        assert!(
            largest * 100 >= land * 85,
            "seed {k}: the largest landmass in phase IV holds {largest} of {land} land cells"
        );

        // After the last bridge: no passable cell touches a passable cell of another plate,
        // so no path leads from one continent to another.
        let end = terrain_at(&biomes, &plan, u64::from(rules.rifts.bridges_to_day) * day);
        let w = i64::from(rules.width);
        let passable = |i: usize| rules.biomes[end[i] as usize].passable;
        for i in 0..end.len() {
            if !passable(i) {
                continue;
            }
            let (x, y) = (i as i64 % w, i as i64 / w);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= i64::from(rules.height) {
                        continue;
                    }
                    let j = (ny * w + nx) as usize;
                    assert!(
                        !passable(j) || plan.plates[j] == plan.plates[i],
                        "seed {k}: cells {i} and {j} join plates after the breakup"
                    );
                }
            }
        }
    }
}

#[test]
fn a_compressed_season_breaks_the_continent_apart() {
    // A world day of 12 epochs, so that the whole breakup fits into 90 epochs.
    let mut rules = Ruleset {
        epochs_per_day: 12,
        ..Ruleset::default()
    };
    rules.rifts.fault_day = 1;
    rules.rifts.shallows_from_day = 2;
    rules.rifts.deep_from_day = 3;
    rules.rifts.bridges_from_day = 5;
    rules.rifts.bridges_to_day = 7;
    rules.validate().unwrap();
    let (_, plan) = world_plan(&rules, &seed(3));
    let mut world = genesis(&rules, &seed(3), [3; 16]);
    world.check_invariants(&rules).unwrap();
    let mut changes = 0;
    let mut closed = Vec::new();
    let mut moved = 0;
    for _ in 0..90 {
        let report = step(&mut world, &rules);
        changes += report.rift_changes;
        closed.extend(report.bridges_closed);
        moved += report.rescued + report.deaths_drowned;
        for o in &world.organisms {
            assert!(world.cells[usize::from(o.cell)].biome != Biome::DeepWater);
        }
    }
    closed.sort_unstable();
    let expected: Vec<u8> = plan.bridges.iter().map(|b| b.id).collect();
    assert_eq!(closed, expected);
    assert!(changes > 0);
    assert!(moved > 0, "some organisms stood on sinking cells");
    for r in &world.rifts {
        let cell = world.cells[usize::from(r.cell)];
        assert_eq!(cell.rift, RiftPhase::Deep);
        assert_eq!(cell.biome, Biome::DeepWater);
    }
    assert!(!world.organisms.is_empty());
}

/// Moves one organism onto a rift cell that sinks at the next epoch boundary.
fn sink_under(world: &mut World) -> u64 {
    let k = world
        .rifts
        .iter()
        .position(|r| r.bridge == 0 && world.cells[usize::from(r.cell)].biome.is_land())
        .expect("a land rift cell");
    let rift = &mut world.rifts[k];
    rift.fault_epoch = world.epoch;
    rift.shallows_epoch = world.epoch;
    rift.deep_epoch = world.epoch;
    let cell = rift.cell;
    let occupied = world.organisms.iter().filter(|o| o.cell == cell).count();
    let o = world
        .organisms
        .iter_mut()
        .find(|o| o.cell != cell)
        .expect("an organism");
    if occupied < 4 {
        o.cell = cell;
    }
    o.id
}

#[test]
fn organisms_on_sinking_cells_are_carried_to_land_or_drown() {
    let rules = Ruleset::default();
    let mut world = genesis(&rules, &seed(5), [5; 16]);
    let id = sink_under(&mut world);
    let report = step(&mut world, &rules);
    assert!(report.rescued >= 1);
    assert_eq!(report.deaths_drowned, 0);
    if let Some(o) = world.organisms.iter().find(|o| o.id == id) {
        assert!(world.cells[usize::from(o.cell)].biome != Biome::DeepWater);
    }

    let mut rules = Ruleset::default();
    rules.rifts.rescue_radius = 0;
    let mut world = genesis(&rules, &seed(5), [5; 16]);
    sink_under(&mut world);
    let report = step(&mut world, &rules);
    assert!(report.deaths_drowned >= 1);
    assert_eq!(report.rescued, 0);
}

#[test]
fn a_spring_flood_turns_land_into_shallows_for_a_while() {
    let mut rules = Ruleset::default();
    rules.events.flood_ppm = 1_000_000;
    let mut world = genesis(&rules, &seed(6), [6; 16]);
    let report = step(&mut world, &rules);
    assert_eq!(report.floods, 1);
    let flood = *world
        .effects
        .iter()
        .find(|e| e.kind == EffectKind::Flood)
        .unwrap();
    assert_eq!(
        flood.remaining_ticks,
        rules.events.flood_ticks - rules.ticks_per_epoch
    );
    let covered: Vec<usize> = (0..world.cells.len())
        .filter(|&c| flood.covers(&world, c))
        .collect();
    assert!(!covered.is_empty());
    for &c in &covered {
        assert_eq!(world.cells[c].food, 0, "nothing grows under a flood");
        assert_eq!(world.cells[c].moisture, 100);
        assert!(world.cells[c].biome.floods());
    }
    // No new floods; the first one recedes after `flood_ticks`.
    rules.events.flood_ppm = 0;
    step(&mut world, &rules);
    assert!(world.effects.iter().all(|e| e.kind != EffectKind::Flood));
    step(&mut world, &rules);
    assert!(
        covered.iter().any(|&c| world.cells[c].food > 0),
        "food grows back"
    );
}

/// Keeps only the first `n` organisms and fixes the clade counts. Clades left without
/// members are dropped without passing through the museum.
fn keep_only(world: &mut World, n: usize) {
    world.organisms.truncate(n);
    for clade in world.clades.values_mut() {
        clade.living = 0;
    }
    for o in &world.organisms {
        world.clades.get_mut(&o.clade_id).unwrap().living += 1;
    }
    world.clades.retain(|_, c| c.living > 0);
}

#[test]
fn named_clades_that_die_out_go_to_the_museum() {
    let rules = Ruleset {
        museum_capacity: 1,
        ..Ruleset::default()
    };
    let mut world = genesis(&rules, &seed(7), [7; 16]);
    // Remove two founder clades at once: both were named (50 members at genesis).
    let doomed: Vec<u32> = world.clades.keys().take(2).copied().collect();
    world.organisms.retain(|o| !doomed.contains(&o.clade_id));
    for id in &doomed {
        world.clades.get_mut(id).unwrap().living = 0;
    }
    let report = step(&mut world, &rules);
    let extinct: Vec<u32> = report.clades_extinct.iter().map(|c| c.id).collect();
    assert!(doomed.iter().all(|id| extinct.contains(id)));
    // The museum keeps the most recent entry only.
    assert_eq!(world.museum.len(), 1);
    assert_eq!(world.museum[0].clade_id, doomed[1]);
    assert_eq!(world.museum[0].peak_living, rules.organisms_per_lineage);
}

#[test]
fn the_spore_bank_revives_a_dying_world_and_ends_the_season_if_it_keeps_dying() {
    let mut rules = Ruleset::default();
    rules.revival.cooldown_epochs = 2;
    let mut world = genesis(&rules, &seed(8), [8; 16]);
    keep_only(&mut world, 10);
    let report = step(&mut world, &rules);
    assert!(report.revival);
    let genomes = world.spore_bank.len() as u32;
    assert_eq!(report.revived, genomes * rules.revival.per_genome);
    assert_eq!(world.revivals, vec![0]);
    // The cooldown holds back the next revival.
    keep_only(&mut world, 10);
    assert!(!step(&mut world, &rules).revival);
    keep_only(&mut world, 10);
    assert!(step(&mut world, &rules).revival);
    assert!(!world.ended);
    // A third revival within seven world days ends the season.
    step(&mut world, &rules);
    keep_only(&mut world, 10);
    assert!(step(&mut world, &rules).revival);
    assert!(world.ended);
    assert!(world.finished(&rules));
    keep_only(&mut world, 0);
    assert!(
        !step(&mut world, &rules).revival,
        "no revivals after the end"
    );
}
