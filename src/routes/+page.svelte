<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { goto } from '$app/navigation';
import { css } from 'styled-system/css';
import { StartScreen } from '$lib/components/start-screen';
import StatusBar from '$lib/components/workbench/StatusBar.svelte';
import TopBar from '$lib/components/workbench/TopBar.svelte';
import { openSettings, workbenchState } from '$lib/components/workbench/workbench.svelte';

type RecentProject = {
  name: string;
  path: string;
  lastOpened: string;
};

let error = $state('');
let recents = $state<RecentProject[]>([]);
let showNew = $state(false);

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

async function createDocument(request: {
  widthMm: number;
  heightMm: number;
  thicknessMm: number;
}) {
  error = '';
  try {
    await invoke('new_document', { request });
    await goto('/workspace');
  } catch (e) {
    error = formatError(e);
  }
}

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
workbenchState.statusMessage = 'Listo';
workbenchState.statusKind = 'ok';

const shell = css({
  w: '100%',
  h: '100vh',
  position: 'relative',
  overflow: 'hidden',
  bg: 'canvas',
  color: 'text',
  fontSize: '13px',
  lineHeight: '1.4',
  fontFamily: 'sans'
});
const content = css({
  position: 'absolute',
  top: '48px',
  left: '0',
  right: '0',
  bottom: '34px',
  overflow: 'auto'
});
</script>

<div class={shell}>
  <TopBar
    projectName="Camcraft"
    onNew={() => (showNew = true)}
    onOpen={openDocument}
    onSettings={openSettings}
  />
  <div class={content}>
    <StartScreen
      {recents}
      {error}
      bind:showNew
      onCreate={createDocument}
      onOpen={openDocument}
      onOpenRecent={openRecent}
      onSettings={openSettings}
    />
  </div>
  <StatusBar />
</div>
