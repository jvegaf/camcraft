<script lang="ts">
interface RecentItem {
  name: string;
  path: string;
  when: string;
}

interface Props {
  recents?: RecentItem[];
}

let {
  recents = [
    {
      name: 'controller-board.cam',
      path: '~/Boards/controller/',
      when: 'Hace 12 min'
    },
    { name: 'usb-adapter.cam', path: '~/Boards/adapters/', when: 'Ayer' },
    { name: 'front-panel.cam', path: '~/Boards/panel/', when: 'Hace 3 días' }
  ]
}: Props = $props();
</script>

<div class="start-shell">
  <div class="start-card">
    <div class="s-panel">
      <p class="eyebrow">Camcraft</p>
      <h2 class="welcome">Bienvenido al estudio CAM</h2>
      <p class="lead">Abrí una placa o retomá un proyecto. El shell usa superficies flotantes, jerarquía de bordes clara y command search central.</p>
      <button class="action primary">
        <span class="ic">
          <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>
        </span>
        <div><strong>Nuevo proyecto PCB</strong><span>Configurar stock, origen y board</span></div>
      </button>
      <button class="action">
        <span class="ic">
          <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h6l2 2h8v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z"/></svg>
        </span>
        <div><strong>Abrir proyecto</strong><span>.cam o fuentes de placa</span></div>
      </button>
    </div>
    <div class="s-panel">
      <h2>Recientes</h2>
      <p class="lead">Retomá con contexto visible antes de abrir.</p>
      {#each recents as recent}
        <button class="recent">
          <div><div class="name">{recent.name}</div><div class="path">{recent.path}</div></div>
          <div class="when">{recent.when}</div>
        </button>
      {/each}
    </div>
  </div>
</div>

<style>
  .start-shell {
    --bg-app:#12151a; --bg-canvas:#0d1014; --bg-panel:#171b21; --bg-raised:#1e242b; --bg-overlay:#232a32;
    --line-hair:#262d36; --line-strong:#35404c;
    --text-hi:#e7ecf3; --text:#c3ccd8; --text-dim:#8b95a3; --text-off:#5c6673;
    --sel:#6fd3e8; --sel-soft:rgba(111,211,232,.13);
    --cut:#f5a623; --cut-soft:rgba(245,166,35,.13);
    --shadow-float:0 1px 0 rgba(255,255,255,.03), 0 12px 32px rgba(0,0,0,.42);
    --font: Inter, "Inter UI", ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
    --mono: ui-monospace, SFMono-Regular, Menlo, monospace;
    height:100%; min-height:520px; border:1px solid var(--line-strong); border-radius:12px; overflow:hidden; background:var(--bg-canvas); position:relative; box-shadow:0 24px 80px rgba(0,0,0,.35); display:grid; place-items:center; color:var(--text); font:13px/1.4 var(--font);
  }
  .start-shell::before { content:""; position:absolute; inset:0; background:linear-gradient(90deg, rgba(38,45,54,.22) 1px, transparent 1px) 0 0/80px 80px, linear-gradient(180deg, rgba(38,45,54,.22) 1px, transparent 1px) 0 0/80px 80px; }
  .start-card { position:relative; width:940px; max-width:calc(100% - 48px); display:grid; grid-template-columns:1fr 1fr; gap:16px; }
  .s-panel { background:rgba(23,27,33,.95); border:1px solid var(--line-strong); border-radius:12px; box-shadow:var(--shadow-float); padding:24px; }
  .eyebrow { font-size:11px; letter-spacing:.08em; text-transform:uppercase; color:var(--text-off); margin:0 0 6px; }
  .welcome { margin:0 0 6px; font:600 28px/1.15 var(--font); color:var(--text-hi); }
  h2 { margin:0; font-size:20px; font-weight:600; color:var(--text-hi); }
  .lead { margin:8px 0 0; color:var(--text-dim); font-size:13px; line-height:1.5; }
  .action { width:100%; display:flex; align-items:center; gap:12px; padding:14px; margin-top:12px; border-radius:8px; background:var(--bg-raised); border:1px solid var(--line-hair); text-align:left; cursor:pointer; font:inherit; color:inherit; }
  .action:hover { border-color:rgba(111,211,232,.3); background:#252d36; }
  .action .ic { width:34px; height:34px; border-radius:8px; display:grid; place-items:center; background:var(--sel-soft); color:var(--sel); flex:none; }
  .action.primary .ic { background:var(--cut-soft); color:var(--cut); }
  .action strong { display:block; font-size:14px; color:var(--text-hi); font-weight:500; }
  .action span { display:block; font-size:12px; color:var(--text-dim); margin-top:2px; }
  .recent { width:100%; display:flex; gap:12px; align-items:center; padding:10px 0; border-bottom:1px solid var(--line-hair); text-align:left; background:none; border-left:none; border-right:none; border-top:none; cursor:pointer; font:inherit; color:inherit; }
  .recent:last-child { border-bottom:0; padding-bottom:0; }
  .recent:hover .name { color:var(--text-hi); }
  .recent .name { font-size:13px; color:var(--text); font-weight:500; }
  .recent .path { font-size:11px; color:var(--text-off); font-family:var(--mono); margin-top:2px; }
  .recent .when { margin-left:auto; font-size:11px; color:var(--text-off); }
  .icon { width:16px; height:16px; stroke:currentColor; fill:none; stroke-width:1.6; stroke-linecap:round; stroke-linejoin:round; flex:none; }
</style>
