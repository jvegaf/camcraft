import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import StatusDot from './StatusDot.svelte';

// Atom contract: a tonal dot. It never carries meaning on its own (decorative by default), so a
// caller that relies on the tone must pass a label.
describe('StatusDot', () => {
  it('renders a decorative dot on the neutral tone by default', () => {
    render(StatusDot, {});

    const dot = document.querySelector('[data-tone]');
    expect(dot).not.toBeNull();
    expect(dot?.getAttribute('data-tone')).toBe('neutral');
    expect(dot?.getAttribute('aria-hidden')).toBe('true');
    expect(dot?.getAttribute('class')).toContain('statusDot--tone_neutral');
  });

  it('renders the ok, selected and draft tones from the artboard rows', () => {
    for (const tone of ['ok', 'selected', 'draft'] as const) {
      const { unmount } = render(StatusDot, { tone });

      const dot = document.querySelector('[data-tone]');
      expect(dot?.getAttribute('data-tone')).toBe(tone);
      expect(dot?.getAttribute('class')).toContain(`statusDot--tone_${tone}`);

      unmount();
    }
  });

  it('becomes an accessible dot when a label is provided', () => {
    render(StatusDot, { tone: 'ok', label: 'Sin errores' });

    const dot = screen.getByRole('img', { name: 'Sin errores' });
    expect(dot.getAttribute('aria-hidden')).toBeNull();
  });
});
