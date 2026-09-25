//! The balance harness (spec §28): runs worlds offline, measures them and draws reports.

mod live;
mod metrics;
mod report;
mod runner;

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use protogaea_core::{Biome, Ruleset};

use metrics::{Summary, Tracker};
use report::Recorder;
use runner::{hex, season_phase, Run};

const USAGE: &str = "\
protogaea-harness — runs Protogaea worlds offline and measures them

USAGE:
  protogaea-harness run     [--seed N] [--days D] [--ruleset FILE] [--out DIR]
      One world: prints progress, writes metrics.csv, summary.json and report.html.
  protogaea-harness sweep   [--seeds 1..21] [--days D] [--threads N] [--ruleset FILE] [--out FILE]
      Many worlds in parallel: the ecosystem health checks of spec §28 for each seed.
  protogaea-harness hash    [--seeds 1,2] [--epochs E] [--every K] [--ruleset FILE]
      State hashes at checkpoints, for cross-platform determinism checks.
  protogaea-harness maps    [--seeds 1..21] [--ruleset FILE]
      The Season 1 map criteria of spec §4 for candidate seeds, without running them.
  protogaea-harness bench   [--seed N] [--warmup D] [--days D] [--ruleset FILE]
      Performance on one thread (spec §29): after D world days of warm-up, the time of each epoch
      (the step and the state root) over the next D days, against the targets of a world day
      under 30 s and an epoch under 100 ms at the 95th percentile.
  protogaea-harness ruleset
      Prints the default ruleset as JSON; edit it and pass it back with --ruleset.
  protogaea-harness live    [--seed N] [--data DIR] [--listen ADDR] [--epoch-seconds S]
                            [--snapshot-every K] [--ruleset FILE]
      A live world in real time: one epoch every S seconds (300), its page on http://ADDR
      (127.0.0.1:8080), a snapshot in DIR (runs/live) every K epochs (12). Resumes from the
      snapshot if there is one. Set PROTOGAEA_PASSWORD (and PROTOGAEA_USER, by default
      `protogaea`) to put the page behind a password.

