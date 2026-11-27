import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import Button from './Button.svelte';

const label = (text: string) =>
  createRawSnippet(() => ({ render: () => `<span>${text}</span>` }));

// Atom contract: labelled button with the design-system variants; "Generar plan" is the primary
// accent the artboard uses for the CAM action.
describe('Button', () => {
  it('renders its label as the accessible name with the primary variant class', () => {
    render(Button, { variant: 'primary', children: label('Generate plan') });

    const button = screen.getByRole('button', { name: 'Generate plan' });
    expect(button.getAttribute('type')).toBe('button');
    expect(button.getAttribute('class')).toContain(
      'atomButton--variant_primary'
    );
  });

  it('defaults to the secondary variant at medium size', () => {
    render(Button, { children: label('Import') });

    const classes = screen
      .getByRole('button', { name: 'Import' })
      .getAttribute('class');
    expect(classes).toContain('atomButton--variant_secondary');
    expect(classes).toContain('atomButton--size_md');
  });

  it('applies the ghost and small variants when asked', () => {
    render(Button, {
      variant: 'ghost',
      size: 'sm',
      children: label('Layers')
    });

    const classes = screen
      .getByRole('button', { name: 'Layers' })
      .getAttribute('class');
    expect(classes).toContain('atomButton--variant_ghost');
    expect(classes).toContain('atomButton--size_sm');
  });

  it('emits one click per press', async () => {
    const user = userEvent.setup();
    const onclick = vi.fn();
    render(Button, { onclick, children: label('Generate plan') });

    await user.click(screen.getByRole('button', { name: 'Generate plan' }));

    expect(onclick).toHaveBeenCalledTimes(1);
  });

  it('does not emit when disabled', async () => {
    const user = userEvent.setup();
    const onclick = vi.fn();
    render(Button, {
      disabled: true,
      onclick,
      children: label('Generate plan')
    });

    const button = screen.getByRole('button', { name: 'Generate plan' });
    expect(button.hasAttribute('disabled')).toBe(true);

    await user.click(button);
    expect(onclick).not.toHaveBeenCalled();
  });
});
