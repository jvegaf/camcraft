<script lang="ts">
import { onMount } from 'svelte';
import { css } from 'styled-system/css';

type Point3Dto = { x: number; y: number; z: number };
type Line3Dto = { start: Point3Dto; end: Point3Dto };
type MeshVertexDto = { position: Point3Dto; depth: number };
type SceneSnapshotDto = {
  stockWidth: number;
  stockHeight: number;
  stockThickness: number;
  stockBox: Line3Dto[];
  vertices: MeshVertexDto[];
  triangles: [number, number, number][];
  cameraTarget: Point3Dto;
  cameraYaw: number;
  cameraPitch: number;
  cameraDistance: number;
};

let container: HTMLDivElement | undefined = $state();
let fpsText = $state('FPS: --');
let statusText = $state('Cargando escena...');
let selectionText = $state('Selección: ninguna');

const wrap = css({
  position: 'absolute',
  inset: '0',
  bg: 'canvas',
  overflow: 'hidden'
});
const hud = css({
  position: 'absolute',
  right: '16px',
  bottom: '48px',
  px: '10px',
  py: '8px',
  borderRadius: '6px',
  bg: 'rgba(16,20,28,0.72)',
  color: '#d7dde6',
  fontSize: '12px',
  fontFamily: 'mono',
  lineHeight: '1.5',
  pointerEvents: 'none',
  whiteSpace: 'pre-line',
  border: '1px solid rgba(120,140,170,0.2)'
});

function depthColor(depth: number): [number, number, number] {
  const d = Math.max(0, Math.min(1, depth));
  const r = 0.25 + d * 0.65;
  const g = 0.58 - d * 0.28;
  const b = 0.72 - d * 0.45;
  return [r, g, b];
}

