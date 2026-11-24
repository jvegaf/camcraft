<script lang="ts">
import { onMount } from 'svelte';
import { goto } from '$app/navigation';
import { openSettings } from '$lib/components/workbench/workbench.svelte';

onMount(() => {
  openSettings();
  void goto('/', { replaceState: true });
});
</script>
