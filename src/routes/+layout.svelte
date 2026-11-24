<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { onMount } from 'svelte';
import '../app.css';
import SettingsDialog from '$lib/components/settings/SettingsDialog.svelte';

let { children } = $props();

type Settings = { theme: string };

function applyTheme(theme: string) {
  const root = document.documentElement;
  if (theme === 'light' || theme === 'dark') {
    root.dataset.theme = theme;
  } else {
    root.removeAttribute('data-theme');
  }
}

onMount(async () => {
  try {
    const settings = await invoke<Settings>('get_settings');
    applyTheme(settings.theme);
  } catch {
    // non-fatal: default theme
  }
});
</script>

{@render children()}
<SettingsDialog />
