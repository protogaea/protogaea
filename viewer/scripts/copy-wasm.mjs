// Copies the core built for the browser (viewer/wasm) into public/, where Vite serves it.
import { copyFileSync, mkdirSync } from 'node:fs';

const from = new URL('../../target/wasm32-unknown-unknown/release/protogaea_wasm.wasm', import.meta.url);
mkdirSync(new URL('../public/', import.meta.url), { recursive: true });
copyFileSync(from, new URL('../public/core.wasm', import.meta.url));
