<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { css } from 'styled-system/css';
import { onMount } from 'svelte';
import ToolLibraryDetail from './ToolLibraryDetail.svelte';
import ToolLibraryPage from './ToolLibraryPage.svelte';
import ToolLibraryPreview from './ToolLibraryPreview.svelte';
import ToolLibraryTree from './ToolLibraryTree.svelte';
import type { ToolDetail } from './toolLibrary';

// Wiring organism (107 S9, S10, S11, S13 + 096 draft semantics): the only place the tool library
// page talks to the backend. It loads the tree through `tool_library_tree` on mount, keeps the
// selected id, loads that tool through `tool_detail`, and composes the three visual organisms into
// `ToolLibraryPage`'s snippet props. Those organisms stay prop-only; no command leaks into them.
//
// The three editable fields are a draft here, never written on edit. `save()` is called by the
// settings dialog from its existing Apply cycle (the same cycle the theme row uses), and only then
// does `tool_library_save` run. The payload is exactly `ToolSaveRequest`: `id`, `name`, `number`,
// `flutes`; the backend denies unknown keys, so the panel cannot offer a read-only field as
// writable. After a successful write the detail is read again, so the form shows the store's record
// rather than the typed draft.
//
// Command failures travel through `onerror` to the dialog's existing error line (assertive, in the
// footer, exactly like a failed theme save). A failed save returns `false`, which tells the dialog
// not to close or report success.
//
// Two reads feed the page and neither of them may pretend: the tree read distinguishes "loaded and
// empty" from "failed to load", so a library nobody could read is never announced as an empty one;
// and `save()` separates the write from the reads that follow it, because a committed write is a
// successful save even when its confirmation read fails.
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

type ToolTree = { schema: string; groups: ToolGroup[] };

/** Exactly the fields `ToolSaveRequest` accepts; no other key may reach the command. */
type ToolSaveRequest = {
  id: string;
  name: string;
  number: number | null;
  flutes: number | null;
};

type ToolDetailEdit = {
  name?: string;
  number?: number | null;
  flutes?: number | null;
};

type Props = {
  /** Reports a command failure to the settings dialog's existing error line. */
  onerror?: (message: string) => void;
};

let { onerror }: Props = $props();

let groups = $state<ToolGroup[]>([]);
// `treeLoaded` is true only after a read that answered. A failed read leaves it false and raises
// `treeFailed`, so the Tools column shows a failure instead of the empty state: an unknown library is
// not an empty one. A failed refresh of a library already loaded keeps the rows it already has.
let treeLoaded = $state(false);
let treeFailed = $state(false);
let selectedId = $state<string | null>(null);
let detail = $state<ToolDetail | null>(null);
let draft = $state<ToolDetailEdit>({});
// Orders detail reads of the tool that is currently selected. The invariant is which tool is
// selected — decided by `selectedId` — not how many loads happened: reselecting the same row is a
// new read of the same tool, so it must not invalidate an authoritative read of that same tool.
let readToken = 0;

/**
 * The one name a brand-new tool starts with. `ToolCreateRequest` accepts a name and nothing else —
 * the store decides the id, the number, the shape and the flutes — and no text prompt exists in this
 * dialog, so the page sends a neutral placeholder and selects the new tool, which puts the operator
 * in the `Name` field of the detail form rather than leaving the tool named by the machine.
 */
const NEW_TOOL_NAME = 'New tool';

/** The detail as the form shows it: the stored record with the unconfirmed draft applied. */
const shownDetail = $derived.by((): ToolDetail | null =>
  detail === null ? null : { ...detail, ...draft }
);

/**
 * The `Valid` badge's truth: the library publishes no validity flag, so the only rule the page can
 * own is the one the form can break — a selected tool whose drafted name is not blank. With no
 * selection there is nothing to vouch for, and the badge is not rendered at all.
 */
const valid = $derived(
  detail !== null && (draft.name ?? detail.name).trim() !== ''
);

function formatError(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e && typeof e === 'object' && 'message' in e) {
    const err = e as { code?: string; message: string };
    return err.code ? `${err.code}: ${err.message}` : err.message;
  }
  return String(e);
}

async function loadTree() {
  try {
    const tree = await invoke<ToolTree>('tool_library_tree');
    groups = tree.groups;
    treeLoaded = true;
    treeFailed = false;
  } catch (e) {
    // Never swallowed: the failure reaches the dialog's error line and, when nothing has ever been
    // read, the column state below. A refresh that fails after a successful read keeps the rows it
    // already has, because those rows are real data rather than a claim about an empty library.
    treeFailed = true;
    onerror?.(formatError(e));
  }
}

