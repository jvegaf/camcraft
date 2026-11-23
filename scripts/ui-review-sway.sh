#!/usr/bin/env bash
set -euo pipefail

# Visual review de cam-ui contra fixtures/ui usando sway headless.
# Corre en background (compositor propio), no toca tu sesión Hyprland.
#
# Uso:
#   scripts/ui-review-sway.sh            # todas las escenas del manifest
#   scripts/ui-review-sway.sh settings   # una escena
#
# Requisitos: sway, grim, jq, python3, cargo.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/fixtures/ui/manifest.json"
DIFF_DIR="$ROOT/target/ui-review-diffs"
SCENES="${*:-}"

for cmd in sway grim jq python3 cargo; do
  command -v "$cmd" >/dev/null || { echo "falta comando: $cmd" >&2; exit 1; }
done

[[ -f "$MANIFEST" ]] || { echo "falta manifest: $MANIFEST" >&2; exit 1; }

mkdir -p "$DIFF_DIR"

if [[ -n "$SCENES" ]]; then
  scene_ids=($SCENES)
else
  mapfile -t scene_ids < <(jq -r '.scenes[].id' "$MANIFEST")
fi

# Sway headless en un socket propio para no tocar la sesión actual.
export WLR_BACKENDS=headless
export WLR_RENDERER=pixman
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
SWAY_SOCKET_NAME="campilot-ui-review-$$"
SWAY_LOG="$DIFF_DIR/sway.log"
SWAY_CONF="$DIFF_DIR/sway-review.conf"

cat > "$SWAY_CONF" <<CONF
output HEADLESS-1 resolution 1920x1200
output HEADLESS-1 scale 1
xwayland disable
for_window [title="campilot"] fullscreen enable, border none
exec true
CONF

