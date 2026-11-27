<script lang="ts">
import { css } from 'styled-system/css';
import type { ToolDetail } from './toolLibrary';

// Organism (107 T7, S11) — the `Preview` column of
// `design/camcraft-workbench-v2/05-settings-tool-library.html`: a drawing of the selected tool's
// cutting geometry plus its metadata. Props only: the caller hands over the `tool_detail` payload
// and this organism calls no command, reads no store and writes nothing.
//
// The drawing answers the owner's correction `jn6b7oo5` verbatim: «los 8mm serian en la zona
// amarilla que es la zona de corte de la herramienta, la zona gris es el vastago. ademas si el
// vastago es de 3.175mm y el diametro de corte es 1mm deberia de ser mas estrecha horizontalmente la
// zona amarilla». One units-per-mm scale is derived from the real cut length, and the same scale
// sizes every feature, so the 3.175 mm shank is visibly wider than the 1 mm cutting zone and the
// 8 mm zone is eight times as tall as it is wide. Nothing is hard-coded from the artboard's sample
// tool: the values come from the payload.
//
// The other three corrections are also honoured: `8` is the cutting zone's vertical dimension
// (over the ochre body, never the grey shank), `Ø1` is the cutter's diameter callout, the guide
// lines the owner deleted (`murjejbx`, `4njc681b`, `hojbo985`, `vzripwjo`) are absent, the drawing
// is centred and fills its card (`1f6b1bi3`), and the SVG is capped at its container's width
// (`op02kggo`).
//
// A value the payload does not hold renders as the app's dash (`—`), never an invented one. The
// backend publishes no shape word for the type (T1's handoff), and the only settled `Typical use`
// copy is S11's end-mill sentence; other kinds read the dash rather than borrow a claim about PCB
// work the model never states.
//
// The payload type is the page's shared contract (`./toolLibrary`), the same declaration the panel
// invokes the command with and the detail form reads.
type Props = {
  /** The `tool_detail` command's payload for the selected tool. */
  detail: ToolDetail;
};

let { detail }: Props = $props();

const UNKNOWN = '—';

// Drawing constants, in user units of the SVG's own coordinate system. The cutting zone is always
// 100 units long; that anchor is what turns millimetres into units, and every other real value is
// multiplied by the same factor. The shank's *length* is the only dimension the payload does not
// carry, so it keeps the artboard's 1.2 proportion to the cutting zone rather than inventing a
// measurement.
const CUT_UNITS = 100;
const SHANK_LENGTH_RATIO = 1.2;
const PAD = 6;
const DIM_GAP = 5;
const DIM_ROOM = 7;
const TEXT_UNITS = 5;
const STROKE_UNITS = 0.8;

/** S11's only settled `Typical use` sentence, keyed by the kind the payload publishes. */
const TYPICAL_USE: Record<string, string> = {
  'flat-end-mill': 'contour and isolation on PCB.'
};

type Drawing = {
  viewBox: string;
  shankX: number;
  shankY: number;
  shankWidth: number;
  shankLength: number;
  cutX: number;
  cutY: number;
  cutWidth: number;
  cutLength: number;
  /** The `8` callout: the cutting zone's real length at the library's precision. */
  cutLabel: string;
  /** The `Ø1` callout: the cutter's real diameter at the library's precision. */
  diameterLabel: string;
  dimX: number;
  dimTop: number;
  dimBottom: number;
  dimTextX: number;
  dimTextY: number;
  diameterDimY: number;
  diameterTextX: number;
  diameterTextY: number;
};

/** A stored number at the library's precision, trimmed of the zeroes a reader does not need. */
function trimmedNumber(value: number): string {
  const text = value.toFixed(3).replace(/0+$/, '').replace(/\.$/, '');
  return text.length > 0 ? text : '0';
}

/**
 * A stored length as the library holds it: up to its three-decimal precision, with the trailing
 * zeroes a reader does not need removed and padded to at least one decimal (`1.0 mm`, `3.175 mm`,
 * `8.0 mm`). Nothing is rounded here — the value is only trimmed and padded for display — and the
 * dash is returned when the library holds no value.
 */
