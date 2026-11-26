import { beforeEach, describe, expect, it } from 'vitest';
import type { CamOperation } from './workbench.svelte';
import {
  selectBoardObject,
  selectOperation,
  setOperations,
  setPointerPosition,
  setStatus,
  setStatusMessage,
  toggleSection,
  workbenchState
} from './workbench.svelte';

// Vitest runs with `passWithNoTests: false`, so this file is also the proof that the frontend gate has
// something to run: an empty suite is a gate that says "green" without having looked at anything.
describe('workbench status line', () => {
  beforeEach(() => {
    setStatus('Listo', 'ok');
  });

  it('reads a message that starts with "error" as an error', () => {
    setStatusMessage('Error: no project open');

    expect(workbenchState.statusMessage).toBe('Error: no project open');
    expect(workbenchState.statusKind).toBe('error');
  });

  it('leaves an ordinary message as info', () => {
    setStatusMessage('Listo');

    expect(workbenchState.statusKind).toBe('info');
  });
});

// The status bar coordinates (095 T3) come from the canvas pointer, so the store needs a real place
// to keep the point the renderer publishes and to clear it when the pointer leaves the canvas.
describe('workbench pointer position', () => {
  it('starts unknown and keeps the point the canvas publishes', () => {
    setPointerPosition(null);
    expect(workbenchState.pointer).toBeNull();

    setPointerPosition({ x: 42.8, y: 31.2, z: 0 });
    expect(workbenchState.pointer).toEqual({ x: 42.8, y: 31.2, z: 0 });

    setPointerPosition(null);
    expect(workbenchState.pointer).toBeNull();
  });
});

// The Browser and the Inspector need presentation state that belongs to the workbench store (095
// T4): which sections are collapsed, which board object and which operation are selected. Nothing
// here duplicates document data; it is only what the operators picked in the panels.
const operation: CamOperation = {
  id: 'op-1',
  index: 1,
  name: 'Profile outline',
  type: null,
  tool: null,
  zCutMm: null,
  plungeFeedMmPerMin: null,
  hitCount: null,
  tools: null,
  durationS: null
};

describe('workbench CAM object presentation state', () => {
  beforeEach(() => {
    workbenchState.operations = [];
    workbenchState.selectedOperationId = null;
    workbenchState.selectedBoardObjectId = null;
    workbenchState.collapsedSections = [];
  });

  it('toggles a browser section open and closed', () => {
    toggleSection('board');
    expect(workbenchState.collapsedSections).toEqual(['board']);

    toggleSection('setup');
    expect(workbenchState.collapsedSections).toEqual(['board', 'setup']);

    toggleSection('board');
    expect(workbenchState.collapsedSections).toEqual(['setup']);
  });

  it('keeps the selected board object and operation', () => {
    selectBoardObject('size');
    selectOperation('op-1');

    expect(workbenchState.selectedBoardObjectId).toBe('size');
    expect(workbenchState.selectedOperationId).toBe('op-1');

    selectBoardObject(null);
    selectOperation(null);
    expect(workbenchState.selectedBoardObjectId).toBeNull();
    expect(workbenchState.selectedOperationId).toBeNull();
  });

  it('drops an operation selection the new operation list no longer has', () => {
    setOperations([operation]);
    selectOperation('op-1');
    expect(workbenchState.selectedOperationId).toBe('op-1');

    setOperations([]);
    expect(workbenchState.selectedOperationId).toBeNull();
  });
});
