# Protogaea viewer

The stage B2 viewer: the map of the world with its layers, live mode, cards and permanent links. TypeScript with PixiJS (WebGL), built by Vite; it reads the [read API](../server/README.md) of the world server, which serves the built files at `/app/`.

```
npm install
npm run dev      # http://localhost:5173/app/, proxying /v0 to PROTOGAEA_API (http://127.0.0.1:8081)
npm run build    # dist/, for protogaea-server --viewer viewer/dist
```

## What it shows

- **The map** (`src/map.ts`): biomes, food, organisms colored by their neutral `hue` gene (hunters darker, armored outlined), current rift phases and the rift schedule (cells that will sink, land bridges in gold), and natural events. Zoomed in, each organism is drawn from its genome (`src/glyph.ts`): legs for movement, eyes for perception, jaws for hunting, a shell for defense, a belly for plant eating and eggs for fertility.
- **Live mode:** the viewer polls `/v0/world`; when an epoch arrives, organisms walk from their old cells to their new ones, newborns fade in and the dead fade out. Reduced motion turns this off.
- **The season timeline:** the six phases of the Breaking of Pangea, the current day and when each land bridge closes.
- **Cards** for a clade (population history, parent and child clades, reference genome) and an organism (clade, parent, offspring, cause of death, genome).
- **The feed** of events: phases, clades named and extinct, a change of the dominant clade, land bridges closing, wildfires, droughts, floods, plague and revivals.
- **Permanent links** in the hash: `#epoch=N` opens an archived snapshot, `#clade=ID` and `#organism=ID` open a card. Without `epoch` the viewer is live.

The interface is in English and Russian, following the browser's language, and follows the system's light or dark theme.
