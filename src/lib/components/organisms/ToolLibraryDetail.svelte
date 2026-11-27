<script lang="ts">
import { css, cx } from 'styled-system/css';
import {
  badge as badgeRecipe,
  input as inputRecipe
} from 'styled-system/recipes';
import type { ToolDetail } from './toolLibrary';

// Organism (107 T6, S10 + S14) — the `Detail` column of
// `design/camcraft-workbench-v2/05-settings-tool-library.html`: the tool's title with its `T` number
// badge, the `Geometry` section and the read-only `Feeds & speeds` section.
//
// What is editable is not a UI preference: the artboard's own markup renders `Name`, `Number` and
// `Flutes` as `<input>`s and every other row as a `.ro` text row, and the write command of 107 T2
// (`ToolSaveRequest`) accepts exactly those three keys and denies unknown ones. Making a read-only
// field editable here would offer an edit the backend rejects, so `Diameter`, the lengths, the feeds
// and `Stepover` stay text.
//
// Props, not stores: the caller hands over the `tool_detail` payload of one tool and receives every
// edit through `onedit` as a one-key patch. The draft belongs to the settings dialog (096 saves only
// on `Save changes`), so this organism calls no command, writes nothing, and cannot drift from the
// dialog's draft.
//
// Two values the backend publishes as absent (`stepoverPercent`, and the shape word S10's `Type` row
// would show next to the kind) render as the app's em dash — never a zero, never an empty cell. A
// tool the library holds without a number gets the same dash in its badge, never `Tnull`.
//
// The payload type itself is the page's shared contract (`./toolLibrary`): the panel, this form and
// the preview read one declaration, so a payload change cannot reach only some of them.

/**
 * What an edit reports: exactly the keys `ToolSaveRequest` accepts, so the panel cannot express a
 * change to a read-only field. One key per call keeps the draft patch unambiguous.
 */
type ToolDetailEdit = {
  name?: string;
  number?: number | null;
  flutes?: number | null;
};

type Props = {
  /** The `tool_detail` command's payload for the selected tool. */
  detail: ToolDetail;
  /** Forwards one edited field. The settings dialog owns the draft and the save. */
  onedit?: (edit: ToolDetailEdit) => void;
};

let { detail, onedit }: Props = $props();

const UNKNOWN = '—';
const uid = $props.id();

/**
 * A stored number at the library's own precision (three decimals), trimmed of the zeroes a reader
 * does not need but padded to `minDecimals` so a length keeps S10's one-decimal shape (`1.0`, `38.0`)
 * and a feed keeps its integer shape (`800`, not `800.0`).
 */
function numberText(value: number, minDecimals: number): string {
  const [whole, fraction = ''] = value.toFixed(3).split('.');
  const trimmed = fraction.replace(/0+$/, '');
  const padded = trimmed.padEnd(minDecimals, '0');
  return padded.length > 0 ? `${whole}.${padded}` : whole;
}

const lengthText = (value: number | null): string =>
  value === null ? UNKNOWN : numberText(value, 1);

const feedText = (value: number | null): string =>
  value === null ? UNKNOWN : numberText(value, 0);

const integerText = (value: number | null): string =>
  value === null ? UNKNOWN : String(value);

const percentText = (value: number | null): string => lengthText(value);

/** The title badge's `T` notation, or the dash when the library holds the tool without a number. */
const badgeText = (number: number | null): string =>
  number === null ? UNKNOWN : `T${number}`;

/** A cleared number box is an absence (`null`), never `0` or `NaN`. */
function wholeOrNull(text: string): number | null {
  const digits = text.trim();
  return /^\d+$/.test(digits) ? Number(digits) : null;
}

const root = css({ display: 'grid', gap: '16px', minW: '0' });
const heading = css({ display: 'flex', alignItems: 'center', gap: '10px' });
// The artboard's `.detail-title h4`: one step above the section captions.
const title = css({
  m: '0',
  fontSize: '16px',
  fontWeight: '600',
  color: 'textHi'
});
const badge = badgeRecipe();
// The artboard's `.form-section`: a hairline between sections, none after the last.
const section = css({
  pb: '16px',
  mb: '16px',
  borderBottom: '1px solid',
  borderColor: 'hairline',
  _last: { pb: '0', mb: '0', borderBottom: '0' }
});
const sectionTitle = css({
  m: '0 0 10px',
  textStyle: 'caption',
  fontWeight: '600',
  color: 'textOff'
});
const fields = css({
  display: 'grid',
  gridTemplateColumns: '1fr 1fr',
  gap: '10px'
});
const field = css({ display: 'grid', gap: '4px', minW: '0' });
const fieldFull = css({ gridColumn: '1 / -1' });
const fieldLabel = css({ fontSize: '11px', color: 'textDim' });
// S10's `<input>`: the artboard's `.field input`, drawn from tokens on top of the shared recipe.
const inputBox = css({
  h: '30px',
  px: '10px',
  py: '0',
  bg: 'panel',
  borderColor: 'border',
  fontSize: '12px'
});
// S10's `.ro`: a text row, not a disabled input, so its value stays readable and copyable.
const readonlyBox = css({
  h: '30px',
  px: '10px',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: '8px',
  borderRadius: 'control',
  border: '1px solid',
  borderColor: 'hairline',
  bg: 'app',
  color: 'textDim',
  fontSize: '12px',
  minW: '0'
});
const readonlyValue = css({
  minW: '0',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
});
// The artboard's `.field .u`: the unit, mono and quiet.
const unit = css({
  flex: 'none',
  fontSize: '11px',
  textStyle: 'mono',
  color: 'textOff'
});
const inputClass = cx(inputRecipe(), inputBox);
</script>

