<script lang="ts">
import { error } from '@sveltejs/kit';
import { css, cx } from 'styled-system/css';
import { onDestroy } from 'svelte';
import { page } from '$app/state';
import { type CatalogEntry, findCatalogEntry } from '$lib/ds/catalog';
import { inventory } from '$lib/ds/inventory.generated';

// Component sheet (097 T2, S2 + S4): the real component rendered once per declared state inside a
// token-styled frame, with the props of each state shown as data. A component that cannot render in
// isolation shows its reason and its states as data, never a fake instance.
//
// The light/dark switch only writes the `html[data-theme]` attribute the Panda conditions in
// `panda.config.ts` (`_light: '[data-theme=light] &'`) already drive. It stores nothing and touches
// no app state, and the previous attribute is restored when the sheet unmounts.

function sheetFor(slug: string | undefined): CatalogEntry {
  const entry = findCatalogEntry(slug ?? '');
  if (!entry) error(404, `Unknown component: ${slug ?? ''}`);
  return entry;
}

function consumersOf(entry: CatalogEntry): string[] {
  return (
    inventory.find(component => component.path === entry.path)?.consumers ?? []
  );
}

function consumerName(path: string): string {
  return path.split('/').pop() ?? path;
}

function describe(props: Record<string, unknown>): string {
  return JSON.stringify(props, (_key, value) =>
    typeof value === 'function' ? '[function]' : value
  );
}

const initialTheme =
  typeof document === 'undefined'
    ? ''
    : (document.documentElement.dataset.theme ?? '');
let theme = $state<'light' | 'dark'>(
  initialTheme === 'light' ? 'light' : 'dark'
);

function setTheme(next: 'light' | 'dark') {
  theme = next;
  if (typeof document !== 'undefined') {
    document.documentElement.dataset.theme = next;
  }
}

onDestroy(() => {
  if (typeof document === 'undefined') return;
  if (initialTheme) document.documentElement.dataset.theme = initialTheme;
  else delete document.documentElement.dataset.theme;
});

const sheet = css({
  minH: '100vh',
  bg: 'canvas',
  color: 'text',
  fontFamily: 'sans',
  fontSize: '13px',
  lineHeight: '1.4',
  p: '24px'
});
const inner = css({ maxW: '1100px', mx: 'auto' });
const back = css({
  display: 'inline-block',
  mb: '14px',
  color: 'sel',
  textStyle: 'small',
  textDecoration: 'none',
  _hover: { textDecoration: 'underline' }
});
const header = css({
  display: 'flex',
  alignItems: 'flex-start',
  gap: '12px',
  mb: '20px'
});
const title = css({ textStyle: 'hero', color: 'textHi', m: '0' });
const layer = css({ textStyle: 'mono', color: 'textOff' });
const purpose = css({ color: 'textDim', m: '0', maxW: '560px' });
const usage = css({
  display: 'flex',
  gap: '6px',
  flexWrap: 'wrap',
  mt: '6px',
  textStyle: 'small',
  color: 'textOff'
});
const usageNames = css({ textStyle: 'mono', color: 'textDim' });
const themeSwitch = css({
  ml: 'auto',
  display: 'flex',
  gap: '4px',
  flex: 'none'
});
const themeButton = css({
  px: '9px',
  py: '4px',
  borderRadius: 'control',
  border: '1px solid',
  borderColor: 'border',
  bg: 'panel',
  color: 'textDim',
  font: 'inherit',
  textStyle: 'small',
  cursor: 'pointer',
  _hover: { bg: 'raised', color: 'textHi' },
  _focusVisible: {
    outline: '2px solid',
    outlineColor: 'sel',
    outlineOffset: '-1px'
  }
});
const themeActive = css({ bg: 'selSoft', color: 'textHi', borderColor: 'sel' });
const matrix = css({
  display: 'grid',
  gap: '12px',
  gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))'
});
const stateCard = css({
  bg: 'panel',
  border: '1px solid',
  borderColor: 'border',
  borderRadius: 'panel',
  boxShadow: 'float',
  overflow: 'hidden'
});
const stateHead = css({
  px: '10px',
  py: '6px',
  borderBottom: '1px solid',
  borderColor: 'hairline',
  color: 'textDim',
  textStyle: 'caption'
});
const frame = css({
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  gap: '10px',
  minH: '88px',
  p: '14px',
  bg: 'app'
});
const props = css({
  display: 'flex',
  gap: '8px',
  px: '10px',
  py: '6px',
  borderTop: '1px solid',
  borderColor: 'hairline',
  textStyle: 'mono',
  color: 'textOff',
  overflowX: 'auto'
});
const note = css({
  bg: 'panel',
  border: '1px solid',
  borderColor: 'warn/40',
  borderRadius: 'panel',
  p: '12px',
  mb: '16px',
  color: 'text',
  textStyle: 'small'
});
const dataList = css({
  display: 'grid',
  gap: '6px',
  m: '0',
  p: '0',
  listStyle: 'none'
});
const dataRow = css({
  bg: 'panel',
  border: '1px solid',
  borderColor: 'border',
  borderRadius: 'panel',
  p: '10px'
});
</script>

{#key page.params.slug}
  {@const entry = sheetFor(page.params.slug)}
  {@const consumers = consumersOf(entry)}
  <div class={sheet}>
    <div class={inner}>
      <a class={back} href="/ds">← Catalog</a>

      <header class={header}>
        <div>
          <h1 class={title}>{entry.name}</h1>
          <span class={layer}>{entry.layer}</span>
          <p class={purpose}>{entry.purpose}</p>
          <p class={usage}>
            <span
              >In use: <b data-ds-consumer-count>{consumers.length}</b> consumers</span
            >
            {#if consumers.length > 0}
              <span class={usageNames} data-ds-consumer-names
                >{consumers.slice(0, 2).map(consumerName).join(', ')}</span
              >
            {/if}
          </p>
        </div>
        <div class={themeSwitch} role="group" aria-label="Sheet theme">
          <button
            type="button"
            class={cx(themeButton, theme === 'light' && themeActive)}
            data-theme-option="light"
            aria-pressed={theme === 'light'}
            onclick={() => setTheme('light')}>Light</button
          >
          <button
            type="button"
            class={cx(themeButton, theme === 'dark' && themeActive)}
            data-theme-option="dark"
            aria-pressed={theme === 'dark'}
            onclick={() => setTheme('dark')}>Dark</button
          >
        </div>
      </header>

      {#if entry.renderable}
        <div class={matrix}>
          {#each entry.states as state (state.label)}
            {@const Component = entry.component}
            <article class={stateCard} data-ds-state data-ds-state-label={state.label}>
              <header class={stateHead}>{state.label}</header>
              <div class={frame} data-ds-instance>
                <Component {...state.props}>{state.content}</Component>
              </div>
              <div class={props}>
                <span>props</span>
                <code data-ds-props>{describe(state.props)}</code>
              </div>
            </article>
          {/each}
        </div>
      {:else}
        <p class={note} data-ds-render-note
          >Does not render in isolation: {entry.reason} Its states stay as data.</p
        >
        <ul class={dataList}>
          {#each entry.states as state (state.label)}
            <li class={dataRow} data-ds-state data-ds-state-label={state.label}>
              <header class={stateHead}>{state.label}</header>
              <div class={props}>
                <span>props</span>
                <code data-ds-props>{describe(state.props)}</code>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
{/key}
