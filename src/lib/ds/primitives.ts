import { defineRecipe, defineSlotRecipe } from '@pandacss/dev';

// Recipes for the atomic primitives of the workbench chrome (task 095 T1).
//
// Every value here comes from `design/system.md`: semantic colors (`sel`, `cut`, `ok`, `textDim`…),
// geometry tokens (`radius-control`), text styles (`caption`, `label`, `small`, `mono`) and the
// border hierarchy (primary `border`, tertiary `hairline`). The artboard
// `design/camcraft-workbench-v2/02-floating-studio.html` is the visual reference; where the artboard
// and `design/system.md` disagree on a token, the system wins and the divergence is reported, not
// patched here.
//
// `atomButton` is a separate recipe from the existing `button` recipe in `camcraft.preset.ts`
// because the preset's `primary` variant is the `Sel` accent used by the start screen, while the
// artboard's primary CAM action («Generar plan») is the `Cut` accent (system.md: `Cut` = toolpath
// cut / primary fabrication action). Rewriting the preset variant would have changed the running
// start screen, which is out of scope for T1.

const focusRing = {
  outline: '2px solid',
  outlineColor: 'sel',
  outlineOffset: '-1px'
};

export const iconRecipe = defineRecipe({
  className: 'icon',
  description: 'Camcraft inline icon (stroke drawn in currentColor)',
  base: {
    flex: 'none',
    stroke: 'currentColor',
    fill: 'none',
    strokeWidth: '1.6',
    strokeLinecap: 'round',
    strokeLinejoin: 'round'
  },
  variants: {
    size: {
      sm: { w: '14px', h: '14px' },
      md: { w: '16px', h: '16px' },
      lg: { w: '20px', h: '20px' }
    }
  },
  defaultVariants: { size: 'md' }
});

export const iconButtonRecipe = defineRecipe({
  className: 'iconButton',
  description: 'Camcraft square icon-only button',
  base: {
    display: 'grid',
    placeItems: 'center',
    flex: 'none',
    border: '1px solid transparent',
    borderRadius: 'control',
    background: 'transparent',
    color: 'textDim',
    cursor: 'pointer',
    transition: 'background 120ms ease, color 120ms ease',
    _hover: { background: 'raised', color: 'textHi' },
    _focusVisible: focusRing,
    _disabled: { opacity: '0.5', cursor: 'not-allowed' }
  },
  variants: {
    size: {
      sm: { w: '24px', h: '24px' },
      md: { w: '30px', h: '30px' }
    }
  },
  defaultVariants: { size: 'md' }
});

export const atomButtonRecipe = defineRecipe({
  className: 'atomButton',
  description: 'Camcraft labelled button (primary/secondary/ghost)',
  base: {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    gap: '6px',
    border: '1px solid',
    borderRadius: 'control',
    fontFamily: 'sans',
    fontWeight: '500',
    lineHeight: '1.4',
    cursor: 'pointer',
    whiteSpace: 'nowrap',
    transition:
      'background 120ms ease, color 120ms ease, border-color 120ms ease',
    _focusVisible: focusRing,
    _disabled: { opacity: '0.5', cursor: 'not-allowed' }
  },
  variants: {
    variant: {
      primary: {
        color: 'cut',
        borderColor: 'cut/28',
        background: 'cutSoft',
        _hover: { color: 'cut', background: 'cut/22' }
      },
      secondary: {
        color: 'text',
        borderColor: 'border',
        background: 'panel',
        _hover: { color: 'textHi', background: 'raised' }
      },
      ghost: {
        color: 'textDim',
        borderColor: 'transparent',
        background: 'transparent',
        _hover: { color: 'textHi', background: 'raised' }
      }
    },
    size: {
      sm: { h: '28px', px: '10px', fontSize: '12px' },
      md: { h: '30px', px: '12px', fontSize: '13px' }
    }
  },
  defaultVariants: { variant: 'secondary', size: 'md' }
});

export const statusDotRecipe = defineRecipe({
  className: 'statusDot',
  description: 'Camcraft tonal status dot',
  base: {
    w: '8px',
    h: '8px',
    borderRadius: '999px',
    flex: 'none'
  },
  variants: {
    // Only roles that exist in `design/system.md` are available. The artboard also draws copper
    // (#c47a2c) and drill (#7cc7ff) dots; those two literals have no design-system token, so they
    // are reported instead of invented here.
    tone: {
      neutral: { background: 'border' },
      ok: { background: 'ok' },
      selected: { background: 'sel' },
      draft: { background: 'textOff' },
      cut: { background: 'cut' },
      warn: { background: 'warn' },
      danger: { background: 'danger' }
    }
  },
  defaultVariants: { tone: 'neutral' }
});