<div class={root} data-tool-library-detail>
  <div class={heading}>
    <h4 class={title}>{detail.name}</h4>
    <span class={badge} data-tool-number>{badgeText(detail.number)}</span>
  </div>

  <section class={section} aria-labelledby={`${uid}-geometry`}>
    <h5 class={sectionTitle} id={`${uid}-geometry`}>Geometry</h5>
    <div class={fields}>
      <div class={cx(field, fieldFull)} data-field="name">
        <label class={fieldLabel} for={`${uid}-name`}>Name</label>
        <input
          class={inputClass}
          id={`${uid}-name`}
          value={detail.name}
          oninput={event =>
            onedit?.({ name: event.currentTarget.value })}
        />
      </div>

      <div class={field} data-field="type">
        <span class={fieldLabel}>Type</span>
        <div class={readonlyBox}>
          <span class={readonlyValue}>{detail.kindLabel}</span>
          <span class={unit}>{UNKNOWN}</span>
        </div>
      </div>

      <div class={field} data-field="number">
        <label class={fieldLabel} for={`${uid}-number`}>Number</label>
        <input
          class={inputClass}
          id={`${uid}-number`}
          type="number"
          min="0"
          step="1"
          value={integerText(detail.number)}
          oninput={event =>
            onedit?.({ number: wholeOrNull(event.currentTarget.value) })}
        />
      </div>

      <div class={field} data-field="diameter">
        <span class={fieldLabel}>Diameter</span>
        <div class={readonlyBox}>
          <span class={readonlyValue}>{lengthText(detail.diameterMm)}</span>
          <span class={unit}>mm</span>
        </div>
      </div>

      <div class={field} data-field="flutes">
        <label class={fieldLabel} for={`${uid}-flutes`}>Flutes</label>
        <input
          class={inputClass}
          id={`${uid}-flutes`}
          type="number"
          min="0"
          step="1"
          value={integerText(detail.flutes)}
          oninput={event =>
            onedit?.({ flutes: wholeOrNull(event.currentTarget.value) })}
        />
      </div>

      <div class={field} data-field="cut-length">
        <span class={fieldLabel}>Cut length</span>
        <div class={readonlyBox}>
          <span class={readonlyValue}>{lengthText(detail.cutLengthMm)}</span>
          <span class={unit}>mm</span>
        </div>
      </div>

      <div class={field} data-field="shank">
        <span class={fieldLabel}>Shank</span>
        <div class={readonlyBox}>
          <span class={readonlyValue}>{lengthText(detail.shankMm)}</span>
          <span class={unit}>mm</span>
        </div>
      </div>

      <div class={field} data-field="overall-length">
        <span class={fieldLabel}>Overall length</span>
        <div class={readonlyBox}>
          <span class={readonlyValue}>{lengthText(detail.overallLengthMm)}</span>
          <span class={unit}>mm</span>
        </div>
      </div>
    </div>
  </section>

  <section class={section} aria-labelledby={`${uid}-feeds`}>
    <h5 class={sectionTitle} id={`${uid}-feeds`}>Feeds &amp; speeds</h5>
    <div class={fields}>
      <div class={field} data-field="spindle">
        <span class={fieldLabel}>Spindle</span>
        <div class={readonlyBox}>
          <span class={readonlyValue}>{integerText(detail.spindleRpm)}</span>
          <span class={unit}>rpm</span>
        </div>
      </div>

      <div class={field} data-field="cut-feed">
        <span class={fieldLabel}>Cut feed</span>
        <div class={readonlyBox}>
          <span class={readonlyValue}>{feedText(detail.cutFeedMmPerMin)}</span>
          <span class={unit}>mm/min</span>
        </div>
      </div>

      <div class={field} data-field="feed-plunge">
        <span class={fieldLabel}>Feed plunge</span>
        <div class={readonlyBox}>
          <span class={readonlyValue}>{feedText(detail.plungeFeedMmPerMin)}</span>
          <span class={unit}>mm/min</span>
        </div>
      </div>

      <div class={field} data-field="stepover">
        <span class={fieldLabel}>Stepover</span>
        <div class={readonlyBox}>
          <span class={readonlyValue}>{percentText(detail.stepoverPercent)}</span>
          <span class={unit}>%</span>
        </div>
      </div>
    </div>
  </section>
</div>
