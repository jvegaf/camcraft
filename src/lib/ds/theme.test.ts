import { afterEach, describe, expect, it } from 'vitest';
import { applyTheme, readTheme, toggleTheme } from './theme';

// Shared mechanism contract (095 T3): the theme lives on `document.documentElement[data-theme]`.
// `SettingsDialog` used to own this function; the TopBar theme action needs the same rule, so the
// rule moved here and both call it. The three cases below are exactly the ones the dialog handled:
// an explicit theme is written, anything else ("system") removes the attribute and falls back to the
// default dark palette of `design/system.md`.
const reset = () => document.documentElement.removeAttribute('data-theme');

describe('shared theme mechanism', () => {
  afterEach(reset);

  it('writes the explicit themes SettingsDialog writes', () => {
    applyTheme('light');
    expect(document.documentElement.dataset.theme).toBe('light');

    applyTheme('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('removes the attribute for the non-explicit "system" preference', () => {
    applyTheme('light');
    applyTheme('system');

    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it('reads dark as the default and flips between dark and light', () => {
    expect(readTheme()).toBe('dark');

    expect(toggleTheme()).toBe('light');
    expect(document.documentElement.dataset.theme).toBe('light');

    expect(toggleTheme()).toBe('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
  });
});
