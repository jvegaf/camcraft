import { defineConfig } from '@pandacss/dev'

export default defineConfig({
  presets: ['@pandacss/preset-base', '@pandacss/preset-panda'],
  preflight: true,
  include: ['./src/**/*.{js,jsx,ts,tsx,svelte}', './pages/**/*.{js,jsx,ts,tsx}'],
  exclude: [],
  theme: {
    extend: {
      tokens: {
        fonts: {
          sans: {
            value:
              'Inter, "Inter UI", ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif',
          },
          mono: {
            value: 'ui-monospace, SFMono-Regular, Menlo, monospace',
          },
        },
        radii: {
          control: { value: '4px' },
          panel: { value: '6px' },
          overlay: { value: '8px' },
          dialog: { value: '10px' },
        },
      },
      semanticTokens: {
        colors: {
          app: { value: { base: '#12151a', _light: '#eef1f5' } },
          canvas: { value: { base: '#0d1014', _light: '#f7f9fb' } },
          panel: { value: { base: '#171b21', _light: '#ffffff' } },
          raised: { value: { base: '#1e242b', _light: '#f2f5f8' } },
          overlay: { value: { base: '#232a32', _light: '#ffffff' } },
          hairline: { value: { base: '#262d36', _light: '#dde3ea' } },
          border: { value: { base: '#35404c', _light: '#c3ccd6' } },
          textHi: { value: { base: '#e7ecf3', _light: '#12181f' } },
          text: { value: { base: '#c3ccd8', _light: '#2b3440' } },
          textDim: { value: { base: '#8b95a3', _light: '#5f6b7a' } },
          textOff: { value: { base: '#5c6673', _light: '#8b95a3' } },
          sel: { value: { base: '#6fd3e8', _light: '#0d7c92' } },
          selSoft: {
            value: { base: 'rgba(111,211,232,.13)', _light: 'rgba(13,124,146,.09)' },
          },
          cut: { value: { base: '#f5a623', _light: '#8b5100' } },
          cutSoft: {
            value: { base: 'rgba(245,166,35,.13)', _light: 'rgba(139,81,0,.08)' },
          },
          ok: { value: { base: '#5ec27f', _light: '#1f7a43' } },
          warn: { value: { base: '#f0b429', _light: '#96610a' } },
          danger: { value: { base: '#f2635f', _light: '#b3342f' } },
        },
      },
    },
  },
  outdir: 'styled-system',
})