function lengthText(value: number | null): string {
  if (value === null) {
    return UNKNOWN;
  }
  const [whole, fraction = ''] = value.toFixed(3).split('.');
  const trimmed = fraction.replace(/0+$/, '');
  return `${whole}.${trimmed.padEnd(1, '0')}`;
}

const drawing = $derived.by((): Drawing | null => {
  const diameter = detail.diameterMm;
  const shank = detail.shankMm;
  const cut = detail.cutLengthMm;
  if (
    diameter === null ||
    shank === null ||
    cut === null ||
    diameter <= 0 ||
    shank <= 0 ||
    cut <= 0
  ) {
    return null;
  }

  // One scale for the whole drawing: the cutting zone's real length is 100 units long, so a real
  // millimetre is `unitsPerMm` units everywhere.
  const unitsPerMm = CUT_UNITS / cut;
  const shankWidth = shank * unitsPerMm;
  const cutWidth = diameter * unitsPerMm;
  const maxWidth = Math.max(shankWidth, cutWidth);
  const shankLength = CUT_UNITS * SHANK_LENGTH_RATIO;

  const centerX = PAD + maxWidth / 2;
  const shankY = PAD;
  const cutY = shankY + shankLength;

  const dimX = PAD + maxWidth + DIM_GAP;
  const dimTop = cutY;
  const dimBottom = cutY + CUT_UNITS;
  const diameterDimY = dimBottom + DIM_GAP;
  const diameterTextY = diameterDimY + DIM_ROOM + TEXT_UNITS;
  const viewW = dimX + DIM_ROOM + PAD;
  const viewH = diameterTextY + PAD;

  return {
    viewBox: `0 0 ${viewW} ${viewH}`,
    shankX: centerX - shankWidth / 2,
    shankY,
    shankWidth,
    shankLength,
    cutX: centerX - cutWidth / 2,
    cutY,
    cutWidth,
    cutLength: CUT_UNITS,
    cutLabel: trimmedNumber(cut),
    diameterLabel: `Ø${trimmedNumber(diameter)}`,
    dimX,
    dimTop,
    dimBottom,
    dimTextX: dimX + DIM_ROOM,
    dimTextY: cutY + CUT_UNITS / 2,
    diameterDimY,
    diameterTextX: centerX,
    diameterTextY
  };
});

const typicalUse = (kind: string): string => TYPICAL_USE[kind] ?? UNKNOWN;

const root = css({
  display: 'flex',
  flexDirection: 'column',
  gap: '12px',
  minW: '0',
  maxW: '100%'
});
// The artboard's `.preview-card`: the drawing's surface, centred and clipped to its column.
const card = css({
  flex: '1',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  minH: '180px',
  minW: '0',
  maxW: '100%',
  p: '12px',
  overflow: 'hidden',
  bg: 'canvas',
  border: '1px solid',
  borderColor: 'hairline',
  borderRadius: 'overlay'
});
// `op02kggo`: never wider than the container, and the height follows the proportional viewBox.
const svg = css({
  display: 'block',
  w: '100%',
  h: 'auto',
  maxW: '100%',
  maxH: '100%'
});
// `jn6b7oo5`: the shank is the grey body; the cutting zone is the ochre cutter.
const shankShape = css({ fill: 'textOff', stroke: 'text' });
const cuttingShape = css({ fill: 'cut', stroke: 'cut' });
// The dimension lines are the app's accent, not a lit guide line.
const dimension = css({ stroke: 'sel', fill: 'none' });
const dimensionText = css({ fill: 'sel' });
const previewMeta = css({
  fontSize: '11px',
  lineHeight: '1.6',
  color: 'textDim',
  minW: '0'
});
const metaRow = css({ minW: '0', overflowWrap: 'anywhere' });
const metaLabel = css({ fontWeight: '500', color: 'text' });
</script>

