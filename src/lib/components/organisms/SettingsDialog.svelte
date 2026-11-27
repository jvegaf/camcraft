<script lang="ts">
import { css, cx } from 'styled-system/css';
import {
  iconButton as iconButtonRecipe,
  panelRow as panelRowRecipe
} from 'styled-system/recipes';
import { loadThemePreference, saveThemePreference } from '$lib/ds/theme';
import { Button, Glyph, Icon, type IconName } from '../atoms';
import { CommandSearch } from '../molecules';
import { closeSettings, workbenchState } from '../workbench/workbench.svelte';
import ToolLibraryPanel from './ToolLibraryPanel.svelte';

// Preferences dialog of `design/camcraft-workbench-v2/04-settings.html`: titlebar with a real search,
// a page list on the left, two-column setting rows on the right and a Restaurar / Cancelar / Aplicar
// footer.
//
// **Real settings only (tasks 090 T2, 091, 107 T3).** A page is listed only when it has a real
// contract behind it: `Appearance` owns the `theme` field of `SettingsDto`
// (`src-tauri/src/lib.rs:93-104`) and `Tool library` owns the CAM library commands (107 T1/T2). The
// artboard's remaining entries (General, Layout, Canvas, Manufacturing, Import) are placeholders and
// are not rendered: a nav entry that stores nothing would be a promise the backend cannot keep.
// When a surface grows a contract, it is added here as one entry of `SETTINGS_GROUPS` — the nav, the
// search and the footer need no change.
//
// Each page also carries the dialog geometry its artboard draws (`size`): 04 draws 920×680 and 05
// needs 1040×720 so the library's three columns fit (107 S6). Width/height are the only per-page
// difference in the shell; the rows (`52px 1fr 56px`) and the columns (`220px 1fr`) are shared.
//
// The persistence contract is unchanged from the previous implementation and is owned by
// `src/lib/ds/theme.ts` (the same module the TopBar theme action writes with): the stored theme is
// read through `loadThemePreference()` when the dialog opens, written through
// `saveThemePreference()` **only** on Aplicar, and Cancelar/Escape/backdrop close without writing.
// Picking a segment or pressing Restaurar valores edits the draft only — the footer says so
// («Cambios sin aplicar») instead of silently storing an unconfirmed preference.
type SettingControl = 'theme';

type SettingRow = {
  id: string;
  label: string;
  hint: string;
  control: SettingControl;
};

type SettingsSection = { id: string; caption: string; rows: SettingRow[] };

/** Which body a page renders: its setting rows, or the tool library organism (107 T3). */
type PageBody = 'sections' | 'tool-library';

/** Dialog geometry per artboard: 04 draws 920×680, 05 needs 1040×720 for its three columns (S6). */
type DialogSize = 'standard' | 'wide';

/** The one action the library panel exposes: write its draft when the dialog confirms. */
type ToolLibraryPanelApi = { save: () => Promise<boolean> };

type SettingsPage = {
  id: string;
  label: string;
  icon: IconName;
  title: string;
  description: string;
  body: PageBody;
  size: DialogSize;
  sections: SettingsSection[];
};

type SettingsGroup = { id: string; caption: string; pages: SettingsPage[] };

const SETTINGS_GROUPS: SettingsGroup[] = [
  {
    id: 'general',
    caption: 'General',
    pages: [
      {
        id: 'appearance',
        label: 'Appearance',
        icon: 'theme',
        title: 'Appearance',
        description:
          'Camcraft visual configuration. Only the options the backend stores today are shown.',
        body: 'sections',
        size: 'standard',
        sections: [
          {
            id: 'theme',
            caption: 'Theme',
            rows: [
              {
                id: 'theme',
                label: 'Interface appearance',
                hint: 'Pick the default Camcraft theme. It is stored when you apply the changes.',
                control: 'theme'
              }
            ]
          }
        ]
      }
    ]
  },
  {
    id: 'cam',
    caption: 'CAM',
    pages: [
      {
        id: 'tool-library',
        label: 'Tool library',
        icon: 'layers',
        title: 'Tool library',
        description:
          'Tools available for CAM operations. Select a tool to edit its parameters, feeds/speeds and see its preview.',
        body: 'tool-library',
        size: 'wide',
        // The library has no configurable row of its own: its tree and detail form belong to the
        // page organism (107 T5/T6), so the search below keeps filtering `sections` only.
        sections: []
      }
    ]
  }
];

/** The values `src/lib/ds/theme.ts` knows how to apply; the labels are the UI copy. */
const THEME_OPTIONS: ReadonlyArray<{ value: string; label: string }> = [
  { value: 'dark', label: 'Dark' },
  { value: 'light', label: 'Light' },
  { value: 'system', label: 'System' }
];

