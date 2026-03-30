use std::{fs, path::PathBuf};

pub(super) const EMBED_INDEX_HTML: &str = r#"<!doctype html>
<html>
<head>
  <meta charset=\"utf-8\" />
  <title>Terminal Miku 3D Preview</title>
  <style>
    html,body,#app{margin:0;padding:0;width:100%;height:100%;background:#111827;color:#d1d5db;font-family:ui-sans-serif,system-ui}
    #hud{position:fixed;left:12px;top:12px;background:rgba(0,0,0,.45);padding:8px 10px;border-radius:8px;font-size:12px;line-height:1.4}
    #err{position:fixed;left:12px;bottom:12px;color:#fecaca}
  </style>
</head>
<body>
  <div id=\"app\"></div>
  <div id=\"hud\">loading...</div>
  <div id=\"err\"></div>
  <a id=\"probe-link\" href=\"/mmd-probe\" style=\"position:fixed;right:12px;top:12px;color:#93c5fd;text-decoration:none;background:rgba(0,0,0,.45);padding:7px 10px;border-radius:8px;font-size:12px\">MMD probe</a>
  <script type=\"module\" src=\"/app.js\"></script>
</body>
</html>
"#;

pub(super) const EMBED_APP_JS: &str = r#"import * as THREE from 'https://unpkg.com/three@0.170.0/build/three.module.js';
import { GLTFLoader } from 'https://unpkg.com/three@0.170.0/examples/jsm/loaders/GLTFLoader.js';

const app = document.getElementById('app');
const hud = document.getElementById('hud');
const err = document.getElementById('err');
const renderer = new THREE.WebGLRenderer({ antialias: true });
renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
renderer.setSize(window.innerWidth, window.innerHeight);
app.appendChild(renderer.domElement);
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x111827);
const camera = new THREE.PerspectiveCamera(55, window.innerWidth / window.innerHeight, 0.01, 400);
camera.position.set(0, 1.2, 3.0);
const light = new THREE.DirectionalLight(0xffffff, 1.2);
light.position.set(3, 4, 2);
scene.add(new THREE.AmbientLight(0xffffff, 0.6), light);
const clock = new THREE.Clock();
let mixer = null;
let actions = [];
let state = null;

window.addEventListener('resize', () => {
  camera.aspect = window.innerWidth / window.innerHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(window.innerWidth, window.innerHeight);
});

async function loadState() {
  const res = await fetch('/state');
  if (!res.ok) throw new Error('state fetch failed');
  return await res.json();
}

function playAnimation(indexOrName) {
  if (!mixer || actions.length === 0) return;
  let action = actions[0];
  if (typeof indexOrName === 'number' && indexOrName >= 0 && indexOrName < actions.length) {
    action = actions[indexOrName];
  } else if (typeof indexOrName === 'string') {
    const found = actions.find(a => a.getClip().name === indexOrName);
    if (found) action = found;
  }
  actions.forEach(a => a.stop());
  action.reset().play();
}

async function init() {
  state = await loadState();
  const loader = new GLTFLoader();
  const gltf = await loader.loadAsync(state.glb_url);
  scene.add(gltf.scene);
  if (gltf.animations && gltf.animations.length > 0) {
    mixer = new THREE.AnimationMixer(gltf.scene);
    actions = gltf.animations.map(clip => mixer.clipAction(clip));
    playAnimation(state.anim_selector ?? 0);
  }
  hud.textContent = `preview | mode=${state.camera_mode} | glb=${state.glb_name} | profile=${state.sync_profile_hit ? 'hit' : 'miss'} | offset=${state.sync_offset_ms ?? 0}ms`;
}

let sync = { master_sec: 0, speed_factor: 1.0, sync_offset_ms: 0, playing: true, seq: 0 };
let localClockSec = 0;
let lastSyncAt = performance.now();
let ws = null;

function applySync(data) {
  const master = data.master_sec + (data.sync_offset_ms || 0) / 1000.0;
  const errSec = master - localClockSec;
  if (Math.abs(errSec) > 0.12) {
    localClockSec = master;
  } else {
    localClockSec += errSec * 0.15;
  }
  sync = data;
  lastSyncAt = performance.now();
}

