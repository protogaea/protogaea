//! Natural events fire when their conditions are met (spec §10).

use protogaea_core::rng::derive;
use protogaea_core::{epoch_seed, genesis, step_epoch, EffectKind, Ruleset};

#[test]
fn forced_events_happen_in_summer() {
    let mut rules = Ruleset::default();
    rules.events.wildfire_ppm = 1_000_000;
    rules.events.wildfire_max_moisture = 101;
    rules.events.drought_ppm = 1_000_000;
    rules.events.plague_min_share_permille = 0;
    rules.events.plague_max_ppm = 1_000_000;
    rules.events.plague_min_members = 1;
    let mut world = genesis(&rules, &derive(b"events", &[]), [9; 16]);
    // One clade holds everyone, so the plague is certain: its chance scales with the share.
    let mut clade = *world.clades.values().next().unwrap();
    clade.living = world.organisms.len() as u32;
    clade.peak_living = clade.living;
    for o in &mut world.organisms {
        o.clade_id = clade.id;
    }
    world.clades.clear();
    world.clades.insert(clade.id, clade);
    world.check_invariants(&rules).unwrap();
    // The middle of summer.
    world.epoch = u64::from(rules.climate.epochs_per_year) * 3 / 8;
    let seed = epoch_seed(&world.world_id, world.epoch, &[0; 32], &world.state_root());
    let report = step_epoch(&mut world, &rules, &seed);
    world.check_invariants(&rules).unwrap();
    assert_eq!(report.wildfires, 1);
    assert_eq!(report.droughts, 1);
    assert_eq!(report.plagues, 1);
    assert!(report.deaths_plague > 0);
    assert!(world.effects.iter().any(|e| e.kind == EffectKind::Ash));
    assert!(world.effects.iter().any(|e| e.kind == EffectKind::Drought));
}

#[test]
fn no_wildfires_or_droughts_outside_summer() {
    let mut rules = Ruleset::default();
    rules.events.wildfire_ppm = 1_000_000;
    rules.events.wildfire_max_moisture = 101;
    rules.events.drought_ppm = 1_000_000;
    let mut world = genesis(&rules, &derive(b"events", &[]), [9; 16]);
    // Spring.
    let seed = epoch_seed(&world.world_id, world.epoch, &[0; 32], &world.state_root());
    let report = step_epoch(&mut world, &rules, &seed);
    assert_eq!((report.wildfires, report.droughts), (0, 0));
}