BEFORE_WL="$(ls "$XDG_RUNTIME_DIR"/wayland-* 2>/dev/null | sort || true)"
sway -c "$SWAY_CONF" >"$SWAY_LOG" 2>&1 &
SWAY_PID=$!
cleanup() {
  if kill -0 "$SWAY_PID" 2>/dev/null; then
    kill "$SWAY_PID" 2>/dev/null || true
    wait "$SWAY_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# Esperar a que sway exponga el socket IPC y el WAYLAND_DISPLAY propio.
for _ in $(seq 1 100); do
  SWAYSOCK="$(ls "$XDG_RUNTIME_DIR"/sway-ipc.*."$SWAY_PID".sock 2>/dev/null | head -1 || true)"
  WAYLAND_DISPLAY=""
  while IFS= read -r sock; do
    [[ -z "$sock" ]] && continue
    if ! grep -qx "$sock" <<<"$BEFORE_WL"; then
      WAYLAND_DISPLAY="$(basename "$sock")"
      break
    fi
  done < <(ls "$XDG_RUNTIME_DIR"/wayland-* 2>/dev/null | grep -v '\.lock$' | sort || true)
  [[ -n "$SWAYSOCK" && -n "$WAYLAND_DISPLAY" ]] && break
  sleep 0.1
done
if [[ -z "${SWAYSOCK:-}" || -z "${WAYLAND_DISPLAY:-}" ]]; then
  echo "sway no arrancó; ver $SWAY_LOG" >&2
  exit 1
fi
export SWAYSOCK WAYLAND_DISPLAY

echo "sway headless pid=$SWAY_PID sock=$SWAYSOCK display=$WAYLAND_DISPLAY"

status=0
for scene in "${scene_ids[@]}"; do
  ref="$ROOT/fixtures/ui/$scene.png"
  if [[ ! -f "$ref" ]]; then
    echo "ui-review: $scene  SKIP  falta referencia $ref"
    status=1
    continue
  fi

  # Ajustar output del headless al tamaño de la referencia.
  read -r width height < <(python3 - "$ref" <<'PY'
import struct,sys
p=sys.argv[1]
with open(p,'rb') as f: f.read(16); w,h=struct.unpack('>II',f.read(8))
print(w,h)
PY
)
  swaymsg "output HEADLESS-1 resolution ${width}x${height} scale 1" >/dev/null

  echo "ui-review: $scene  capturing ${width}x${height}..."
  actual="$DIFF_DIR/$scene.actual.png"
  diff="$DIFF_DIR/$scene.diff.png"
  rm -f "$actual" "$diff"

  CAM_UI_SCENE="$scene" cargo run -p cam-desktop --manifest-path "$ROOT/Cargo.toml" >"$DIFF_DIR/$scene.app.log" 2>&1 &
  APP_PID=$!

  # Esperar a que la ventana aparezca y se renderice.
  for _ in $(seq 1 120); do
    if swaymsg -t get_tree | jq -e '.. | .name? // empty | select(.=="campilot")' >/dev/null 2>&1; then
      break
    fi
    sleep 0.25
  done
  sleep 1.5
  grim "$actual" || true

  kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true

  if [[ ! -f "$actual" ]]; then
    echo "ui-review: $scene  FAIL  no se pudo capturar"
    status=1
    continue
  fi

  python3 - "$ref" "$actual" "$diff" "$scene" <<'PY'
import sys, struct, zlib, os
ref_p, act_p, diff_p, name = sys.argv[1:5]

def read_png(path):
    with open(path,'rb') as f: data=f.read()
    i=8; ihdr=None; idat=b''; ct=6
    while i < len(data):
        length=struct.unpack('>I',data[i:i+4])[0]
        ctype=data[i+4:i+8]
        chunk=data[i+8:i+8+length]
        i+=12+length
        if ctype==b'IHDR':
            w,h,bit,cm,comp,filt,inter=struct.unpack('>IIBBBBB',chunk)
            ihdr=(w,h); ct=cm
        elif ctype==b'IDAT':
            idat+=chunk
        elif ctype==b'IEND':
            break
    w,h=ihdr
    if ct == 2: bpp=3
    elif ct == 6: bpp=4
    else: raise SystemExit(f"color type {ct} no soportado en {path}")
    raw=zlib.decompress(idat)
    stride=w*bpp
    def paeth(a,b,c):
        p=a+b-c; pa,pb,pc=abs(p-a),abs(p-b),abs(p-c)
        return a if pa<=pb and pa<=pc else (b if pb<=pc else c)
    recon=bytearray(h*stride)
    j=0
    for row in range(h):
        fb=raw[j]; j+=1
        cur=bytearray(raw[j:j+stride]); j+=stride
        row_off=row*stride
        for k in range(stride):
            a=recon[row_off+k-bpp] if k>=bpp else 0
            b=recon[(row-1)*stride+k] if row>0 else 0
            c=recon[(row-1)*stride+k-bpp] if row>0 and k>=bpp else 0
            if fb==0: v=cur[k]
            elif fb==1: v=(cur[k]+a)&255
            elif fb==2: v=(cur[k]+b)&255
            elif fb==3: v=(cur[k]+((a+b)//2))&255
            elif fb==4: v=(cur[k]+paeth(a,b,c))&255
            else: v=cur[k]
            recon[row_off+k]=v
    if bpp == 3:
        rgba=bytearray(w*h*4)
        for i in range(w*h):
            rgba[i*4:i*4+3]=recon[i*3:i*3+3]
            rgba[i*4+3]=255
        recon=rgba
    return w,h,recon

def write_png(path,w,h,rgba):
    raw=bytearray()
    stride=w*4
    for row in range(h):
        raw.append(0)
        raw.extend(rgba[row*stride:(row+1)*stride])
    comp=zlib.compress(bytes(raw))
    def chunk(t,d):
        c=struct.pack('>I',len(d))+t+d
        return c+struct.pack('>I',zlib.crc32(t+d)&0xffffffff)
    out=b'\x89PNG\r\n\x1a\n'
    out+=chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,8,6,0,0,0))
    out+=chunk(b'IDAT',comp)
    out+=chunk(b'IEND',b'')
    with open(path,'wb') as f: f.write(out)

rw,rh,ref=read_png(ref_p)
aw,ah,act=read_png(act_p)
if (rw,rh)!=(aw,ah):
    print(f"ui-review: {name}  SIZE_MISMATCH ref={rw}x{rh} actual={aw}x{ah}")
    sys.exit(2)
total=rw*rh
diff_px=0; channel=0
diff_img=bytearray(total*4)
# Umbral por pixel para ignorar ruido de AA/render entre rutas distintas.
# Total de diff de canal (R+G+B+A) por pixel; por defecto 0 (estricto).
pix_threshold=int(os.environ.get('UI_REVIEW_PIXEL_THRESHOLD','16'))
for i in range(0,total*4,4):
    dr=abs(ref[i]-act[i]); dg=abs(ref[i+1]-act[i+1]); db=abs(ref[i+2]-act[i+2]); da=abs(ref[i+3]-act[i+3])
    p=dr+dg+db+da; channel+=p
    if p>pix_threshold:
        diff_px+=1
        alpha=max(80,min(255,p))
        diff_img[i:i+4]=bytes([255,0,0,alpha])
    else:
        g=(ref[i]+ref[i+1]+ref[i+2])//3
        diff_img[i:i+4]=bytes([g//3,g//3,g//3,255])
pct=diff_px*100.0/total
if diff_px==0:
    try: os.remove(diff_p)
    except: pass
    try: os.remove(act_p)
    except: pass
    print(f"ui-review: {name}  OK  match exacto")
    sys.exit(0)
write_png(diff_p,rw,rh,diff_img)
print(f"ui-review: {name}  DIFF  {diff_px}/{total} px ({pct:.3}%), channel_diff={channel}")
print(f"  reference: {ref_p}")
print(f"  actual:    {act_p}")
print(f"  diff:      {diff_p}")
max_pct=0.50
max_channel=total*4*2
if pct<=max_pct and channel<=max_channel:
    print(f"ui-review: {name}  OK  dentro de tolerancia ({pct:.3}%)")
    sys.exit(0)
sys.exit(1)
PY
  rc=$? || rc=$?
  if [[ $rc -ne 0 ]]; then status=1; fi
done

exit $status