Seeds are a list (1,2,5), a half-open range (1..21) or a single number.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, rest) = match args.split_first() {
        Some((command, rest)) => (command.as_str(), rest),
        None => ("help", &[][..]),
    };
    let result = match command {
        "run" => cmd_run(rest),
        "sweep" => cmd_sweep(rest),
        "hash" => cmd_hash(rest),
        "maps" => cmd_maps(rest),
        "bench" => cmd_bench(rest),
        "ruleset" => cmd_ruleset(),
        "live" => cmd_live(rest),
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_run(args: &[String]) -> Result<(), String> {
    let opts = Options::parse(args, &["seed", "days", "ruleset", "out"])?;
    let seed: u64 = opts.get("seed", 1)?;
    let days: f64 = opts.get("days", 3.0)?;
    let rules = load_rules(&opts)?;
    let out = PathBuf::from(
        opts.value("out")
            .map_or_else(|| format!("runs/seed-{seed}"), str::to_string),
    );
    std::fs::create_dir_all(&out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;

    let epochs = epochs_for(days, &rules);
    let mut run = Run::new(seed, rules);
    let mut tracker = Tracker::new(&run.world, &run.rules, &run.plan);
    let mut recorder = Recorder::new(epochs);
    recorder.observe(&run.world);
    let per_day = u64::from(run.rules.epochs_per_day);
    let started = std::time::Instant::now();
    println!(
        "seed {seed}: {days} world days ({epochs} epochs); {} plates, {} land bridges",
        run.plan.plate_count,
        run.plan.bridges.len()
    );
    for _ in 0..epochs {
        let report = run.step();
        tracker.record(&run.world, &report, &run.rules);
        recorder.observe(&run.world);
        let epoch = run.world.epoch;
        for id in &report.bridges_closed {
            println!(
                "  day {:>5.2}: land bridge {id} closed",
                epoch as f64 / per_day as f64
            );
        }
        if report.revival {
            println!(
                "  day {:>5.2}: the spore bank revived the world with {} organisms",
                epoch as f64 / per_day as f64,
                report.revived
            );
        }
        let finished = run.world.finished(&run.rules);
        if epoch.is_multiple_of(per_day) || finished {
            let last = tracker.series.last().expect("recorded");
            println!(
                "  day {:>5.2}: population {:>5} (grazers {:>5}, armored {:>5}, hunters {:>5}), clades of 20+ {:>3}, continents {} ({})",
                epoch as f64 / per_day as f64,
                last.population,
                last.grazers,
                last.armored,
                last.hunters,
                last.clades_20,
                last.continents,
                season_phase(&run.rules, epoch).1,
            );
            if run.plan.plate_count > 1 && season_phase(&run.rules, epoch).0 >= 3 {
                print_plates(&run);
            }
        }
        if finished {
            if run.world.ended {
                println!("  the season ended by extinction at epoch {epoch}");
            } else {
                println!("  everything died at epoch {epoch}");
            }
            break;
        }
    }
    let elapsed = started.elapsed().as_secs_f64();
    let summary = tracker.summary(seed, &run.rules);
    let checks = summary.checks();

    write_metrics_csv(&out.join("metrics.csv"), &tracker)?;
    let summary_json = serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?;
    write(&out.join("summary.json"), &summary_json)?;
    let rules_json = serde_json::to_string_pretty(&run.rules).map_err(|e| e.to_string())?;
    write(&out.join("ruleset.json"), &rules_json)?;
    let report_path = out.join("report.html");
    recorder
        .write_html(&report_path, &run, &tracker.series, &summary, &checks)
        .map_err(|e| format!("cannot write {}: {e}", report_path.display()))?;

    println!(
        "\nfinished in {elapsed:.1} s; state root {}",
        hex(&run.state_root())
    );
    print_checks(&summary);
    println!("\nreport: {}", report_path.display());
    Ok(())
}

/// One line per plate: its population, dominant clade and mean traits.
fn print_plates(run: &Run) {
    let profiles = metrics::plate_profiles(&run.world, &run.plan.plates);
    for p in &profiles {
        let traits: Vec<String> = p
            .trait_means_x10
            .iter()
            .map(|&t| format!("{:.1}", f64::from(t) / 10.0))
            .collect();
        println!(
            "      plate {}: {:>5} organisms, clade {:>4} holds {:>3}%, M P G H D F {}",
            p.plate,
            p.population,
            p.dominant_clade,
            p.dominant_permille / 10,
            traits.join(" "),
        );
    }
    println!(
        "      distance between plate means {:.1}, between dominant clades {:.1}; clade makeup differs by {}%, hues by {:.0}°",
        f64::from(metrics::centroid_divergence_x10(&profiles)) / 10.0,
        f64::from(metrics::divergence_x10(&run.world, &run.plan.plates)) / 10.0,
        metrics::composition_permille(&profiles) / 10,
        metrics::hue_divergence(&profiles),
    );
}

fn cmd_sweep(args: &[String]) -> Result<(), String> {
    let opts = Options::parse(args, &["seeds", "days", "threads", "ruleset", "out"])?;
    let seeds = parse_seeds(opts.value("seeds").unwrap_or("1..21"))?;
    let days: f64 = opts.get("days", 3.0)?;
    let default_threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let threads: usize = opts.get("threads", default_threads)?;
    let rules = load_rules(&opts)?;
    let epochs = epochs_for(days, &rules);
    println!(
        "{} seeds × {days} world days on {threads} threads",
        seeds.len()
    );

    let started = std::time::Instant::now();
    let results: Mutex<Vec<Option<Summary>>> = Mutex::new(vec![None; seeds.len()]);
    let next = AtomicUsize::new(0);
    let work = || loop {
        let k = next.fetch_add(1, Ordering::Relaxed);
        let Some(&seed) = seeds.get(k) else {
            break;
        };
        let mut run = Run::new(seed, rules.clone());
        let mut tracker = Tracker::new(&run.world, &run.rules, &run.plan);
        for _ in 0..epochs {
            let report = run.step();
            tracker.record(&run.world, &report, &run.rules);
            if run.world.finished(&run.rules) {
                break;
            }
        }
        let summary = tracker.summary(seed, &run.rules);
        println!(
            "  seed {seed} done: {}/{} checks pass",
            passed(&summary),
            summary.checks().len()
        );
        results.lock().expect("no thread panicked")[k] = Some(summary);
    };
    if threads <= 1 {
        work();
    } else {
        std::thread::scope(|scope| {
            for _ in 0..threads.min(seeds.len()) {
                scope.spawn(work);
            }
        });
    }
    let summaries: Vec<Summary> = results
        .into_inner()
        .expect("no thread panicked")
        .into_iter()
        .map(|s| s.expect("every seed ran"))
        .collect();

    println!(
        "\n{:>6} {:>7} {:>6} {:>7} {:>7} {:>9} {:>8} {:>9} {:>6} {:>8} {:>4} {:>6} {:>5} {:>6} {:>8} {:>5} {:>6}",
        "seed",
        "extinct",
        "final",
        "hunters",
        "equil%",
        "min cl20",
        "chg/3d",
        "dom days",
        "cap%",
        "gen/day",
        "rev",
        "cont",
        "div+",
        "cdiv+",
        "comp%→",
        "hue°",
        "pass"
    );
    for s in &summaries {
        println!(
            "{:>6} {:>7} {:>6} {:>7} {:>7.1} {:>9} {:>8.1} {:>9.2} {:>6.2} {:>8.1} {:>4} {:>6} {:>5} {:>6} {:>8} {:>5.0} {:>4}/{}",
            s.seed,
            if s.extinct { "yes" } else { "no" },
            s.final_population,
            s.final_hunters,
            s.equilibrium_pct,
            s.min_clades_20_after_day_3
                .map_or("—".to_string(), |m| m.to_string()),
            s.dominant_changes_per_3_days,
            s.longest_dominance_days,
            s.cap_ticks_pct,
            s.generations_per_day,
            s.revivals,
            format!("{}/{}", s.final_continents, s.plates),
            s.divergence_growth
                .map_or("—".to_string(), |g| format!("{g:.1}")),
            s.centroid_divergence_growth
                .map_or("—".to_string(), |g| format!("{g:.1}")),
            format!(
                "{}→{}",
                s.start_composition_permille
                    .map_or("—".to_string(), |c| (c / 10).to_string()),
                s.final_composition_permille / 10
            ),
            s.final_hue_divergence,
            passed(s),
            s.checks().len(),
        );
    }
    println!("\npass rate per check:");
    let names: Vec<&str> = summaries
        .first()
        .map(|s| s.checks().iter().map(|c| c.name).collect())
        .unwrap_or_default();
    for (i, name) in names.iter().enumerate() {
        let judged: Vec<bool> = summaries
            .iter()
            .filter_map(|s| s.checks()[i].pass)
            .collect();
        let ok = judged.iter().filter(|&&p| p).count();
        if judged.is_empty() {
            println!("  {:>5}  {name} (run too short to judge)", "—");
        } else {
            println!("  {:>4}%  {name}", ok * 100 / judged.len());
        }
    }
    println!("\nfinished in {:.1} s", started.elapsed().as_secs_f64());

    if let Some(path) = opts.value("out") {
        let mut csv = String::from(
            "seed,extinct,final_population,min_population,equilibrium_pct,min_clades_20_after_day_3,\
             dominant_changes_per_3_days,longest_dominance_days,cap_ticks_pct,generations_per_day,\
             revivals,ended_by_extinction,drowned,plates,final_continents,divergence_growth,\
             centroid_divergence_growth,final_composition_permille,final_hue_divergence,checks_passed\n",
        );
        for s in &summaries {
            csv.push_str(&format!(
                "{},{},{},{},{:.2},{},{:.2},{:.3},{:.3},{:.2},{},{},{},{},{},{},{},{},{:.1},{}\n",
                s.seed,
                s.extinct,
                s.final_population,
                s.min_population,
                s.equilibrium_pct,
                s.min_clades_20_after_day_3
                    .map_or(String::new(), |m| m.to_string()),
                s.dominant_changes_per_3_days,
                s.longest_dominance_days,
                s.cap_ticks_pct,
                s.generations_per_day,
                s.revivals,
                s.ended_by_extinction,
                s.drowned,
                s.plates,
                s.final_continents,
                s.divergence_growth
                    .map_or(String::new(), |g| format!("{g:.1}")),
                s.centroid_divergence_growth
                    .map_or(String::new(), |g| format!("{g:.1}")),
                s.final_composition_permille,
                s.final_hue_divergence,
                passed(s),
            ));
        }
        write(&PathBuf::from(path), &csv)?;
        println!("results: {path}");
    }
    Ok(())
}

fn cmd_bench(args: &[String]) -> Result<(), String> {
    let opts = Options::parse(args, &["seed", "warmup", "days", "ruleset"])?;
    let seed: u64 = opts.get("seed", 1)?;
    let warmup: f64 = opts.get("warmup", 2.0)?;
    let days: f64 = opts.get("days", 1.0)?;
    let rules = load_rules(&opts)?;
    let per_day = f64::from(rules.epochs_per_day);
    let mut run = Run::new(seed, rules.clone());
    println!("seed {seed}: warming up for {warmup} world days");
    for _ in 0..epochs_for(warmup, &rules) {
        run.step();
    }
    let epochs = epochs_for(days, &rules);
    println!(
        "timing {epochs} epochs from day {:.2}, population {}",
        run.world.epoch as f64 / per_day,
        run.world.organisms.len()
    );
    let mut times = Vec::with_capacity(epochs as usize);
    let (mut min_pop, mut max_pop) = (usize::MAX, 0);
    for _ in 0..epochs {
        let started = std::time::Instant::now();
        run.step();
        times.push(started.elapsed().as_secs_f64());
        min_pop = min_pop.min(run.world.organisms.len());
        max_pop = max_pop.max(run.world.organisms.len());
    }
    let total: f64 = times.iter().sum();
    times.sort_by(f64::total_cmp);
    let pct = |p: f64| times[((times.len() - 1) as f64 * p).round() as usize] * 1000.0;
    let day_seconds = total / times.len() as f64 * per_day;
    let (p50, p95, max) = (pct(0.5), pct(0.95), pct(1.0));
    println!("population {min_pop}–{max_pop}");
    println!(
        "epoch: p50 {p50:.1} ms, p95 {p95:.1} ms, max {max:.1} ms (target: p95 under 100 ms) — {}",
        if p95 < 100.0 { "pass" } else { "FAIL" }
    );
    println!(
        "world day: {day_seconds:.1} s (target: under 30 s) — {}",
        if day_seconds < 30.0 { "pass" } else { "FAIL" }
    );
    println!("state root {}", hex(&run.state_root()));
    Ok(())
}

fn cmd_hash(args: &[String]) -> Result<(), String> {
    let opts = Options::parse(args, &["seeds", "epochs", "every", "ruleset"])?;
    let seeds = parse_seeds(opts.value("seeds").unwrap_or("1,2"))?;
    let epochs: u64 = opts.get("epochs", 500)?;
    let every: u64 = opts.get("every", 100)?;
    if every == 0 {
        return Err("--every must be positive".into());
    }
    let rules = load_rules(&opts)?;
    for seed in seeds {
        let mut run = Run::new(seed, rules.clone());
        println!("seed {seed} epoch 0 {}", hex(&run.state_root()));
        for _ in 0..epochs {
            run.step();
            let epoch = run.world.epoch;
            if epoch.is_multiple_of(every) || epoch == epochs {
                println!("seed {seed} epoch {epoch} {}", hex(&run.state_root()));
            }
        }
    }
    Ok(())
}

/// How a candidate seed's map meets the Season 1 criteria (spec §4).
struct MapCriteria {
    plates: u8,
    bridge_biomes: Vec<Biome>,
    /// The fewest land cells a plate keeps after the breakup.
    min_land: usize,
    /// Every plate keeps land of all five biomes.
    all_biomes: bool,
    /// The fewest founders that start on a plate.
    min_founders: usize,
}

impl MapCriteria {
    fn of(seed: u64, rules: &Ruleset) -> Self {
        let run = Run::new(seed, rules.clone());
        let plan = &run.plan;
        let count = usize::from(plan.plate_count.max(1));
        let mut on_rift = vec![false; run.genesis_biomes.len()];
        for r in &plan.rifts {
            on_rift[usize::from(r.cell)] = true;
        }
        let mut land = vec![0usize; count];
        let mut kinds = vec![[false; Biome::COUNT]; count];
        for (i, &b) in run.genesis_biomes.iter().enumerate() {
            if b.is_land() && !on_rift[i] {
                let p = usize::from(plan.plates[i]);
                land[p] += 1;
                kinds[p][b as usize] = true;
            }
        }
        let mut founders = vec![0usize; count];
        for o in &run.world.organisms {
            founders[usize::from(plan.plates[usize::from(o.cell)])] += 1;
        }
        Self {
            plates: plan.plate_count,
            bridge_biomes: plan
                .bridges
                .iter()
                .map(|b| run.genesis_biomes[usize::from(b.center)])
                .collect(),
            min_land: land.iter().copied().min().unwrap_or(0),
            all_biomes: kinds.iter().all(|k| {
                Biome::ALL
                    .iter()
                    .filter(|b| b.is_land())
                    .all(|&b| k[b as usize])
            }),
            min_founders: founders.iter().copied().min().unwrap_or(0),
        }
    }

    fn distinct_bridges(&self) -> bool {
        let b = &self.bridge_biomes;
        (0..b.len()).all(|i| !b[..i].contains(&b[i]))
    }

    fn pass(&self, rules: &Ruleset) -> bool {
        (rules.rifts.plates_min..=rules.rifts.plates_max).contains(&self.plates)
            && (3..=5).contains(&self.bridge_biomes.len())
            && self.distinct_bridges()
            && self.all_biomes
            && self.min_founders >= 40
    }
}

fn cmd_maps(args: &[String]) -> Result<(), String> {
    let opts = Options::parse(args, &["seeds", "ruleset"])?;
    let seeds = parse_seeds(opts.value("seeds").unwrap_or("1..21"))?;
    let rules = load_rules(&opts)?;
    println!(
        "Season 1 map criteria (spec §4): 3–5 land bridges in different biomes; every future \
         continent keeps all five land biomes and starts with at least 40 organisms.\n"
    );
    println!(
        "{:>6} {:>6} {:>30} {:>9} {:>10} {:>12} {:>5}",
        "seed", "plates", "land bridges", "min land", "5 biomes", "min founders", "pass"
    );
    let mut passing = Vec::new();
    for &seed in &seeds {
        let m = MapCriteria::of(seed, &rules);
        let bridges: Vec<&str> = m.bridge_biomes.iter().map(|&b| biome_name(b)).collect();
        let pass = m.pass(&rules);
        println!(
            "{:>6} {:>6} {:>30} {:>9} {:>10} {:>12} {:>5}",
            seed,
            m.plates,
            format!(
                "{}{}",
                bridges.join(","),
                if m.distinct_bridges() {
                    ""
                } else {
                    " (repeats)"
                }
            ),
            m.min_land,
            if m.all_biomes { "yes" } else { "no" },
            m.min_founders,
            if pass { "yes" } else { "no" },
        );
        if pass {
            passing.push(seed.to_string());
        }
    }
    println!(
        "\n{} of {} seeds meet every criterion: {}",
        passing.len(),
        seeds.len(),
        if passing.is_empty() {
            "none".to_string()
        } else {
            passing.join(", ")
        }
    );
    Ok(())
}

fn biome_name(biome: Biome) -> &'static str {
    match biome {
        Biome::DeepWater => "deep water",
        Biome::Shallows => "shallows",
        Biome::Forest => "forest",
        Biome::Steppe => "steppe",
        Biome::Desert => "desert",
        Biome::Mountains => "mountains",
        Biome::Swamp => "swamp",
    }
}

fn cmd_ruleset() -> Result<(), String> {
    let json = serde_json::to_string_pretty(&Ruleset::default()).map_err(|e| e.to_string())?;
    println!("{json}");
    Ok(())
}

fn cmd_live(args: &[String]) -> Result<(), String> {
    let opts = Options::parse(
        args,
        &[
            "seed",
            "data",
            "listen",
            "epoch-seconds",
            "snapshot-every",
            "ruleset",
        ],
    )?;
    let epoch_seconds: u64 = opts.get("epoch-seconds", 300)?;
    let snapshot_every: u64 = opts.get("snapshot-every", 12)?;
    if epoch_seconds == 0 || snapshot_every == 0 {
        return Err("--epoch-seconds and --snapshot-every must be positive".into());
    }
    let credentials = match std::env::var("PROTOGAEA_PASSWORD") {
        Ok(password) if !password.is_empty() => {
            let user = std::env::var("PROTOGAEA_USER")
                .ok()
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| "protogaea".to_string());
            Some((user, password))
        }
        _ => None,
    };
    live::run_live(live::LiveOptions {
        seed: opts.get("seed", 1)?,
        data: PathBuf::from(opts.value("data").unwrap_or("runs/live")),
        listen: opts.value("listen").unwrap_or("127.0.0.1:8080").to_string(),
        epoch_seconds,
        snapshot_every,
        rules: load_rules(&opts)?,
        credentials,
    })
}