const SETTINGS_PAGES = SETTINGS_GROUPS.flatMap(group => group.pages);
const FIRST_PAGE = SETTINGS_PAGES[0];

// `stored` is what the backend last confirmed; `settings` is the draft the footer applies. Keeping
// them apart is what makes «Cambios sin aplicar» honest and what lets Cancelar leave the stored
// preference untouched without a rollback to undo.
type Settings = { theme: string };
const DEFAULTS: Settings = { theme: 'dark' };

let stored = $state<Settings>({ ...DEFAULTS });
let settings = $state<Settings>({ ...DEFAULTS });
let error = $state('');
let query = $state('');
let activePageId = $state(FIRST_PAGE.id);
let searchSlot: HTMLDivElement | undefined = $state();
let toolLibraryPanel: ToolLibraryPanelApi | undefined = $state();

const uid = $props.id();
const pageRow = panelRowRecipe({ interactive: true });
const closeButton = iconButtonRecipe({ size: 'md' });

const normalizedQuery = $derived(query.trim().toLowerCase());
const activePage = $derived(
  SETTINGS_PAGES.find(page => page.id === activePageId) ?? FIRST_PAGE
);
const dirty = $derived(settings.theme !== stored.theme);

function rowMatches(row: SettingRow, needle: string): boolean {
  return (
    row.label.toLowerCase().includes(needle) ||
    row.hint.toLowerCase().includes(needle)
  );
}

/** The active page's sections with the rows the search keeps; empty sections are dropped. */
const visibleSections = $derived(
  normalizedQuery === ''
    ? activePage.sections
    : activePage.sections
        .map(section => ({
          ...section,
          rows: section.rows.filter(row => rowMatches(row, normalizedQuery))
        }))
        .filter(section => section.rows.length > 0)
);

const visibleRowCount = $derived(
  visibleSections.reduce((count, section) => count + section.rows.length, 0)
);
const noResults = $derived(normalizedQuery !== '' && visibleRowCount === 0);

function formatError(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e && typeof e === 'object' && 'message' in e) {
    const err = e as { code?: string; message: string };
    return err.code ? `${err.code}: ${err.message}` : err.message;
  }
  return String(e);
}

async function loadSettings() {
  try {
    const theme = await loadThemePreference();
    stored = { theme };
    settings = { theme };
  } catch (e) {
    // The draft keeps the documented default so the dialog still shows a real, selectable value.
    error = formatError(e);
  }
}

async function saveSettings() {
  error = '';
  try {
    // The library's draft is confirmed by the same Apply that confirms the theme. Its save reports
    // a failure through the shared error line and returns false, so nothing closes on a false success.
    if (activePage.body === 'tool-library') {
      const saved = await toolLibraryPanel?.save();
      if (saved === false) return;
    }
    const theme = await saveThemePreference(settings.theme);
    stored = { theme };
    settings = { theme };
    closeSettings();
  } catch (e) {
    error = formatError(e);
  }
}

/** Draft-only reset: the stored preference changes only when Aplicar writes it. */
function restoreDefaults() {
  settings = { ...DEFAULTS };
  error = '';
}

function selectTheme(theme: string) {
  settings = { ...settings, theme };
  error = '';
}

function clearSearch() {
  query = '';
}

function focusSearch() {
  searchSlot?.querySelector('input')?.focus();
}

function handleKeydown(event: KeyboardEvent) {
  if (!workbenchState.settingsOpen) return;
  // Dialog-local accelerators: they exist only while the dialog is open, so no global shortcut of
  // the app changes behaviour.
  if (
    (event.metaKey || event.ctrlKey) &&
    !event.altKey &&
    event.key.toLowerCase() === 'f'
  ) {
    event.preventDefault();
    focusSearch();
    return;
  }
  if (event.key === 'Escape') {
    event.preventDefault();
    closeSettings();
  }
}

$effect(() => {
  if (workbenchState.settingsOpen) {
    // A freshly opened dialog starts from the stored preference and an empty search.
    error = '';
    query = '';
    activePageId = FIRST_PAGE.id;
    stored = { ...DEFAULTS };
    settings = { ...DEFAULTS };
    void loadSettings();
  }
});

