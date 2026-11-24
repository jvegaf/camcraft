export type ViewportSelection = {
  kind: 'triangle';
  index: number;
  avgDepthMm: number;
} | null;

export type WorkbenchState = {
  selection: ViewportSelection;
  statusMessage: string;
};

export const workbenchState: WorkbenchState = $state({
  selection: null,
  statusMessage: 'Listo'
});

export function setSelection(selection: ViewportSelection) {
  workbenchState.selection = selection;
}

export function setStatusMessage(message: string) {
  workbenchState.statusMessage = message;
}