fn passed(summary: &Summary) -> usize {
    summary
        .checks()
        .iter()
        .filter(|c| c.pass == Some(true))
        .count()
}

fn print_checks(summary: &Summary) {
    println!("ecosystem health (spec §28, the checks measured so far):");
    for check in summary.checks() {
        let mark = match check.pass {
            Some(true) => "pass",
            Some(false) => "FAIL",
            None => " —  ",
        };
        println!("  [{mark}] {}", check.name);
    }
}

fn epochs_for(days: f64, rules: &Ruleset) -> u64 {
    (days * f64::from(rules.epochs_per_day)).round().max(1.0) as u64
}

fn load_rules(opts: &Options) -> Result<Ruleset, String> {
    let rules = match opts.value("ruleset") {
        None => Ruleset::default(),
        Some(path) => {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
            serde_json::from_str(&text).map_err(|e| format!("invalid ruleset {path}: {e}"))?
        }
    };
    rules.validate()?;
    Ok(rules)
}

fn write_metrics_csv(path: &std::path::Path, tracker: &Tracker) -> Result<(), String> {
    let mut csv = String::from(
        "epoch,population,grazers,armored,hunters,clades,clades_20,dominant_clade,dominant_permille,\
         births,deaths,kills,plague_deaths,drowned,floods,wildfires,droughts,plagues,revivals,ticks_at_cap,\
         continents,divergence_x10,movement_x10,perception_x10,plants_x10,hunting_x10,defense_x10,fertility_x10\n",
    );
    for s in &tracker.series {
        let traits: Vec<String> = s.trait_means_x10.iter().map(u32::to_string).collect();
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            s.epoch,
            s.population,
            s.grazers,
            s.armored,
            s.hunters,
            s.clades,
            s.clades_20,
            s.dominant_clade,
            s.dominant_permille,
            s.births,
            s.deaths,
            s.kills,
            s.plague_deaths,
            s.drowned,
            s.floods,
            s.wildfires,
            s.droughts,
            s.plagues,
            s.revivals,
            s.ticks_at_cap,
            s.continents,
            s.divergence_x10,
            traits.join(","),
        ));
    }
    write(path, &csv)
}