export const keyHintRecipe = defineRecipe({
  className: 'keyHint',
  description: 'Camcraft keyboard shortcut chip',
  base: {
    display: 'inline-flex',
    alignItems: 'center',
    flex: 'none',
    px: '5px',
    py: '1px',
    borderRadius: 'control',
    border: '1px solid',
    borderColor: 'hairline',
    background: 'raised',
    color: 'textOff',
    textStyle: 'mono',
    whiteSpace: 'nowrap'
  }
});

export const panelSectionRecipe = defineSlotRecipe({
  className: 'panelSection',
  slots: ['root', 'head', 'title', 'rule', 'count', 'body'],
  base: {
    root: { mb: '12px', _last: { mb: '0' } },
    head: {
      display: 'flex',
      alignItems: 'center',
      gap: '8px',
      px: '4px',
      mb: '6px'
    },
    title: {
      m: '0',
      flex: 'none',
      textStyle: 'caption',
      color: 'textOff'
    },
    rule: {
      flex: '1',
      h: '1px',
      background: 'hairline',
      opacity: '0.9'
    },
    count: {
      flex: 'none',
      textStyle: 'mono',
      color: 'textOff',
      whiteSpace: 'nowrap'
    },
    body: { display: 'grid', gap: '1px' }
  }
});

export const panelRowRecipe = defineSlotRecipe({
  className: 'panelRow',
  slots: ['root', 'label', 'meta'],
  base: {
    root: {
      w: '100%',
      minH: '28px',
      display: 'flex',
      alignItems: 'center',
      gap: '8px',
      px: '8px',
      py: '6px',
      borderRadius: 'control',
      border: 'none',
      background: 'transparent',
      color: 'text',
      textAlign: 'left',
      fontFamily: 'sans',
      _focusVisible: focusRing
    },
    label: {
      flex: '1',
      minW: '0',
      overflow: 'hidden',
      textOverflow: 'ellipsis',
      whiteSpace: 'nowrap',
      textStyle: 'small'
    },
    meta: {
      flex: 'none',
      ml: 'auto',
      textStyle: 'mono',
      color: 'textOff',
      whiteSpace: 'nowrap'
    }
  },
  variants: {
    interactive: {
      true: { root: { cursor: 'pointer', _hover: { background: 'raised' } } },
      false: {}
    },
    selected: {
      true: {
        root: { background: 'selSoft', color: 'textHi' }
      },
      false: {}
    }
  },
  compoundVariants: [
    {
      interactive: true,
      selected: true,
      css: { root: { _hover: { background: 'selSoft' } } }
    }
  ],
  defaultVariants: { interactive: false, selected: false }
});

export const commandSearchRecipe = defineSlotRecipe({
  className: 'commandSearch',
  description: 'Camcraft command search field',
  slots: ['root', 'field', 'reason'],
  base: {
    root: {
      w: '380px',
      maxW: '40vw',
      h: '30px',
      display: 'flex',
      alignItems: 'center',
      gap: '8px',
      px: '10px',
      borderRadius: 'control',
      border: '1px solid',
      borderColor: 'border',
      background: 'app',
      color: 'textDim',
      _focusWithin: {
        borderColor: 'sel',
        boxShadow: '0 0 0 2px token(colors.selSoft)'
      }
    },
    field: {
      flex: '1',
      minW: '0',
      border: 'none',
      background: 'transparent',
      color: 'text',
      textStyle: 'small',
      _placeholder: { color: 'textOff' },
      _focus: { outline: 'none' },
      // A field with no query handler is `disabled`: say so in the cursor instead of inviting a
      // click that cannot do anything.
      _disabled: { color: 'textOff', cursor: 'not-allowed' }
    },
    // The reason a field without `onquery` is unavailable. It is rendered as text (never a
    // colour-only hint) and referenced by the field's `aria-describedby`; it shrinks before the
    // field does so the artboard placeholder stays readable.
    reason: {
      flex: '0 1 auto',
      minW: '0',
      maxW: '60%',
      overflow: 'hidden',
      textOverflow: 'ellipsis',
      whiteSpace: 'nowrap',
      textStyle: 'small',
      color: 'textOff'
    }
  }
});

export const primitiveRecipes = {
  icon: iconRecipe,
  iconButton: iconButtonRecipe,
  atomButton: atomButtonRecipe,
  statusDot: statusDotRecipe,
  keyHint: keyHintRecipe
};

export const primitiveSlotRecipes = {
  panelSection: panelSectionRecipe,
  panelRow: panelRowRecipe,
  commandSearch: commandSearchRecipe
};
