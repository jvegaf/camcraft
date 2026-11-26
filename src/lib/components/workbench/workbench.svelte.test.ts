import { beforeEach, describe, expect, it } from 'vitest';
import {
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