onMount(() => {
  let disposed = false;
  let raf = 0;
  let renderer: import('three').WebGLRenderer | undefined;
  let controls: any;
  let ro: ResizeObserver | undefined;
  const frameTimes: number[] = [];
  let fpsWindowStart = performance.now();

  (async () => {
    const THREE = await import('three');
    const { OrbitControls } = await import('three/addons/controls/OrbitControls.js');
    const { invoke } = await import('@tauri-apps/api/core');

    if (disposed || !container) return;
    let snapshot: SceneSnapshotDto;
    try {
      snapshot = await invoke<SceneSnapshotDto>('scene_snapshot');
    } catch (e) {
      statusText = `Error escena: ${String(e)}`;
      return;
    }
    if (disposed || !container) return;

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x121821);

    const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 5000);
    const t = snapshot.cameraTarget;
    const yaw = snapshot.cameraYaw;
    const pitch = snapshot.cameraPitch;
    const dist = snapshot.cameraDistance;
    camera.position.set(
      t.x + dist * Math.cos(yaw) * Math.cos(pitch),
      t.z + dist * Math.sin(pitch),
      -(t.y + dist * Math.sin(yaw) * Math.cos(pitch))
    );
    camera.up.set(0, 1, 0);

    renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    container.appendChild(renderer.domElement);

    controls = new OrbitControls(camera, renderer.domElement);
    controls.target.set(t.x, t.z, -t.y);
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.screenSpacePanning = true;
    controls.update();

    const ambient = new THREE.AmbientLight(0xffffff, 0.65);
    scene.add(ambient);
    const dir = new THREE.DirectionalLight(0xffffff, 0.85);
    dir.position.set(120, 180, 80);
    scene.add(dir);

    const boxPoints: number[] = [];
    for (const line of snapshot.stockBox) {
      boxPoints.push(line.start.x, line.start.z, -line.start.y);
      boxPoints.push(line.end.x, line.end.z, -line.end.y);
    }
    const boxGeom = new THREE.BufferGeometry();
    boxGeom.setAttribute('position', new THREE.Float32BufferAttribute(boxPoints, 3));
    const boxMat = new THREE.LineBasicMaterial({ color: 0x88a0b8, transparent: true, opacity: 0.7 });
    scene.add(new THREE.LineSegments(boxGeom, boxMat));

    const positions = new Float32Array(snapshot.vertices.length * 3);
    const colors = new Float32Array(snapshot.vertices.length * 3);
    snapshot.vertices.forEach((v, i) => {
      positions[i * 3] = v.position.x;
      positions[i * 3 + 1] = v.position.z;
      positions[i * 3 + 2] = -v.position.y;
      const [r, g, b] = depthColor(v.depth);
      colors[i * 3] = r;
      colors[i * 3 + 1] = g;
      colors[i * 3 + 2] = b;
    });
    const indices = new Uint32Array(snapshot.triangles.length * 3);
    snapshot.triangles.forEach((tri, i) => {
      indices[i * 3] = tri[0];
      indices[i * 3 + 1] = tri[1];
      indices[i * 3 + 2] = tri[2];
    });
    const meshGeom = new THREE.BufferGeometry();
    meshGeom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    meshGeom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
    meshGeom.setIndex(new THREE.BufferAttribute(indices, 1));
    meshGeom.computeVertexNormals();
    const meshMat = new THREE.MeshStandardMaterial({
      vertexColors: true,
      roughness: 0.72,
      metalness: 0.08,
      side: THREE.DoubleSide
    });
    const surfaceMesh = new THREE.Mesh(meshGeom, meshMat);
    scene.add(surfaceMesh);

    const grid = new THREE.GridHelper(220, 22, 0x334455, 0x223344);
    grid.position.set(50, -20.01, -50);
    scene.add(grid);

    // hover/select highlight
    const highlightMat = new THREE.MeshBasicMaterial({
      color: 0x6fd3e8,
      transparent: true,
      opacity: 0.45,
      depthTest: true,
      side: THREE.DoubleSide
    });
    const hoverMesh = new THREE.Mesh(new THREE.BufferGeometry(), highlightMat.clone());
    hoverMesh.visible = false;
    scene.add(hoverMesh);
    const selectMat = new THREE.MeshBasicMaterial({
      color: 0xf5a623,
      transparent: true,
      opacity: 0.55,
      depthTest: true,
      side: THREE.DoubleSide
    });
    const selectMesh = new THREE.Mesh(new THREE.BufferGeometry(), selectMat);
    selectMesh.visible = false;
    scene.add(selectMesh);

    const raycaster = new THREE.Raycaster();
    const pointer = new THREE.Vector2();
    let hoveredTri: number | null = null;
    let selectedTri: number | null = null;

    function triGeometry(triIndex: number) {
      const tri = snapshot.triangles[triIndex];
      const g = new THREE.BufferGeometry();
      const pos = new Float32Array(9);
      for (let j = 0; j < 3; j++) {
        const v = snapshot.vertices[tri[j]].position;
        pos[j * 3] = v.x;
        pos[j * 3 + 1] = v.z;
        pos[j * 3 + 2] = -v.y;
      }
      g.setAttribute('position', new THREE.BufferAttribute(pos, 3));
      g.computeVertexNormals();
      return g;
    }

    function updatePointer(event: PointerEvent) {
      if (!renderer) return;
      const rect = renderer.domElement.getBoundingClientRect();
      pointer.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
      pointer.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
    }

    function pickTriangle(event: PointerEvent): number | null {
      if (!renderer) return null;
      updatePointer(event);
      raycaster.setFromCamera(pointer, camera);
      const hits = raycaster.intersectObject(surfaceMesh, false);
      if (hits.length === 0) return null;
      const faceIndex = hits[0].faceIndex;
      if (faceIndex == null) return null;
      return faceIndex;
    }

    const onPointerMove = (event: PointerEvent) => {
      const tri = pickTriangle(event);
      if (tri === hoveredTri) return;
      hoveredTri = tri;
      if (tri == null) {
        hoverMesh.visible = false;
      } else {
        hoverMesh.geometry.dispose();
        hoverMesh.geometry = triGeometry(tri);
        hoverMesh.visible = tri !== selectedTri;
      }
    };

    const onClick = (event: PointerEvent) => {
      const tri = pickTriangle(event);
      selectedTri = tri;
      if (tri == null) {
        selectMesh.visible = false;
        selectionText = 'Selección: ninguna';
      } else {
        selectMesh.geometry.dispose();
        selectMesh.geometry = triGeometry(tri);
        selectMesh.visible = true;
        hoverMesh.visible = false;
        const triVerts = snapshot.triangles[tri];
        const d = triVerts
          .map((vi) => snapshot.vertices[vi].depth)
          .reduce((a, b) => a + b, 0) / 3;
        selectionText = `Selección: tri ${tri}\nProfundidad media: ${(d * snapshot.stockThickness).toFixed(2)} mm`;
      }
    };

    renderer.domElement.addEventListener('pointermove', onPointerMove);
    renderer.domElement.addEventListener('click', onClick);

    const resize = () => {
      if (!container || !renderer) return;
      const w = container.clientWidth;
      const h = container.clientHeight;
      if (w <= 0 || h <= 0) return;
      renderer.setSize(w, h, false);
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
    };
    resize();
    ro = new ResizeObserver(resize);
    ro.observe(container);

    statusText = `Scene: ${snapshot.vertices.length} verts, ${snapshot.triangles.length} tris`;

    let last = performance.now();
    const tick = (now: number) => {
      if (disposed) return;
      const dt = now - last;
      last = now;
      if (dt > 0 && dt < 200) {
        frameTimes.push(dt);
        if (frameTimes.length > 2000) frameTimes.shift();
      }
      if (now - fpsWindowStart >= 1000) {
        const recent = frameTimes.slice(-120);
        const avg = recent.reduce((a, b) => a + b, 0) / Math.max(1, recent.length);
        const fps = 1000 / Math.max(1, avg);
        const sorted = [...recent].sort((a, b) => a - b);
        const minFps = 1000 / Math.max(1, sorted[sorted.length - 1] || 16.7);
        const p1 = sorted[Math.floor(sorted.length * 0.99)] || sorted[sorted.length - 1] || 16.7;
        const low1 = 1000 / Math.max(1, p1);
        fpsText = `FPS avg: ${fps.toFixed(1)}\nFPS min: ${minFps.toFixed(1)}\nFPS 1% low: ${low1.toFixed(1)}`;
        fpsWindowStart = now;
      }
      controls.update();
      renderer!.render(scene, camera);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);

    return () => {
      renderer?.domElement.removeEventListener('pointermove', onPointerMove);
      renderer?.domElement.removeEventListener('click', onClick);
    };
  })().then((cleanup) => {
    if (disposed) cleanup?.();
  });

  return () => {
    disposed = true;
    cancelAnimationFrame(raf);
    ro?.disconnect();
    controls?.dispose?.();
    if (renderer) {
      renderer.dispose();
      renderer.domElement.remove();
    }
  };
});
</script>

<div class={wrap} bind:this={container}>
  <div class={hud}>{statusText}{'\n'}{fpsText}{'\n'}{selectionText}</div>
</div>
