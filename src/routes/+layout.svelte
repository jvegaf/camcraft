<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { onMount } from 'svelte';
import '../app.css';
import SettingsDialog from '$lib/components/settings/SettingsDialog.svelte';
import { applyTheme } from '$lib/ds/theme';

let { children } = $props();

type Settings = { theme: string };

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
