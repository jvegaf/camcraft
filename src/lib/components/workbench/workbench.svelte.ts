export type ViewportSelection = {
  kind: 'triangle';
  index: number;
  avgDepthMm: number;
} | null;

export type DocumentStatus = {
  schema: string;
  documentSchema: string;
  dirty: boolean;
  path: string | null;
  layerCount: number;
  toolpathCount: number;
  boardWidthMm: number;
  boardHeightMm: number;
  boardThicknessMm: number;
};

export type WorkbenchState = {
  selection: ViewportSelection;
  statusMessage: string;
  document: DocumentStatus | null;
};

export const workbenchState: WorkbenchState = $state({
  selection: null,
  statusMessage: 'Listo',
  document: null
});

export function setSelection(selection: ViewportSelection) {
  workbenchState.selection = selection;
}

export function setStatusMessage(message: string) {
  workbenchState.statusMessage = message;
}

export function setDocument(document: DocumentStatus | null) {
  workbenchState.document = document;
}

export async function refreshDocumentStatus(): Promise<void> {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const status = await invoke<DocumentStatus>('document_status');
    workbenchState.document = status;
    workbenchState.statusMessage = status.dirty ? 'Cambios sin guardar' : 'Listo';
  } catch (e) {
    workbenchState.statusMessage = `Error: ${String(e)}`;
  }
}