<section class={root} aria-label="Preview">
  <div class={card} data-preview-card>
    {#if drawing}
      <svg
        class={svg}
        viewBox={drawing.viewBox}
        role="img"
        aria-label={`Zoomed view of the selected ${detail.kindLabel.toLowerCase()} cutting zone`}
      >
        <!-- Grey shank: the real shank diameter at the drawing's one scale. -->
        <rect
          data-part="shank"
          class={shankShape}
          x={drawing.shankX}
          y={drawing.shankY}
          width={drawing.shankWidth}
          height={drawing.shankLength}
          stroke-width={STROKE_UNITS}
        />
        <!-- Ochre cutting zone: the real diameter wide and the real cut length long. -->
        <rect
          data-part="cutting-zone"
          class={cuttingShape}
          x={drawing.cutX}
          y={drawing.cutY}
          width={drawing.cutWidth}
          height={drawing.cutLength}
          stroke-width={STROKE_UNITS}
        />
        <!-- The `8` callout, over the ochre cutting zone and clear of the shank. -->
        <g class={dimension} stroke-width={STROKE_UNITS}>
          <line
            data-part="cut-dimension"
            x1={drawing.dimX}
            y1={drawing.dimTop}
            x2={drawing.dimX}
            y2={drawing.dimBottom}
          />
          <line
            x1={drawing.dimX - 2}
            y1={drawing.dimTop}
            x2={drawing.dimX + 2}
            y2={drawing.dimTop}
          />
          <line
            x1={drawing.dimX - 2}
            y1={drawing.dimBottom}
            x2={drawing.dimX + 2}
            y2={drawing.dimBottom}
          />
        </g>
        <text
          class={dimensionText}
          x={drawing.dimTextX}
          y={drawing.dimTextY}
          font-size={TEXT_UNITS}
          font-family="ui-monospace, SFMono-Regular, Menlo, monospace"
          text-anchor="middle"
          transform={`rotate(-90 ${drawing.dimTextX} ${drawing.dimTextY})`}
        >
          {drawing.cutLabel}
        </text>
        <!-- The `Ø1` callout, under the cutting edge. -->
        <g class={dimension} stroke-width={STROKE_UNITS}>
          <line
            data-part="diameter-dimension"
            x1={drawing.cutX}
            y1={drawing.diameterDimY}
            x2={drawing.cutX + drawing.cutWidth}
            y2={drawing.diameterDimY}
          />
          <line
            x1={drawing.cutX}
            y1={drawing.diameterDimY - 2}
            x2={drawing.cutX}
            y2={drawing.diameterDimY + 2}
          />
          <line
            x1={drawing.cutX + drawing.cutWidth}
            y1={drawing.diameterDimY - 2}
            x2={drawing.cutX + drawing.cutWidth}
            y2={drawing.diameterDimY + 2}
          />
        </g>
        <text
          class={dimensionText}
          x={drawing.diameterTextX}
          y={drawing.diameterTextY}
          font-size={TEXT_UNITS}
          font-family="ui-monospace, SFMono-Regular, Menlo, monospace"
          text-anchor="middle"
        >
          {drawing.diameterLabel}
        </text>
      </svg>
    {:else}
      <p class={previewMeta} data-preview-fallback>{UNKNOWN}</p>
    {/if}
  </div>

  <div class={previewMeta}>
    <div class={metaRow} data-meta="type">
      <span class={metaLabel}>Type:</span>
      <!-- The backend publishes no shape word, so the type is the kind plus the dash. -->
      {detail.kindLabel} · {UNKNOWN}
    </div>
    <div class={metaRow} data-meta="diameter">
      <span class={metaLabel}>Diameter:</span>
      {lengthText(detail.diameterMm)} mm
    </div>
    <div class={metaRow} data-meta="shank">
      <span class={metaLabel}>Shank:</span>
      {lengthText(detail.shankMm)} mm
    </div>
    <div class={metaRow} data-meta="cut">
      <span class={metaLabel}>Cut:</span>
      {lengthText(detail.cutLengthMm)} mm
    </div>
    <div class={metaRow} data-meta="typical-use">
      <span class={metaLabel}>Typical use:</span>
      {typicalUse(detail.kind)}
    </div>
  </div>
</section>
