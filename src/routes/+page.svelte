<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { goto } from '$app/navigation';
import { StartScreen } from '$lib/components/start-screen';

type RecentProject = {
  name: string;
  path: string;
  lastOpened: string;
};

let error = $state('');
let recents = $state<RecentProject[]>([]);

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
</script>

<StartScreen
  {recents}
  {error}
  onCreate={createDocument}
  onOpen={openDocument}
  onOpenRecent={openRecent}
  onSettings={() => goto('/settings')}
/>
