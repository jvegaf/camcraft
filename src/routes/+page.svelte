<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { goto } from '$app/navigation';

type RecentProject = {
  name: string;
  path: string;
  lastOpened: string;
};

// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
let error = $state('');
// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
let recents = $state<RecentProject[]>([]);
let widthMm = $state(1219);
let heightMm = $state(2438);
let thicknessMm = $state(18);

function formatError(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e && typeof e === 'object' && 'message' in e) {
    const err = e as { code?: string; message: string };
    return err.code ? `${err.code}: ${err.message}` : err.message;
  }
  return String(e);
}

async function refreshRecents() {
  try {
    recents = await invoke<RecentProject[]>('list_recents');
  } catch {
    // non-fatal
  }
}

// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
async function createDocument(event: Event) {
  event.preventDefault();
  error = '';
  try {
    await invoke('new_document', {
      request: {
        widthMm: Number(widthMm),
        heightMm: Number(heightMm),
        thicknessMm: Number(thicknessMm)
      }
    });
    await goto('/workspace');
  } catch (e) {
    error = formatError(e);
  }
}

// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
async function openDocument() {
  error = '';
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Camcraft document', extensions: ['cmc', 'json'] }]
    });
    if (typeof selected !== 'string') return;
    await invoke('open_document', { path: selected });
    await goto('/workspace');
  } catch (e) {
    error = formatError(e);
  }
}

// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
async function openRecent(path: string) {
  error = '';
  try {
    await invoke('open_document', { path });
    await goto('/workspace');
  } catch (e) {
    error = formatError(e);
  }
}

void refreshRecents();
</script>

<main class="shell">
  <section class="start">
    <aside class="brand">
      <p class="eyebrow">Camcraft</p>
      <h1>Start</h1>
      <p class="lead">Open a recent document or create a new board.</p>

      <div class="actions">
        <button type="button" class="primary" onclick={openDocument}>Open document</button>
      </div>

      {#if error}
        <p class="error">{error}</p>
      {/if}

      <form class="new-doc" onsubmit={createDocument}>
        <p class="label">New document</p>
        <div class="fields">
          <label>Width (mm)<input type="number" step="any" bind:value={widthMm} /></label>
          <label>Height (mm)<input type="number" step="any" bind:value={heightMm} /></label>
          <label>Thickness (mm)<input type="number" step="any" bind:value={thicknessMm} /></label>
        </div>
        <button type="submit" class="primary">Create document</button>
      </form>
    </aside>

    <section class="recents">
      <div class="recents-head">
        <h2>Recent</h2>
        <span class="count">{recents.length}</span>
      </div>
      {#if recents.length === 0}
        <p class="muted">No recent documents yet.</p>
      {:else}
        <ul class="recent-list">
          {#each recents as recent (recent.path)}
            <li>
              <button type="button" class="recent-item" onclick={() => openRecent(recent.path)}>
                <span class="recent-name">{recent.name}</span>
                <span class="recent-path">{recent.path}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  </section>
</main>

<style>
  :global(body) {
    margin: 0;
    background: #eef1f4;
    color: #1f2430;
    font-family: Inter, system-ui, sans-serif;
  }
  .shell { min-height: 100vh; display: grid; place-items: center; padding: 32px; box-sizing: border-box; }
  .start {
    width: min(1040px, 100%);
    min-height: 640px;
    display: grid;
    grid-template-columns: 360px 1fr;
    background: #f7f8fa;
    border: 1px solid #d7dde5;
    border-radius: 16px;
    overflow: hidden;
    box-shadow: 0 20px 60px rgba(20, 28, 40, 0.08);
  }
  .brand { padding: 32px; border-right: 1px solid #d7dde5; background: #fbfcfd; }
  .eyebrow { margin: 0; font-size: 12px; letter-spacing: 0.08em; text-transform: uppercase; color: #667085; }
  h1 { margin: 8px 0 0; font-size: 40px; line-height: 1; }
  .lead { margin: 12px 0 24px; color: #5f6b7a; max-width: 28ch; }
  .actions { display: flex; gap: 8px; flex-wrap: wrap; margin-bottom: 24px; }
  button { font: inherit; font-size: 14px; padding: 9px 14px; border-radius: 8px; border: 1px solid #cfd6df; background: #fff; color: #1f2430; cursor: pointer; }
  button.primary { background: #1f2430; color: #fff; border-color: #1f2430; }
  .error { color: #b42318; font-size: 14px; margin: 0 0 16px; }
  .label { margin: 0 0 8px; font-size: 12px; letter-spacing: 0.04em; text-transform: uppercase; color: #667085; }
  .new-doc { display: grid; gap: 12px; padding-top: 20px; border-top: 1px solid #e3e8ee; }
  .fields { display: grid; gap: 10px; }
  label { display: grid; gap: 4px; font-size: 13px; color: #344054; }
  input { font: inherit; padding: 8px 10px; border-radius: 8px; border: 1px solid #cfd6df; background: #fff; }
  .recents { padding: 32px; }
  .recents-head { display: flex; align-items: center; gap: 10px; margin-bottom: 16px; }
  h2 { margin: 0; font-size: 20px; }
  .count { font-size: 12px; color: #667085; background: #e9eef4; padding: 2px 8px; border-radius: 999px; }
  .muted { color: #667085; }
  .recent-list { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; }
  .recent-item { width: 100%; text-align: left; padding: 12px 14px; background: #fff; border: 1px solid #dfe5ec; border-radius: 10px; }
  .recent-item:hover { border-color: #b8c4d3; background: #fcfdff; }
  .recent-name { display: block; font-size: 14px; font-weight: 600; color: #1f2430; }
  .recent-path { display: block; margin-top: 4px; font-size: 12px; color: #667085; word-break: break-all; }
  @media (max-width: 900px) {
    .start { grid-template-columns: 1fr; min-height: auto; }
    .brand { border-right: 0; border-bottom: 1px solid #d7dde5; }
  }
</style>
