import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import KeyHint from './KeyHint.svelte';

// Atom contract: the keyboard chip. The artboard chrome says exactly "⌘K".
describe('KeyHint', () => {
  it('renders the command palette hint from the artboard', () => {
    render(KeyHint, {});

    const hint = screen.getByText('⌘K');
    expect(hint.tagName).toBe('KBD');
    expect(hint.getAttribute('class')).toContain('keyHint');
  });

  it('renders another shortcut when the caller overrides the keys', () => {
    render(KeyHint, { keys: '⌘S' });

    expect(screen.getByText('⌘S').tagName).toBe('KBD');
  });
});