const overlay = css({
  position: 'fixed',
  inset: '0',
  zIndex: '100',
  display: 'grid',
  placeItems: 'center',
  bg: 'rgba(8,10,14,.45)'
});
const dialog = css({
  maxW: 'calc(100vw - 48px)',
  maxH: 'calc(100vh - 48px)',
  bg: 'panel',
  border: '1px solid',
  borderColor: 'border',
  borderRadius: 'dialog',
  boxShadow: '0 1px 0 rgba(255,255,255,.03), 0 16px 48px rgba(0,0,0,.5)',
  display: 'grid',
  gridTemplateRows: '52px 1fr 56px',
  overflow: 'hidden',
  color: 'text',
  fontFamily: 'sans',
  textStyle: 'body',
  // design/system.md «Accesibilidad y comportamiento»: honoring the OS motion preference is a
  // requirement, and the atoms/recipes only animate through transitions.
  _motionReduce: {
    '& *': { transition: 'none', animation: 'none' }
  }
});
// The width/height pairs of the two artboards (04: 920×680, 05: 1040×720). Kept as two static
// objects, not a computed value, so Panda extracts both and the page picks one by class.
const DIALOG_SIZES: Record<DialogSize, string> = {
  standard: css({ w: '920px', h: '680px' }),
  wide: css({ w: '1040px', h: '720px' })
};
const titlebar = css({
  display: 'flex',
  alignItems: 'center',
  gap: '12px',
  px: '16px',
  borderBottom: '1px solid',
  borderColor: 'hairline'
});
const title = css({ m: '0', textStyle: 'title', color: 'textHi' });
// The command search molecule owns the field's own metrics (380px in the app bar); the artboard
// sizes the dialog's chip at 260px, so the slot sets the width and the search follows it.
const searchArea = css({
  ml: 'auto',
  flex: 'none',
  w: '260px',
  maxW: '40vw',
  '& [role="search"]': { w: '100%' }
});
const content = css({
  display: 'grid',
  gridTemplateColumns: '220px 1fr',
  minH: '0'
});
const pages = css({
  display: 'flex',
  flexDirection: 'column',
  gap: '2px',
  bg: 'canvas',
  borderRight: '1px solid',
  borderColor: 'hairline',
  p: '12px 8px',
  overflow: 'auto'
});
const groupCaption = css({
  m: '10px 8px 6px',
  textStyle: 'caption',
  color: 'textOff'
});
// The `panelRow` recipe already supplies the wash, the hover and the `:focus-visible` ring; only
// the icon tone differs from a browser row (`TextDim` at rest, `Sel` when the page is current).
const pageIconTone = css({
  '& svg': { color: 'textDim' },
  '&[aria-current="page"] svg': { color: 'sel' }
});
const main = css({ p: '24px 28px', overflow: 'auto', minH: '0' });
// The library page fills its pane instead of scrolling as a column of rows, so its main becomes the
// flex column the page organism expects (`flex: 1`); `Appearance` keeps the block `main` above.
const mainPane = css({
  display: 'flex',
  flexDirection: 'column',
  overflow: 'hidden',
  '& > h3, & > p': { flex: 'none' }
});
const pageBody = css({
  display: 'flex',
  flexDirection: 'column',
  flex: '1',
  minH: '0',
  minW: '0'
});
const pageTitle = css({ m: '0', textStyle: 'dialog', color: 'textHi' });
const pageDescription = css({
  m: '6px 0 20px',
  textStyle: 'body',
  lineHeight: '1.5',
  color: 'textDim',
  maxW: '640px'
});
const section = css({
  py: '18px',
  borderTop: '1px solid',
  borderColor: 'hairline',
  _firstOfType: { borderTop: '0', pt: '0' }
});
const sectionCaption = css({
  m: '0 0 12px',
  textStyle: 'caption',
  color: 'textOff'
});
const setting = css({
  display: 'grid',
  gridTemplateColumns: '240px 1fr',
  gap: '16px',
  alignItems: 'start',
  py: '10px'
});
const settingLabel = css({ textStyle: 'label', color: 'textHi' });
const settingHint = css({ textStyle: 'small', color: 'textDim', mt: '3px' });
// The artboard's `.seg`. No design-system recipe covers a segmented control and the shared recipes
// are not this task's surface, so the pattern is drawn here from tokens only.
const seg = css({
  display: 'inline-flex',
  bg: 'app',
  border: '1px solid',
  borderColor: 'border',
  borderRadius: 'panel',
  overflow: 'hidden'
});
const segOption = css({
  h: '30px',
  px: '12px',
  border: 'none',
  borderRight: '1px solid',
  borderColor: 'hairline',
  bg: 'transparent',
  color: 'textDim',
  textStyle: 'small',
  cursor: 'pointer',
  _last: { borderRight: 'none' },
  _hover: { bg: 'raised', color: 'text' },
  _focusVisible: {
    outline: '2px solid',
    outlineColor: 'sel',
    outlineOffset: '-1px'
  },
  '&[aria-pressed="true"]': { bg: 'selSoft', color: 'sel' }
});
const emptyState = css({
  display: 'grid',
  justifyItems: 'start',
  gap: '8px',
  py: '24px 0'
});
const emptyTitle = css({ m: '0', textStyle: 'body', color: 'text' });
const emptyHint = css({ m: '0', textStyle: 'small', color: 'textDim' });
const footer = css({
  display: 'flex',
  alignItems: 'center',
  gap: '10px',
  px: '16px',
  borderTop: '1px solid',
  borderColor: 'hairline',
  bg: 'app/50'
});
const footerStatus = css({
  flex: '1',
  minW: '0',
  display: 'flex',
  alignItems: 'center',
  overflow: 'hidden'
});
const errorText = css({
  m: '0',
  textStyle: 'small',
  color: 'danger',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
});
const noteText = css({
  m: '0',
  textStyle: 'small',
  color: 'textDim',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
});
</script>

