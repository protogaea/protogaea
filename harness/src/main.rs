//! The balance harness (spec §28): runs worlds offline, measures them and draws reports.

mod metrics;
mod report;
mod runner;

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use protogaea_core::Ruleset;

use metrics::{Summary, Tracker};
use report::{Recorder, RunInfo};
use runner::{hex, Run};

const USAGE: &str = "\
protogaea-harness — runs Protogaea worlds offline and measures them

USAGE:
  protogaea-harness run     [--seed N] [--days D] [--ruleset FILE] [--out DIR]
      One world: prints progress, writes metrics.csv, summary.json and report.html.
  protogaea-harness sweep   [--seeds 1..21] [--days D] [--threads N] [--ruleset FILE] [--out FILE]
      Many worlds in parallel: the ecosystem health checks of spec §28 for each seed.
  protogaea-harness hash    [--seeds 1,2] [--epochs E] [--every K] [--ruleset FILE]
      State hashes at checkpoints, for cross-platform determinism checks.
  protogaea-harness ruleset
      Prints the default ruleset as JSON; edit it and pass it back with --ruleset.

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
        "ruleset" => cmd_ruleset(),
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
    let mut tracker = Tracker::new(&run.world);
    let mut recorder = Recorder::new(epochs);
    recorder.observe(&run.world);
    let per_day = u64::from(run.rules.epochs_per_day);
    let started = std::time::Instant::now();
    println!("seed {seed}: {days} world days ({epochs} epochs)");
    for _ in 0..epochs {
        let report = run.step();
        tracker.record(&run.world, &report, &run.rules);
        recorder.observe(&run.world);
        let epoch = run.world.epoch;
        if epoch.is_multiple_of(per_day) || run.world.organisms.is_empty() {
            let last = tracker.series.last().expect("recorded");
            println!(
                "  day {:>5.2}: population {:>5} (grazers {:>5}, armored {:>5}, hunters {:>5}), clades of 20+ {:>3}",
                epoch as f64 / per_day as f64,
                last.population,
                last.grazers,
                last.armored,
                last.hunters,
                last.clades_20,
            );
        }
        if run.world.organisms.is_empty() {
            println!("  everything died at epoch {epoch}");
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
    let info = RunInfo {
        seed,
        rules: &run.rules,
        world: &run.world,
    };
    let report_path = out.join("report.html");
    recorder
        .write_html(&report_path, info, &tracker.series, &summary, &checks)
        .map_err(|e| format!("cannot write {}: {e}", report_path.display()))?;

    println!(
        "\nfinished in {elapsed:.1} s; state hash {}",
        hex(&run.state_hash())
    );
    print_checks(&summary);
    println!("\nreport: {}", report_path.display());
    Ok(())
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
        let mut tracker = Tracker::new(&run.world);
        for _ in 0..epochs {
            let report = run.step();
            tracker.record(&run.world, &report, &run.rules);
            if run.world.organisms.is_empty() {
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
        "\n{:>6} {:>7} {:>6} {:>7} {:>7} {:>9} {:>8} {:>9} {:>6} {:>8} {:>6}",
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
        "pass"
    );
    for s in &summaries {
        println!(
            "{:>6} {:>7} {:>6} {:>7} {:>7.1} {:>9} {:>8.1} {:>9.2} {:>6.2} {:>8.1} {:>4}/{}",
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
             dominant_changes_per_3_days,longest_dominance_days,cap_ticks_pct,generations_per_day,checks_passed\n",
        );
        for s in &summaries {
            csv.push_str(&format!(
                "{},{},{},{},{:.2},{},{:.2},{:.3},{:.3},{:.2},{}\n",
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
                passed(s),
            ));
        }
        write(&PathBuf::from(path), &csv)?;
        println!("results: {path}");
    }
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
        println!("seed {seed} epoch 0 {}", hex(&run.state_hash()));
        for _ in 0..epochs {
            run.step();
            let epoch = run.world.epoch;
            if epoch.is_multiple_of(every) || epoch == epochs {
                println!("seed {seed} epoch {epoch} {}", hex(&run.state_hash()));
            }
        }
    }
    Ok(())
}

fn cmd_ruleset() -> Result<(), String> {
    let json = serde_json::to_string_pretty(&Ruleset::default()).map_err(|e| e.to_string())?;
    println!("{json}");
    Ok(())
}

fn passed(summary: &Summary) -> usize {
    summary
        .checks()
        .iter()
        .filter(|c| c.pass == Some(true))
        .count()
}

fn print_checks(summary: &Summary) {
    println!("ecosystem health (spec §28, stage A1 subset):");
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
         births,deaths,kills,ticks_at_cap,movement_x10,perception_x10,plants_x10,hunting_x10,defense_x10,fertility_x10\n",
    );
    for s in &tracker.series {
        let traits: Vec<String> = s.trait_means_x10.iter().map(u32::to_string).collect();
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
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
            s.ticks_at_cap,
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
