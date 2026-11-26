// Organisms (095 T3 + T4): the chrome and CAM-content blocks of the floating studio. Each one owns
// its own local UI state or reads the workbench store, composes atoms/molecules, and keeps the props
// it had in the hand-rolled `workbench/` folder so pages and routes keep working (S8).
export { default as CamObjectBrowser } from './CamObjectBrowser.svelte';
export { default as OperationInspector } from './OperationInspector.svelte';
export { default as StatusBar } from './StatusBar.svelte';
export { default as ToolRail } from './ToolRail.svelte';
export { default as TopBar } from './TopBar.svelte';
export { default as ViewportHud } from './ViewportHud.svelte';
