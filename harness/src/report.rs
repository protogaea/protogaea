//! A self-contained HTML report of one run: the map with playback, population, clade shares
//! (a Muller plot) and mean traits. It is a development tool, not the viewer of stage B.

use std::collections::BTreeMap;
use std::path::Path;

use protogaea_core::genome::HUNTING;
use protogaea_core::{Ruleset, World};
use serde::Serialize;

use crate::metrics::{Check, EpochStats, Summary};

const MAX_FRAMES: u64 = 150;
const MAX_SAMPLES: u64 = 400;
const MAX_SERIES_POINTS: usize = 1500;
const MULLER_CLADES: usize = 14;

#[derive(Serialize)]
struct Frame {
    epoch: u64,
    /// Each organism packed as `cell × 1000 + hue × 2 + hunter`.
    orgs: Vec<u32>,
}

/// Collects map frames and clade counts while a run progresses.
pub struct Recorder {
    frame_every: u64,
    sample_every: u64,
    frames: Vec<Frame>,
    samples: Vec<(u64, BTreeMap<u32, u32>)>,
    hues: BTreeMap<u32, u16>,
}

impl Recorder {
    pub fn new(total_epochs: u64) -> Self {
        Self {
            frame_every: (total_epochs / MAX_FRAMES).max(1),
            sample_every: (total_epochs / MAX_SAMPLES).max(1),
            frames: Vec::new(),
            samples: Vec::new(),
            hues: BTreeMap::new(),
        }
    }

    pub fn observe(&mut self, world: &World) {
        let epoch = world.epoch;
        if epoch.is_multiple_of(self.frame_every) {
            let orgs = world
                .organisms
                .iter()
                .map(|o| {
                    u32::from(o.cell) * 1000
                        + u32::from(o.genome.hue) * 2
                        + u32::from(o.genome.traits[HUNTING] >= 4)
                })
                .collect();
            self.frames.push(Frame { epoch, orgs });
        }
        if epoch.is_multiple_of(self.sample_every) {
            let mut counts = BTreeMap::new();
            for c in world.clades.values() {
                counts.insert(c.id, c.living);
                self.hues.entry(c.id).or_insert(c.reference.hue);
            }
            self.samples.push((epoch, counts));
        }
    }

    pub fn write_html(
        &self,
        path: &Path,
        run: RunInfo<'_>,
        series: &[EpochStats],
        summary: &Summary,
        checks: &[Check],
    ) -> std::io::Result<()> {
        let data = ReportData {
            seed: run.seed,
            days: summary.days,
            epochs_per_day: run.rules.epochs_per_day,
            width: run.world.width,
            height: run.world.height,
            max_organisms: run.rules.max_organisms,
            biomes: run.world.cells.iter().map(|c| c.biome as u8).collect(),
            series: SeriesData::from(series),
            frames: &self.frames,
            muller: self.muller(),
            summary,
            checks,
        };
        let json = serde_json::to_string(&data).map_err(std::io::Error::other)?;
        std::fs::write(path, TEMPLATE.replace("/*DATA*/", &json))
    }

    /// The most present clades over the run, plus "other".
    fn muller(&self) -> MullerData {
        let mut presence: BTreeMap<u32, u64> = BTreeMap::new();
        for (_, counts) in &self.samples {
            for (&id, &n) in counts {
                *presence.entry(id).or_insert(0) += u64::from(n);
            }
        }
        let mut ranked: Vec<(u32, u64)> = presence.into_iter().collect();
        ranked.sort_by_key(|&(id, total)| (std::cmp::Reverse(total), id));
        let top: Vec<u32> = ranked
            .iter()
            .take(MULLER_CLADES)
            .map(|&(id, _)| id)
            .collect();
        let counts = self
            .samples
            .iter()
            .map(|(_, counts)| {
                let mut row: Vec<u32> = top
                    .iter()
                    .map(|id| counts.get(id).copied().unwrap_or(0))
                    .collect();
                let listed: u32 = row.iter().sum();
                row.push(counts.values().sum::<u32>() - listed);
                row
            })
            .collect();
        MullerData {
            epochs: self.samples.iter().map(|(e, _)| *e).collect(),
            clades: top
                .iter()
                .map(|&id| MullerClade {
                    id,
                    hue: self.hues.get(&id).copied().unwrap_or(0),
                })
                .collect(),
            counts,
        }
    }
}

