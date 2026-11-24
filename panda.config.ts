import { defineConfig } from '@pandacss/dev';
import presetBase from '@pandacss/preset-base';
import presetPanda from '@pandacss/preset-panda';
import { camcraftPreset } from './src/lib/ds/camcraft.preset';

export default defineConfig({
  preflight: true,
  presets: [presetBase, presetPanda, camcraftPreset],
  include: ['./src/**/*.{js,ts,svelte}'],
  exclude: [],
  outdir: 'styled-system',
  jsxFramework: 'svelte'
});
