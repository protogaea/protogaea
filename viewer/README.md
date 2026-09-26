# Protogaea viewer

The stage B2 viewer: the map of the world with its layers, live mode, cards and permanent links. TypeScript with PixiJS (WebGL), built by Vite; it reads the [read API](../server/README.md) of the world server, which serves the built files at `/app/`.

```
npm install
npm run dev      # http://localhost:5173/app/, proxying /v0 to PROTOGAEA_API (http://127.0.0.1:8081)
npm run build    # dist/, for protogaea-server --viewer viewer/dist
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
- **While you were away:** after an absence of an hour of world time or more, a summary of what changed: population, the leading clade then and now, clades named and extinct, land bridges closed and the best stories.
- **The last day in 30 seconds:** the last world day replayed on the map, organisms walking between frames, with the day's stories as captions when they happen.
- **Predictions for tomorrow:** on a living clade's card, will it still be alive, or larger, this time tomorrow; checked the next day and scored in "My predictions". Kept in the browser only.
- **The feed** of events: phases, clades named and extinct, a change of the dominant clade, land bridges closing, wildfires, droughts, floods, plague and revivals.
- **Permanent links** in the hash: `#epoch=N` opens an archived snapshot, `#clade=ID` and `#organism=ID` open a card. Without `epoch` the viewer is live.

The interface is in English and Russian, following the browser's language, and follows the system's light or dark theme.

## License

The viewer is licensed under the GNU Affero General Public License v3.0 only ([LICENSE](LICENSE)). It bundles PixiJS (MIT), the Geist fonts (SIL Open Font License 1.1) and Phosphor icons (MIT) under their own licenses.