fn write(path: &std::path::Path, contents: &str) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn parse_seeds(text: &str) -> Result<Vec<u64>, String> {
    let bad = || format!("invalid seeds `{text}`: use 1,2,5 or 1..21");
    if let Some((a, b)) = text.split_once("..") {
        let (a, b): (u64, u64) = (
            a.trim().parse().map_err(|_| bad())?,
            b.trim().parse().map_err(|_| bad())?,
        );
        if a >= b {
            return Err(bad());
        }
        return Ok((a..b).collect());
    }
    text.split(',')
        .map(|s| s.trim().parse().map_err(|_| bad()))
        .collect()
}

/// `--key value` pairs. The harness is outside consensus, so a hash map is fine here.
struct Options {
    values: HashMap<String, String>,
}

impl Options {
    fn parse(args: &[String], allowed: &[&str]) -> Result<Self, String> {
        let mut values = HashMap::new();
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            let key = arg
                .strip_prefix("--")
                .ok_or_else(|| format!("unexpected argument `{arg}`"))?;
            if !allowed.contains(&key) {
                return Err(format!("unknown option `--{key}`"));
            }
            let value = it
                .next()
                .ok_or_else(|| format!("`--{key}` needs a value"))?;
            values.insert(key.to_string(), value.clone());
        }
        Ok(Self { values })
    }

    fn value(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    fn get<T: FromStr>(&self, key: &str, default: T) -> Result<T, String> {
        match self.values.get(key) {
            None => Ok(default),
            Some(v) => v
                .parse()
                .map_err(|_| format!("invalid value for --{key}: `{v}`")),
        }
    }
}
