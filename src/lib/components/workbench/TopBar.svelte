<script lang="ts">
interface Props {
  projectName?: string;
  searchPlaceholder?: string;
  layoutLabel?: string;
}

let {
  projectName = 'controller-board.cam',
  searchPlaceholder = 'Buscar comando, capa u operación…',
  layoutLabel = 'Floating'
}: Props = $props();
</script>

<header class="topbar">
  <div class="mark" aria-hidden="true">C</div>
  <div class="product">Camcraft</div>
  <div class="sep" aria-hidden="true"></div>
  <button class="workspace" aria-label="Proyecto actual">
    <span>{projectName}</span>
    <span class="muted">▾</span>
  </button>
  <div class="search" role="search" aria-label="Buscar comando">
    <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6"/><path d="M20 20l-4.2-4.2"/></svg>
    <span>{searchPlaceholder}</span>
    <kbd>⌘K</kbd>
  </div>
  <div class="top-actions">
    <div class="layout-pill" aria-label="Layout flotante"><b>{layoutLabel}</b> layout</div>
    <button class="icon-btn" aria-label="Tema">
      <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/></svg>
    </button>
    <button class="icon-btn" aria-label="Configuración">
      <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M19 12a7 7 0 0 0-.1-1l2-1.5-1.8-3.1-2.3.8a7 7 0 0 0-1.7-1l-.3-2.4h-3.6l-.3 2.4a7 7 0 0 0-1.7 1l-2.3-.8L4.1 9.5 6.1 11a7 7 0 0 0 0 2l-2 1.5 1.8 3.1 2.3-.8a7 7 0 0 0 1.7 1l.3 2.4h3.6l.3-2.4a7 7 0 0 0 1.7-1l2.3.8 1.8-3.1-2-1.5c.1-.3.1-.7.1-1z"/></svg>
    </button>
  </div>
</header>

<style>
  .topbar { position:absolute; top:0; left:0; right:0; height:48px; display:flex; align-items:center; gap:10px; padding:0 12px; background:rgba(23,27,33,.92); backdrop-filter:blur(8px); border-bottom:1px solid var(--line-hair); z-index:30; }
  .mark { width:26px; height:26px; border-radius:7px; display:grid; place-items:center; background:linear-gradient(180deg,#22303b,#18222b); border:1px solid var(--line-strong); color:var(--sel); font:700 12px var(--font); flex:none; }
  .product { font-size:15px; font-weight:600; color:var(--text-hi); letter-spacing:-.01em; }
  .sep { width:1px; height:18px; background:var(--line-hair); flex:none; }
  .workspace { display:flex; align-items:center; gap:6px; padding:5px 8px; border-radius:6px; color:var(--text); font-size:12px; border:1px solid transparent; }
  .workspace:hover { background:var(--bg-raised); border-color:var(--line-hair); }
  .workspace .muted { color:var(--text-off); }
  .search { margin:0 auto; width:380px; max-width:40vw; height:30px; border-radius:6px; background:rgba(18,21,26,.75); border:1px solid var(--line-strong); display:flex; align-items:center; gap:8px; padding:0 10px; color:var(--text-dim); font-size:12px; }
  .search:focus-within { border-color:rgba(111,211,232,.45); box-shadow:0 0 0 2px rgba(111,211,232,.12); }
  .search kbd { margin-left:auto; font:11px var(--mono); color:var(--text-off); padding:1px 5px; border-radius:3px; background:var(--bg-raised); border:1px solid var(--line-hair); }
  .top-actions { display:flex; align-items:center; gap:4px; }
  .icon-btn { width:30px; height:30px; display:grid; place-items:center; border-radius:6px; color:var(--text-dim); }
  .icon-btn:hover { background:var(--bg-raised); color:var(--text-hi); }
  .icon-btn:focus-visible, .workspace:focus-visible { outline:2px solid var(--sel); outline-offset:-1px; }
  .layout-pill { display:flex; align-items:center; gap:6px; padding:4px 8px; border-radius:6px; border:1px solid var(--line-hair); background:var(--bg-raised); font-size:11px; color:var(--text-dim); }
  .layout-pill b { color:var(--text); font-weight:500; }
  .icon { width:16px; height:16px; stroke:currentColor; fill:none; stroke-width:1.6; stroke-linecap:round; stroke-linejoin:round; flex:none; }
  button { font:inherit; color:inherit; background:none; border:none; }
  button { cursor:pointer; }
</style>
