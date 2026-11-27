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
  jsxFramework: 'svelte',
  conditions: {
    extend: {
      _light: '[data-theme=light] &',
      _dark: '[data-theme=dark] &',
      light: '[data-theme=light] &',
      dark: '[data-theme=dark] &'
    }
  }
});
