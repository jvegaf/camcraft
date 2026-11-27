<script lang="ts">
import WorkbenchPage from '../workbench/+page.svelte';

// `/workspace` renders the workbench screen (101 T2, L4). Until 101 this route mounted the
// `WorkbenchShell` component through the `workbench/` barrel; the shell is now the body of
// `src/routes/workbench/+page.svelte`, so the route mounts that page instead of a duplicated copy of
// its handlers. The label it used to pass (`document.cam`) is the page's own default, so the
// rendered result is identical.
</script>

<WorkbenchPage />
