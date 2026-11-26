// Entry point of the workbench folder: `WorkbenchShell` and `BoardCanvas` live here and are
// re-exported for the `/workbench` and `/workspace` routes.
//
// The chrome and CAM-content organisms live in the atomic folder `src/lib/components/organisms/`
// and the re-exports below keep the older `$lib/components/workbench` imports working (S8). They
// are a convenience, not the only path: `WorkbenchShell` imports the organisms from
// `$lib/components/organisms` and the start page does the same, so a consumer may import an
// organism from either place.
export {
  CamObjectBrowser,
  OperationInspector,
  StatusBar,
  ToolRail,
  TopBar,
  ViewportHud
} from '../organisms';
export { default as BoardCanvas } from './BoardCanvas.svelte';
export { default as WorkbenchShell } from './WorkbenchShell.svelte';
