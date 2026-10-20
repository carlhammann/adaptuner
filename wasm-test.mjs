import fs from 'node:fs';
import init, { AdaptunerEngine } from './pkg/adaptuner.js';

const wasm = fs.readFileSync('./pkg/adaptuner_bg.wasm');
await init({ module_or_path: wasm });

const engine = new AdaptunerEngine();

console.log('--- batch 1 ---');

const result1 = engine.process([
  {
    time: 0,
    channel: 0,
    message: {
      type: 'noteOn',
      note: 60,
      velocity: 100,
    },
  },
]);

console.dir(result1, { depth: null });

console.log('--- batch 2 ---');

const result2 = engine.process([
  {
    time: 500000,
    channel: 0,
    message: {
      type: 'noteOff',
      note: 60,
      velocity: 0,
    },
  },
]);

console.dir(result2, { depth: null });