<svelte:window onkeydown={handleKeydown} />

{#if workbenchState.settingsOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class={overlay}
    onclick={event => {
      if (event.target === event.currentTarget) closeSettings();
    }}
  >
    <div
      class={cx(dialog, DIALOG_SIZES[activePage.size])}
      role="dialog"
      aria-label="Preferences"
    >
      <div class={titlebar}>
        <h2 class={title}>Preferences</h2>
        <div class={searchArea} bind:this={searchSlot}>
          <CommandSearch
            placeholder="Search settings…"
            shortcut="⌘F"
            ariaLabel="Search settings"
            value={query}
            onquery={value => {
              query = value;
            }}
          />
        </div>
        <button
          type="button"
          class={closeButton}
          aria-label="Close"
          onclick={closeSettings}
        >
          <Glyph name="close" />
        </button>
      </div>

      <div class={content}>
        <nav class={pages} aria-label="Preferences pages">
          {#each SETTINGS_GROUPS as group (group.id)}
            <div class={groupCaption}>{group.caption}</div>
            {#each group.pages as page (page.id)}
              <button
                type="button"
                class={cx(pageRow.root, pageIconTone)}
                aria-current={activePageId === page.id ? 'page' : undefined}
                onclick={() => {
                  activePageId = page.id;
                }}
              >
                <Icon name={page.icon} />
                <span class={pageRow.label}>{page.label}</span>
              </button>
            {/each}
          {/each}
        </nav>

        <section
          class={cx(main, activePage.body === 'tool-library' && mainPane)}
          aria-labelledby={`${uid}-page-title`}
        >
          <h3 class={pageTitle} id={`${uid}-page-title`}>{activePage.title}</h3>
          <p class={pageDescription}>{activePage.description}</p>

          {#if activePage.body === 'tool-library'}
            <!-- The panel owns the library commands and fills the page's three snippet props with
                 the tree, the detail form and the preview (107 T5/T6/T7 + wiring). -->
            <div class={pageBody}>
              <ToolLibraryPanel
                bind:this={toolLibraryPanel}
                onerror={message => {
                  error = message;
                }}
              />
            </div>
          {:else if noResults}
            <div class={emptyState} role="status">
              <p class={emptyTitle}>
                No results for “{query.trim()}”
              </p>
              <p class={emptyHint}>
                Try another word or clear the search to see all the options.
              </p>
              <Button variant="ghost" size="sm" onclick={clearSearch}>
                Clear search
              </Button>
            </div>
          {:else}
            {#each visibleSections as sectionData (sectionData.id)}
              <div class={section}>
                <h4 class={sectionCaption}>{sectionData.caption}</h4>
                {#each sectionData.rows as row (row.id)}
                  <div class={setting}>
                    <div>
                      <div class={settingLabel}>{row.label}</div>
                      <div class={settingHint} id={`${uid}-${row.id}-hint`}>
                        {row.hint}
                      </div>
                    </div>
                    <div>
                      {#if row.control === 'theme'}
                        <div class={seg} role="group" aria-label={row.label}>
                          {#each THEME_OPTIONS as option (option.value)}
                            <button
                              type="button"
                              class={segOption}
                              aria-pressed={settings.theme === option.value}
                              aria-describedby={`${uid}-${row.id}-hint`}
                              onclick={() => selectTheme(option.value)}
                            >
                              {option.label}
                            </button>
                          {/each}
                        </div>
                      {/if}
                    </div>
                  </div>
                {/each}
              </div>
            {/each}
          {/if}
        </section>
      </div>

      <div class={footer}>
        <Button variant="ghost" onclick={restoreDefaults}>
          Restore defaults
        </Button>
        <div class={footerStatus}>
          {#if error}
            <p class={errorText} aria-live="assertive">{error}</p>
          {:else if dirty}
            <p class={noteText} aria-live="polite">
              Unsaved changes: use Apply to store them.
            </p>
          {/if}
        </div>
        <Button onclick={closeSettings}>Cancel</Button>
        <Button variant="primary" onclick={saveSettings}>Apply</Button>
      </div>
    </div>
  </div>
{/if}
