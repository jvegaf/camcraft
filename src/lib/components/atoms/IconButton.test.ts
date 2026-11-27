import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import IconButton from './IconButton.svelte';

// Atom contract: a square button with a mandatory accessible name, a decorative icon inside, a
// visible focus target and the shadowless hover/focus ring from the design system.
describe('IconButton', () => {
  it('is a button whose accessible name is the required aria-label', () => {
    render(IconButton, { ariaLabel: 'Theme', icon: 'theme' });

    const button = screen.getByRole('button', { name: 'Theme' });
    expect(button.getAttribute('type')).toBe('button');
    expect(button.getAttribute('class')).toContain('iconButton');
  });

  it('draws the icon as decoration, not as a second accessible name', () => {
    render(IconButton, { ariaLabel: 'Settings', icon: 'settings' });

    const button = screen.getByRole('button', { name: 'Settings' });
    const svg = button.querySelector('svg');
    expect(svg).not.toBeNull();
    expect(svg?.getAttribute('aria-hidden')).toBe('true');
  });

  it('can take keyboard focus, so the focus-visible ring is reachable', () => {
    render(IconButton, { ariaLabel: 'Theme', icon: 'theme' });

    const button = screen.getByRole('button', { name: 'Theme' });
    button.focus();
    expect(document.activeElement).toBe(button);
  });

  it('calls onclick once per press', async () => {
    const user = userEvent.setup();
    const onclick = vi.fn();
    render(IconButton, { ariaLabel: 'Theme', icon: 'theme', onclick });

    await user.click(screen.getByRole('button', { name: 'Theme' }));

    expect(onclick).toHaveBeenCalledTimes(1);
  });

  it('applies the size variant class and disables the control', async () => {
    const user = userEvent.setup();
    const onclick = vi.fn();
    render(IconButton, {
      ariaLabel: 'Search in browser',
      icon: 'search',
      size: 'sm',
      disabled: true,
      onclick
    });

    const button = screen.getByRole('button', { name: 'Search in browser' });
    expect(button.getAttribute('class')).toContain('iconButton--size_sm');
    expect(button.hasAttribute('disabled')).toBe(true);

    await user.click(button);
    expect(onclick).not.toHaveBeenCalled();
  });
});
