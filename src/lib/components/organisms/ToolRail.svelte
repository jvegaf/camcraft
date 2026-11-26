<script lang="ts">
import { css } from 'styled-system/css';
import { visuallyHidden } from 'styled-system/patterns';
import { Button, Icon } from '../atoms';

// Organism (095 T3, S4): the viewport tool rail of
// `design/camcraft-workbench-v2/02-floating-studio.html` `.tool-rail` — Importar, Capas, Origen and
// the primary Cut action Generar plan.
//
// Only Importar has a command behind it (the file dialog plus `open_document`, which moved here from
// the old Abrir tool; the old Guardar tool now lives in the TopBar icon actions). The app has no
// layer tree, no setup-origin editor and no plan generator yet, so those three render as `disabled`
// buttons — visible at artboard parity but never a click that silently does nothing — and each one
// carries the reason in its own text so it is not a hover-only affordance.
interface Props {
  onOpen?: () => void;
}

let { onOpen }: Props = $props();

const rail = css({
  position: 'absolute',
  top: '64px',
  left: '316px',
  display: 'flex',
  gap: '6px',
  zIndex: '20'
});
</script>

<div class={rail} role="group" aria-label="Acciones rápidas">
  <Button variant="secondary" size="sm" onclick={onOpen}>
    <Icon name="import" size="sm" />
    Importar
  </Button>
  <Button variant="secondary" size="sm" disabled>
    <Icon name="layers" size="sm" />
    Capas
    <span class={visuallyHidden()}
      >— todavía no disponible: el árbol de capas no está cableado.</span
    >
  </Button>
  <Button variant="secondary" size="sm" disabled>
    <Icon name="origin" size="sm" />
    Origen
    <span class={visuallyHidden()}
      >— todavía no disponible: el origen del setup no está cableado.</span
    >
  </Button>
  <Button variant="primary" size="sm" disabled>
    <Icon name="arrow-right" size="sm" />
    Generar plan
    <span class={visuallyHidden()}
      >— todavía no disponible: el backend no expone generación de planes.</span
    >
  </Button>
</div>
