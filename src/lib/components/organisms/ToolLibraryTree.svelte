<script lang="ts">
import { css, cx } from 'styled-system/css';
import { panelRow as panelRowRecipe } from 'styled-system/recipes';

// Organism (107 T5, S9 + S13) — the `Tools` column of
// `design/camcraft-workbench-v2/05-settings-tool-library.html` (`.tree-group`/`.tool-row`): a group
// per tool kind with its count, and one selectable row per tool carrying the tool number, its name
// and a kind-dependent meta.
//
// Props, not stores: the caller hands over the `tool_library_tree` payload's `groups` and the
// selected id, and receives the chosen id through `onselect`. No command is called from here, so this
// organism holds no library state and cannot drift from the page's.
//
// The payload's `label` is the backend's singular word (`Flat end mill`, `Drill`, `V-bit` —
// `ToolKind::label`, `src-tauri/crates/cam-store/src/tools.rs:231`), while S9's headings are plural
// page copy (`End mills`, `Drills`, `V-bits`). Pluralising therefore happens here, as a **closed table keyed by the kind's
// stable slug** and not as `label + 's'`: English plurals are not a suffix rule (S9's own flat-end
// mill group is `End mills`, which no mechanical rule derives from `Flat end mill`), and a slug the
// table does not know keeps the backend's singular label instead of inventing a plural for it.
//
// A tool the library holds without a number arrives as `number: null` and `meta: "—"`; the row shows
// that dash in the number chip, never `T0`, `Tnull` or an empty cell.
type ToolRow = {
  id: string;
  number: number | null;
  name: string;
  meta: string;
};

type ToolGroup = {
  kind: string;
  label: string;
  count: number;
  tools: ToolRow[];
};

type Props = {
  /** The `tool_library_tree` payload's `groups`, as the command publishes them. */
  groups: ToolGroup[];
  /** The tool the page has selected; its row is the one marked `aria-current`. */
  selectedId?: string | null;
  /** Forwards the id of the row the operator picked. The page owns the Tauri call. */
  onselect?: (id: string) => void;
};

let { groups, selectedId = null, onselect }: Props = $props();

const GROUP_HEADINGS: Record<string, string> = {
  'flat-end-mill': 'End mills',
  'ball-nose': 'Ball-nose mills',
  'corner-radius': 'Corner-radius mills',
  'v-bit': 'V-bits',
  engraving: 'Engraving bits',
  drill: 'Drills',
  'isolation-mill': 'Isolation mills'
};

/** The group heading: the table's plural, or the backend's own word for a kind it does not name. */
const headingOf = (group: ToolGroup): string =>
  GROUP_HEADINGS[group.kind] ?? group.label;

const uid = $props.id();
const headingId = (kind: string) => `${uid}-${kind}`;

const tree = css({ display: 'grid', gap: '10px' });
const group = css({ minW: '0' });
// The artboard's `.tree-group h5`: caption shape, the count pushed to the right.
const groupHead = css({
  display: 'flex',
  alignItems: 'center',
  gap: '8px',
  m: '0',
  mb: '4px',
  px: '8px',
  py: '4px',
  textStyle: 'caption',
  color: 'textOff'
});
const groupTitle = css({
  minW: '0',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
});
const groupCount = css({
  flex: 'none',
  ml: 'auto',
  textStyle: 'mono',
  color: 'textOff'
});
const list = css({
  display: 'grid',
  gap: '1px',
  m: '0',
  p: '0',
  listStyle: 'none'
});
// The artboard's `.tool-row .n`: the mono chip holding `T1`.
const numberChip = css({
  flex: 'none',
  w: '22px',
  h: '22px',
  display: 'grid',
  placeItems: 'center',
  borderRadius: 'control',
  border: '1px solid',
  borderColor: 'hairline',
  bg: 'panel',
  color: 'textDim',
  textStyle: 'mono',
  fontSize: '10px'
});
const numberChipSelected = css({
  color: 'sel',
  borderColor: 'sel/25'
});
const numberText = (number: number | null) =>
  number === null ? '—' : `T${number}`;
</script>

<div class={tree} data-tool-library-tree>
  {#each groups as group (group.kind)}
    <section class={group} aria-labelledby={headingId(group.kind)}>
      <h5 class={groupHead} id={headingId(group.kind)}>
        <span class={groupTitle}>{headingOf(group)}</span>
        <span class={groupCount}>{group.count}</span>
      </h5>
      <ul class={list}>
        {#each group.tools as tool (tool.id)}
          {@const selected = tool.id === selectedId}
          {@const rowStyles = panelRowRecipe({ interactive: true, selected })}
          <li>
            <button
              type="button"
              class={rowStyles.root}
              aria-current={selected ? 'true' : undefined}
              onclick={() => onselect?.(tool.id)}
            >
              <span class={cx(numberChip, selected && numberChipSelected)}>
                {numberText(tool.number)}
              </span>
              <span class={rowStyles.label}>{tool.name}</span>
              <span class={rowStyles.meta}>{tool.meta}</span>
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {/each}
</div>
