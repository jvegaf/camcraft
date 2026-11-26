import { beforeEach, describe, expect, it } from 'vitest';
import {
  setPointerPosition,
  setStatus,
  setStatusMessage,
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
