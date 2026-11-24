import { definePreset } from '@pandacss/dev';

export const camcraftPreset = definePreset({
  name: 'camcraft',
  theme: {
    extend: {
      tokens: {
        fonts: {
          sans: {
            value:
              'Inter, "Inter UI", ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif'
          },
          mono: { value: 'ui-monospace, SFMono-Regular, Menlo, monospace' }
        },
        radii: {
          control: { value: '4px' },
          panel: { value: '8px' },
          overlay: { value: '10px' },
          dialog: { value: '12px' }
        }
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
            value: {
              base: 'rgba(111,211,232,.13)',
              _light: 'rgba(13,124,146,.09)'
            }
          },
          cut: { value: { base: '#f5a623', _light: '#8b5100' } },
          cutSoft: {
            value: {
              base: 'rgba(245,166,35,.13)',
              _light: 'rgba(139,81,0,.08)'
            }
          },
          ok: { value: { base: '#5ec27f', _light: '#1f7a43' } },
          warn: { value: { base: '#f0b429', _light: '#96610a' } },
          danger: { value: { base: '#f2635f', _light: '#b3342f' } }
        }
      },
      textStyles: {
        hero: {
          value: { fontSize: '28px', fontWeight: '600', lineHeight: '1.15' }
        },
        dialog: {
          value: { fontSize: '20px', fontWeight: '600', lineHeight: '1.3' }
        },
        title: {
          value: { fontSize: '15px', fontWeight: '600', lineHeight: '1.4' }
        },
        body: {
          value: { fontSize: '13px', fontWeight: '400', lineHeight: '1.4' }
        },
        label: {
          value: { fontSize: '12px', fontWeight: '500', lineHeight: '1.4' }
        },
        small: {
          value: { fontSize: '12px', fontWeight: '400', lineHeight: '1.4' }
        },
        caption: {
          value: {
            fontSize: '11px',
            fontWeight: '500',
            lineHeight: '1.4',
            letterSpacing: '0.06em',
            textTransform: 'uppercase'
          }
        },
        mono: {
          value: {
            fontFamily: 'mono',
            fontSize: '12px',
            fontWeight: '400',
            lineHeight: '1.4'
          }
        }
      },
      recipes: {
        button: {
          className: 'button',
          description: 'Camcraft button',
          base: {
            display: 'inline-flex',
            alignItems: 'center',
            justifyContent: 'center',
            gap: '8px',
            borderRadius: 'control',
            border: '1px solid',
            borderColor: 'border',
            background: 'transparent',
            color: 'text',
            font: 'inherit',
            cursor: 'pointer',
            transition: 'all 120ms ease',
            _hover: { background: 'raised', color: 'textHi' },
            _disabled: { opacity: '0.5', cursor: 'not-allowed' }
          },
          variants: {
            variant: {
              ghost: { borderColor: 'transparent', background: 'transparent' },
              secondary: { borderColor: 'border', background: 'transparent' },
              solid: {
                borderColor: 'border',
                background: 'raised',
                color: 'textHi'
              },
              primary: {
                borderColor: 'sel',
                background: 'selSoft',
                color: 'textHi',
                _hover: { background: 'selSoft', filter: 'brightness(1.08)' }
              },
              danger: {
                borderColor: 'danger',
                background: 'transparent',
                color: 'danger'
              }
            },
            size: {
              sm: { h: '28px', px: '10px', fontSize: '12px' },
              md: { h: '32px', px: '12px', fontSize: '13px' },
              icon: { w: '32px', h: '32px', px: '0' }
            }
          },
          defaultVariants: { variant: 'secondary', size: 'md' }
        },
        panel: {
          className: 'panel',
          description: 'Camcraft floating panel surface',
          base: {
            background: 'panel',
            border: '1px solid',
            borderColor: 'border',
            borderRadius: 'panel',
            boxShadow:
              '0 1px 0 rgba(255,255,255,.03), 0 12px 32px rgba(0,0,0,.42)',
            color: 'text'
          },
          variants: {
            tone: {
              default: {},
              overlay: { background: 'overlay', borderRadius: 'overlay' }
            }
          },
          defaultVariants: { tone: 'default' }
        }
      }
    }
  }
});