function connectSyncSocket() {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  const url = `${proto}://${location.host}/sync`;
  ws = new WebSocket(url);
  ws.onopen = () => {
    lastSyncAt = performance.now();
  };
  ws.onmessage = (ev) => {
    try {
      applySync(JSON.parse(ev.data));
    } catch (_) {}
  };
  ws.onerror = () => {};
  ws.onclose = () => {
    setTimeout(connectSyncSocket, 1200);
  };
}

async function fallbackPoll() {
  if (ws && ws.readyState === WebSocket.OPEN) return;
  try {
    const res = await fetch('/sync');
    if (!res.ok) return;
    const data = await res.json();
    applySync(data);
  } catch (_) {}
}
setInterval(fallbackPoll, 250);

function tick() {
  requestAnimationFrame(tick);
  const dt = clock.getDelta();
  const speed = Number.isFinite(sync.speed_factor) ? sync.speed_factor : 1.0;
  if (sync.playing !== false) {
    localClockSec += dt * speed;
  }
  if (mixer) {
    mixer.update(dt * speed);
  }
  const staleMs = performance.now() - lastSyncAt;
  const profile = state?.sync_profile_hit ? 'hit' : 'miss';
  const profileKey = state?.sync_profile_key || 'none';
  const drift = Number.isFinite(state?.sync_drift_ema) ? state.sync_drift_ema.toFixed(4) : '0.0000';
  const snaps = Number.isFinite(state?.sync_hard_snap_count) ? state.sync_hard_snap_count : 0;
  hud.textContent = `preview | mode=${state?.camera_mode ?? 'n/a'} | t=${localClockSec.toFixed(3)} | sync_seq=${sync.seq} | stale=${staleMs.toFixed(0)}ms | profile=${profile} | drift=${drift} | snaps=${snaps} | key=${profileKey}`;
  renderer.render(scene, camera);
}

init().then(() => {
  connectSyncSocket();
  tick();
}).catch((e) => { err.textContent = String(e); console.error(e); });
"#;

pub(super) const EMBED_MMD_PROBE_HTML: &str = r#"<!doctype html>
<html>
<head>
  <meta charset=\"utf-8\" />
  <title>Terminal Miku 3D MMD Probe</title>
  <style>
    html,body{margin:0;padding:0;background:#0f172a;color:#e2e8f0;font-family:ui-monospace,SFMono-Regular,Menlo,monospace}
    main{max-width:960px;margin:24px auto;padding:0 16px}
    pre{background:#111827;padding:12px;border-radius:8px;overflow:auto}
    a{color:#93c5fd}
  </style>
</head>
<body>
  <main>
    <h1>MMD Bridge Probe</h1>
    <p>GLTF 경로와 PMX/VMD 브릿지 점검 정보를 보여줍니다. 런타임 직접 파싱이 아니라 웹 브릿지 검증용입니다.</p>
    <pre id=\"probe\">loading...</pre>
    <p><a href=\"/\">Back to preview</a></p>
  </main>
  <script type=\"module\" src=\"/mmd_probe.js\"></script>
</body>
</html>
"#;

pub(super) const EMBED_MMD_PROBE_JS: &str = r#"const out = document.getElementById('probe');

async function run() {
  const stateRes = await fetch('/state');
  if (!stateRes.ok) throw new Error('failed to fetch /state');
  const state = await stateRes.json();

  const lines = [
    `glb: ${state.glb_name}`,
    `camera_vmd: ${state.camera_vmd_name || 'none'}`,
    `sync_offset_ms: ${state.sync_offset_ms ?? 0}`,
    `sync_profile_key: ${state.sync_profile_key || 'none'}`,
    `sync_profile_hit: ${state.sync_profile_hit === true}`,
    '',
    'Web import paths:',
    '- GLTFLoader: native GLB path',
    '- MMDLoader: PMX + VMD staging path (for conversion parity checks)',
    '- Runtime policy: PMX/VMD direct runtime parsing is out-of-scope in this probe',
  ];
  out.textContent = lines.join('\n');
}

run().catch((err) => {
  out.textContent = String(err);
  console.error(err);
});
"#;

pub(super) fn load_preview_file(name: &str) -> Option<String> {
    let path = PathBuf::from("preview-web").join(name);
    fs::read_to_string(path).ok()
}