pub struct RunInfo<'a> {
    pub seed: u64,
    pub rules: &'a Ruleset,
    pub world: &'a World,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportData<'a> {
    seed: u64,
    days: f64,
    epochs_per_day: u32,
    width: u16,
    height: u16,
    max_organisms: u32,
    biomes: Vec<u8>,
    series: SeriesData,
    frames: &'a [Frame],
    muller: MullerData,
    summary: &'a Summary,
    checks: &'a [Check],
}

#[derive(Serialize)]
struct SeriesData {
    epoch: Vec<u64>,
    population: Vec<u32>,
    hunters: Vec<u32>,
    armored: Vec<u32>,
    grazers: Vec<u32>,
    clades20: Vec<u32>,
    traits: Vec<Vec<u32>>,
}

impl From<&[EpochStats]> for SeriesData {
    fn from(series: &[EpochStats]) -> Self {
        let step = series.len().div_ceil(MAX_SERIES_POINTS).max(1);
        let picked: Vec<&EpochStats> = series.iter().step_by(step).collect();
        let column = |f: fn(&EpochStats) -> u32| picked.iter().map(|&s| f(s)).collect::<Vec<u32>>();
        Self {
            epoch: picked.iter().map(|s| s.epoch).collect(),
            population: column(|s| s.population),
            hunters: column(|s| s.hunters),
            armored: column(|s| s.armored),
            grazers: column(|s| s.grazers),
            clades20: column(|s| s.clades_20),
            traits: (0..6)
                .map(|k| picked.iter().map(|s| s.trait_means_x10[k]).collect())
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct MullerData {
    epochs: Vec<u64>,
    clades: Vec<MullerClade>,
    counts: Vec<Vec<u32>>,
}

#[derive(Serialize)]
struct MullerClade {
    id: u32,
    hue: u16,
}

const TEMPLATE: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Protogaea run report</title>
<style>
  body { font: 14px/1.4 system-ui, sans-serif; margin: 16px; background: #f7f6f2; color: #222; }
  h1 { font-size: 20px; margin: 0 0 4px; }
  h2 { font-size: 14px; margin: 14px 0 4px; color: #444; }
  .row { display: flex; flex-wrap: wrap; gap: 24px; align-items: flex-start; }
  canvas { background: #fff; border: 1px solid #ddd; max-width: 100%; }
  .muted { color: #666; }
  table { border-collapse: collapse; margin-top: 8px; }
  td { padding: 2px 10px 2px 0; }
  .pass { color: #1e7d32; } .fail { color: #b3261e; } .na { color: #888; }
  button { font: inherit; padding: 2px 12px; }
</style>
</head>
<body>
<h1>Protogaea — seed <span id="seed"></span></h1>
<div class="muted" id="headline"></div>
<table id="checks"></table>
<div class="row">
  <div>
    <h2>World</h2>
    <canvas id="map"></canvas>
    <div><button id="play">Play</button> <input id="slider" type="range" min="0" value="0" style="width: 360px"> <span id="label" class="muted"></span></div>
    <p class="muted">Squares are organisms colored by their neutral hue; related organisms share a color. Dark squares are hunters (hunting ≥ 4).</p>
  </div>
  <div>
    <h2>Population</h2><canvas id="pop" width="580" height="190"></canvas>
    <h2>Clade shares (Muller plot)</h2><canvas id="muller" width="580" height="190"></canvas>
    <h2>Clades with 20 or more organisms</h2><canvas id="clades" width="580" height="110"></canvas>
    <h2>Mean traits</h2><canvas id="traits" width="580" height="170"></canvas>
  </div>
</div>
<script>const DATA = /*DATA*/;</script>
<script>
(function () {
  "use strict";
  const D = DATA;
  const BIOME_COLORS = ["#27496d", "#5a86b4", "#3e7a3b", "#b8b26b", "#e1cb8d", "#8c8b86", "#4e6b57"];
  const CELL = 8;
  let cursorEpoch = null;

  document.getElementById("seed").textContent = D.seed;
  const s = D.summary;
  document.getElementById("headline").textContent =
    `${s.days.toFixed(2)} world days · final population ${s.final_population} · ` +
    `equilibrium ${s.equilibrium_pct.toFixed(1)}% of ${D.maxOrganisms} · ` +
    `${s.generations_per_day.toFixed(1)} generations/day · ` +
    `dominant changes ${s.dominant_changes_per_3_days.toFixed(1)} per 3 days`;
  const checks = document.getElementById("checks");
  for (const c of D.checks) {
    const tr = document.createElement("tr");
    const mark = c.pass === null ? ["na", "—"] : c.pass ? ["pass", "pass"] : ["fail", "fail"];
    tr.innerHTML = `<td class="${mark[0]}">${mark[1]}</td><td></td>`;
    tr.lastChild.textContent = c.name;
    checks.appendChild(tr);
  }

  const map = document.getElementById("map");
  map.width = D.width * CELL;
  map.height = D.height * CELL;
  const ctx = map.getContext("2d");
  const terrain = document.createElement("canvas");
  terrain.width = map.width;
  terrain.height = map.height;
  const t = terrain.getContext("2d");
  D.biomes.forEach((b, i) => {
    t.fillStyle = BIOME_COLORS[b];
    t.fillRect((i % D.width) * CELL, Math.floor(i / D.width) * CELL, CELL, CELL);
  });

  const slider = document.getElementById("slider");
  const label = document.getElementById("label");
  slider.max = Math.max(0, D.frames.length - 1);

  function drawFrame(k) {
    const f = D.frames[k];
    if (!f) return;
    ctx.drawImage(terrain, 0, 0);
    const slots = new Uint8Array(D.width * D.height);
    for (const v of f.orgs) {
      const cell = Math.floor(v / 1000);
      const rest = v % 1000;
      const hue = rest >> 1;
      const hunter = rest & 1;
      const n = slots[cell]++;
      const x = (cell % D.width) * CELL + (n & 1) * 4;
      const y = Math.floor(cell / D.width) * CELL + (n >> 1) * 4;
      ctx.fillStyle = `hsl(${hue}, 75%, ${hunter ? 30 : 60}%)`;
      ctx.fillRect(x, y, 4, 4);
    }
    label.textContent = `day ${(f.epoch / D.epochsPerDay).toFixed(2)} · epoch ${f.epoch} · ${f.orgs.length} organisms`;
    cursorEpoch = f.epoch;
    drawCharts();
  }

  function maxOf(values) {
    let m = 0;
    for (const v of values) if (v > m) m = v;
    return m;
  }

  function lineChart(canvas, xs, series, fixedMax) {
    const c = canvas.getContext("2d");
    const W = canvas.width, H = canvas.height;
    const pad = { l: 40, r: 8, t: 18, b: 16 };
    c.clearRect(0, 0, W, H);
    if (xs.length === 0) return;
    let max = fixedMax || 1;
    if (!fixedMax) for (const s of series) max = Math.max(max, maxOf(s.values));
    const x0 = xs[0], x1 = xs[xs.length - 1];
    const px = (x) => pad.l + ((x - x0) / Math.max(1, x1 - x0)) * (W - pad.l - pad.r);
    const py = (v) => H - pad.b - (v / max) * (H - pad.t - pad.b);
    c.strokeStyle = "#ccc";
    c.beginPath();
    c.moveTo(pad.l, pad.t);
    c.lineTo(pad.l, H - pad.b);
    c.lineTo(W - pad.r, H - pad.b);
    c.stroke();
    c.fillStyle = "#666";
    c.font = "11px system-ui, sans-serif";
    c.fillText(String(Math.round(max)), 4, pad.t + 4);
    c.fillText("0", 4, H - pad.b);
    c.fillText(`day ${(x1 / D.epochsPerDay).toFixed(1)}`, W - 56, H - 3);
    for (const s of series) {
      c.strokeStyle = s.color;
      c.lineWidth = 1.5;
      c.beginPath();
      s.values.forEach((v, i) => (i ? c.lineTo(px(xs[i]), py(v)) : c.moveTo(px(xs[i]), py(v))));
      c.stroke();
    }
    let lx = pad.l + 4;
    for (const s of series) {
      c.fillStyle = s.color;
      c.fillRect(lx, 5, 10, 3);
      c.fillStyle = "#333";
      c.fillText(s.label, lx + 14, 11);
      lx += c.measureText(s.label).width + 28;
    }
    drawCursor(c, px, pad.t, H - pad.b);
  }

  function drawCursor(c, px, top, bottom) {
    if (cursorEpoch === null) return;
    const X = px(cursorEpoch);
    c.strokeStyle = "rgba(0, 0, 0, 0.35)";
    c.lineWidth = 1;
    c.beginPath();
    c.moveTo(X, top);
    c.lineTo(X, bottom);
    c.stroke();
  }

  function mullerChart(canvas) {
    const c = canvas.getContext("2d");
    const W = canvas.width, H = canvas.height;
    c.clearRect(0, 0, W, H);
    const M = D.muller;
    const n = M.epochs.length;
    if (n < 2) return;
    const x0 = M.epochs[0], x1 = M.epochs[n - 1];
    const px = (e) => ((e - x0) / Math.max(1, x1 - x0)) * W;
    const totals = M.counts.map((row) => row.reduce((a, b) => a + b, 0) || 1);
    const bands = M.clades.length + 1;
    const below = (i, j) => {
      let sum = 0;
      for (let m = 0; m < j; m++) sum += M.counts[i][m];
      return sum / totals[i];
    };
    for (let j = 0; j < bands; j++) {
      c.beginPath();
      for (let i = 0; i < n; i++) {
        const X = px(M.epochs[i]), Y = H - below(i, j + 1) * H;
        if (i) c.lineTo(X, Y); else c.moveTo(X, Y);
      }
      for (let i = n - 1; i >= 0; i--) c.lineTo(px(M.epochs[i]), H - below(i, j) * H);
      c.closePath();
      c.fillStyle = j < M.clades.length ? `hsl(${M.clades[j].hue}, 60%, ${45 + (j % 3) * 10}%)` : "#e2e2e2";
      c.fill();
    }
    drawCursor(c, px, 0, H);
  }

  function drawCharts() {
    const S = D.series;
    lineChart(document.getElementById("pop"), S.epoch, [
      { label: "all", color: "#222", values: S.population },
      { label: "grazers", color: "#2e8b57", values: S.grazers },
      { label: "armored", color: "#6d7a86", values: S.armored },
      { label: "hunters", color: "#c0392b", values: S.hunters },
    ]);
    lineChart(document.getElementById("clades"), S.epoch, [
      { label: "clades of 20+", color: "#2c6fbb", values: S.clades20 },
    ]);
    const names = ["movement", "perception", "plants", "hunting", "defense", "fertility"];
    const colors = ["#8e44ad", "#16a085", "#27ae60", "#c0392b", "#6d7a86", "#d35400"];
    lineChart(
      document.getElementById("traits"),
      S.epoch,
      S.traits.map((values, k) => ({ label: names[k], color: colors[k], values: values.map((v) => v / 10) })),
      8
    );
    mullerChart(document.getElementById("muller"));
  }

  let timer = null;
  const play = document.getElementById("play");
  play.addEventListener("click", () => {
    if (timer) {
      clearInterval(timer);
      timer = null;
      play.textContent = "Play";
      return;
    }
    play.textContent = "Pause";
    timer = setInterval(() => {
      const next = (Number(slider.value) + 1) % D.frames.length;
      slider.value = String(next);
      drawFrame(next);
    }, 120);
  });
  slider.addEventListener("input", () => drawFrame(Number(slider.value)));
  drawFrame(0);
})();
</script>
</body>
</html>
"##;