async function loadDetail(id: string) {
  const token = ++readToken;
  try {
    const loaded = await invoke<ToolDetail | null>('tool_detail', { id });
    if (token !== readToken || selectedId !== id) return;
    detail = loaded;
    draft = {};
  } catch (e) {
    if (token !== readToken || selectedId !== id) return;
    detail = null;
    draft = {};
    onerror?.(formatError(e));
  }
}

function selectTool(id: string) {
  selectedId = id;
  void loadDetail(id);
}

/**
 * `+ New`: run the library's create command and make the new tool the selection. The result is the
 * store's own record, so it is shown as-is; the tree is read again so the new row appears in its
 * group with the number the store gave it.
 */
async function createTool() {
  try {
    const created = await invoke<ToolDetail>('tool_library_create', {
      request: { name: NEW_TOOL_NAME }
    });
    // Invalidate any detail read still in flight: it belongs to the previous selection.
    readToken += 1;
    selectedId = created.id;
    detail = created;
    draft = {};
    await loadTree();
  } catch (e) {
    onerror?.(formatError(e));
  }
}

/** Every edit stays in the draft; nothing here calls a command. */
function editTool(edit: ToolDetailEdit) {
  draft = { ...draft, ...edit };
}

const request = (): ToolSaveRequest | null => {
  if (detail === null) return null;
  return {
    id: detail.id,
    name: draft.name ?? detail.name,
    number: draft.number !== undefined ? draft.number : detail.number,
    flutes: draft.flutes !== undefined ? draft.flutes : detail.flutes
  };
};

/**
 * Write the draft, called by the settings dialog when the operator confirms. Returns whether the
 * write succeeded, so the dialog only closes on a real save; the error itself already reached the
 * dialog's error line.
 *
 * The write and the reads that follow it are separate outcomes. Once `tool_library_save` has
 * answered, the library has changed and the save has succeeded: a failing confirmation read must not
 * turn it into a failure that leaves the draft in place for a retry, because that retry would write
 * again what already landed.
 */
export async function save(): Promise<boolean> {
  const body = request();
  if (body === null) return true;

  let written: ToolDetail;
  try {
    written = await invoke<ToolDetail>('tool_library_save', { request: body });
  } catch (e) {
    onerror?.(formatError(e));
    return false;
  }

  // The draft was written, so the form must not keep offering it. Guarded by the selection: if the
  // operator picked another tool while the write was in flight, that tool's draft is not this one.
  if (selectedId === body.id) draft = {};

  // The store is the authority: read the record back instead of trusting the typed draft.
  const token = ++readToken;
  try {
    const stored = await invoke<ToolDetail | null>('tool_detail', {
      id: body.id
    });
    // The guard is the selection, not the read count: a reselect of the same tool during the write
    // or its re-read does not make this authoritative read stale, while a response for a tool that is
    // no longer selected is discarded. The bump cancels any same-tool read issued before this one,
    // so a pre-write snapshot cannot paint over the record the store now holds.
    if (selectedId === body.id) {
      readToken += 1;
      detail = stored;
      draft = {};
    }
  } catch (e) {
    // The write landed but the re-read did not. The form shows the record the write answered with,
    // which is what the store now holds, rather than the pre-write snapshot; the failed read is
    // reported on the dialog's error line and the save still reports success.
    if (selectedId === body.id) {
      readToken += 1;
      detail = written;
    }
    onerror?.(formatError(e));
  }

  // The tree is a separate read: refresh it so an edited name or number reaches the left column now
  // instead of on the next remount. It is not rebuilt from the detail payload, so the tree stays the
  // store's projection and keeps its own grouping and meta. A failure here is reported by
  // `loadTree` itself and does not undo the write.
  await loadTree();
  return true;
}

onMount(() => {
  void loadTree();
});

const emptyState = css({
  m: '0',
  px: '8px',
  py: '10px',
  textStyle: 'small',
  color: 'textDim'
});
</script>

<ToolLibraryPage oncreate={createTool} {valid}>
  {#snippet tools()}
    {#if !treeLoaded && treeFailed}
      <p class={emptyState}>The tool library could not be read.</p>
    {:else if !treeLoaded}
      <p class={emptyState}>Loading tools…</p>
    {:else if groups.length === 0}
      <p class={emptyState}>No tools in the library yet.</p>
    {:else}
      <ToolLibraryTree
        {groups}
        {selectedId}
        onselect={selectTool}
      />
    {/if}
  {/snippet}

  {#snippet detail()}
    {#if shownDetail}
      <ToolLibraryDetail detail={shownDetail} onedit={editTool} />
    {/if}
  {/snippet}

  {#snippet preview()}
    {#if shownDetail}
      <ToolLibraryPreview detail={shownDetail} />
    {/if}
  {/snippet}
</ToolLibraryPage>
