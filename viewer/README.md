# Protogaea viewer

The stage B2 viewer: the map of the world with its layers, live mode, cards and permanent links. TypeScript with PixiJS (WebGL), built by Vite; it reads the [read API](../server/README.md) of the world server, which serves the built files at `/app/`.

```
npm install
npm run dev      # http://localhost:5173/app/, proxying /v0 to PROTOGAEA_API (http://127.0.0.1:8081)
npm run build    # dist/, for protogaea-server --viewer viewer/dist; builds the core for the browser first
npm run wasm     # only the core: viewer/wasm to public/core.wasm (needs `rustup target add wasm32-unknown-unknown`)
```

## What it shows

- **The map** (`src/map.ts`, `src/terrain.ts`): an illustrated terrain drawn from the cells: soft coastlines with beaches and shallows, water shaded by depth, hill shading, and a texture for each biome (forest canopy, steppe grass, desert dunes, mountain ridges with snow, swamp pools). Zoomed in, the visible cells are redrawn at a higher resolution with finer detail, as map services do. Rift lines are a dotted seam before they open and an amber glow as faults; land bridges carry gold markers with their numbers; wildfires, floods and droughts pulse where they are.
- **Organisms at three scales:** from afar one mark per cell (sized by how many live there, colored by the main clade, red where hunters are), closer one dot per organism (hunters ringed in red), and close up each organism drawn from its genome (`src/glyph.ts`): legs for movement, eyes for perception, jaws for hunting, a shell for defense, a belly for plant eating and eggs for fertility, facing the way it last walked.
- **Layers:** biomes, clade territories (each cell tinted by the clade most of its organisms belong to), food, organisms, rifts, natural events.
- **Interaction:** smooth zoom with the wheel, a double click, pinch or `+`, `-` and `0`; dragging; a minimap with the current view; hovering shows the cell and the organism under the pointer; choosing a clade lights up its range and flies there; clicking a wildfire or a flood in the feed flies to it.
- **The Muller plot** (`src/muller.ts`): clade shares over the whole season, each clade drawn inside the band of the clade it split off from, with the season's phases and land bridge closings on the time axis; hovering shows a clade's name and share, clicking opens its card.
- **The clade tree** (`src/tree.ts`): the named clades on a time axis from founding to extinction, as thick as their peak, hanging from the clade they split off from.
- **Names:** clades appear by their Latin binomial names once named.
- **Live mode:** the viewer polls `/v0/world`; when an epoch arrives, organisms walk from their old cells to their new ones, newborns fade in and the dead fade out. Reduced motion turns this off.
- **The season timeline:** the six phases of the Breaking of Pangea, the current day and when each land bridge closes.
- **Cards** for a clade (population history, parent and child clades, reference genome) and an organism (clade, parent, offspring, cause of death, genome).
- **Today's stories:** the best stories of the last world day (comebacks, crossings, invasions, arms races, the last of a great clade, a new leader, clades split by the sea, falls), each a sentence with the protagonist and a link.
- **A welcome** on the first visit, and again from the ? button: what the viewer is looking at and what to do (stories, predictions, the replay, the Telegram bot). The welcome and the digest fit a phone screen.
- **While you were away:** after an absence of an hour of world time or more, a summary of what changed: population, the leading clade then and now, clades named and extinct, land bridges closed and the best stories.
- **The last day in 30 seconds:** the last world day replayed on the map, organisms walking between frames, with the day's stories as captions when they happen.
- **Predictions for tomorrow:** on a living clade's card, will it still be alive, or larger, this time tomorrow; checked the next day and scored in "My predictions". Kept in the browser only.
- **Visit counts:** for the tests of stage B the viewer reports what is done with it (opened, a story followed, a card opened, a prediction made, a day replayed) under a random id kept in the browser; no names or addresses. The server's `/v0/visits/summary` turns them into the test's measures.
- **Wishes and sparks in the browser** (stage C): the Wish button opens the naturalist panel. A key made in the browser (kept in `localStorage`) signs the wish; rain or drought is placed with a click on the map, a migration starts from a clade's card (the viewer finds the clade's densest 5 × 5 area, which must hold 10 of it, and the target is clicked), a revival from a museum exhibit extinct for 36 epochs or more (the start is clicked); the wish is sent with its first spark, and sparks keep being kindled on up to eight web workers, each with its own copy of the core (yespower in WebAssembly, `spark_api` in [`wasm/`](wasm/src/lib.rs)). Batches go out every three seconds, and every receipt is checked against the operator's key; one that does not check stops the mining. "My wishes" shows each wish's status and its work against the price, and any of them can be supported with more sparks.
- **Miracles** in the feed (applied, or refused with the reason) and on the map: a rain miracle glows teal, a drought amber.
- **The feed** of events: phases, clades named and extinct, a change of the dominant clade, land bridges closing, wildfires, droughts, floods, plague and revivals.
- **The time machine:** a click on the season timeline opens that moment, and ←/→ (Shift for an hour) step through epochs. An epoch between the server's snapshots is recomputed in the browser: the core, compiled to WebAssembly from [`wasm/`](wasm/src/lib.rs), resumes the nearest earlier snapshot in a worker and steps it forward exactly as the server did, and the result's `state_root` is compared with the epoch header in the server's log. Stepping forward continues from the last result. The miracles of each epoch come from `/v0/miracles` and are applied in the same order, so epochs after a miracle check too. Where the epoch has a header signed by the operator, its signature is checked in the browser and the recomputed root must equal the signed one. In the past, the stories and the event feed end at the epoch on the map.
- **The museum:** the named clades that went extinct, greatest or most recent first, each with its life span, peak and habitat, and a link to its last moment on the map.
- **The chronicle:** the world day by day, written from templates (the population, the leader, clades named, land bridges closed, nature), with the day's stories and a link to the end of the day.
- **Comparing two moments:** the pin in the header remembers a moment; open another, and a card shows what changed between them: the population by archetype, clades, the leader, the named clades that appeared and disappeared, and the main stories in between.
- **Inclusion proofs checked in the browser:** an organism's card checks, with the same core, that the organism is part of the state: the server's Merkle proof must reach the `state_root` of the epoch header in the log.
- **Permanent links** in the hash: `#epoch=N` opens any past epoch, `#clade=ID` and `#organism=ID` open a card. Without `epoch` the viewer is live.

The interface is in English and Russian, following the browser's language, and follows the system's light or dark theme.

## License

The viewer is licensed under the GNU Affero General Public License v3.0 only ([LICENSE](LICENSE)). It bundles PixiJS (MIT), the Geist fonts (SIL Open Font License 1.1) and Phosphor icons (MIT) under their own licenses.
