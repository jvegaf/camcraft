<script lang="ts">
import { css } from 'styled-system/css';

interface Props {
  x?: string;
  y?: string;
  z?: string;
  message?: string;
  status?: string;
}

let {
  x = 'X 42.80',
  y = 'Y 31.20',
  z = 'Z 2.00',
  message = 'mm · Grid 1 mm · GRBL',
  status = '● Sin errores'
}: Props = $props();

const statusBar = css({
  position: 'absolute',
  left: '0',
  right: '0',
  bottom: '0',
  h: '34px',
  display: 'flex',
  alignItems: 'center',
  gap: '12px',
  px: '14px',
  bg: 'panel',
  backdropFilter: 'blur(8px)',
  borderTop: '1px solid',
  borderColor: 'hairline',
  fontSize: '11px',
  color: 'textDim',
  zIndex: '30'
});
const mono = css({ fontSize: '11px', fontFamily: 'mono', color: 'text' });
const ok = css({ color: 'ok' });
const spacer = css({ flex: '1' });
const sep = css({ w: '1px', h: '14px', bg: 'hairline' });
</script>

<footer class={statusBar} aria-label="Barra de estado">
  <span class={mono}>{x}</span>
  <span class={mono}>{y}</span>
  <span class={mono}>{z}</span>
  <span class={sep} aria-hidden="true"></span>
  <span>{message}</span>
  <span class={spacer}></span>
  <span class={ok}>{status}</span>
</footer>
