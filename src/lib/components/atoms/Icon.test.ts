import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Icon from './Icon.svelte';

// Atom contract: an inline svg drawn in currentColor, decorative by default, with an accessible
// name only when the caller asks for one.
describe('Icon', () => {
  it('renders a decorative inline svg that inherits the current text color', () => {
    render(Icon, { name: 'search' });

    const svg = document.querySelector('svg');
    expect(svg).not.toBeNull();
    expect(svg?.getAttribute('viewBox')).toBe('0 0 24 24');
    expect(svg?.getAttribute('data-icon')).toBe('search');
    expect(svg?.getAttribute('aria-hidden')).toBe('true');
    expect(svg?.getAttribute('stroke')).toBe('currentColor');
    expect(svg?.getAttribute('fill')).toBe('none');
    expect(svg?.querySelectorAll('path, circle, rect').length).toBeGreaterThan(
      0
    );
    expect(svg?.getAttribute('class')).toContain('icon');
  });

  it('exposes an accessible name when a label is provided', () => {
    render(Icon, { name: 'origin', label: 'Origen' });

    const svg = screen.getByRole('img', { name: 'Origen' });
    expect(svg.getAttribute('aria-hidden')).toBeNull();
  });

  it('applies the size variant class', () => {
    render(Icon, { name: 'plus', size: 'sm' });

    expect(document.querySelector('svg')?.getAttribute('class')).toContain(
      'icon--size_sm'
    );
  });
});
