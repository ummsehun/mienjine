import * as THREE from "https://unpkg.com/three@0.170.0/build/three.module.js";
import { GLTFLoader } from "https://unpkg.com/three@0.170.0/examples/jsm/loaders/GLTFLoader.js";

const app = document.getElementById("app");
const hud = document.getElementById("hud");
const err = document.getElementById("err");

const renderer = new THREE.WebGLRenderer({ antialias: true });
renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
renderer.setSize(window.innerWidth, window.innerHeight);
app.appendChild(renderer.domElement);

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x111827);
const camera = new THREE.PerspectiveCamera(
  55,
  window.innerWidth / window.innerHeight,
  0.01,
  400,
);
camera.position.set(0, 1.2, 3.0);

const light = new THREE.DirectionalLight(0xffffff, 1.2);
light.position.set(3, 4, 2);
scene.add(new THREE.AmbientLight(0xffffff, 0.6), light);

window.addEventListener("resize", () => {
  camera.aspect = window.innerWidth / window.innerHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(window.innerWidth, window.innerHeight);
});

let mixer = null;
let actions = [];
let state = null;
const clock = new THREE.Clock();

// SyncController: encapsulates sync state to prevent race conditions
// between tick(), WebSocket callbacks, and HTTP polling.
class SyncController {
  constructor() {
    this.localClockSec = 0;
    this.lastSyncAt = performance.now();
    this.sync = { master_sec: 0, speed_factor: 1.0, sync_offset_ms: 0, playing: true, seq: 0 };
    this.ws = null;
    this.pollInterval = null;
    this.pendingSync = null;
  }

  applySync(data) {
    const master = data.master_sec + (data.sync_offset_ms || 0) / 1000.0;
    const errSec = master - this.localClockSec;
    if (Math.abs(errSec) > 0.12) {
      this.localClockSec = master;
    } else {
      this.localClockSec += errSec * 0.15;
    }
    this.sync = data;
    this.lastSyncAt = performance.now();
  }

  async fetchSync() {
    if (this.pendingSync) return this.pendingSync;
    this.pendingSync = this._doFetch().finally(() => {
      this.pendingSync = null;
    });
    return this.pendingSync;
  }

  async _doFetch() {
    const res = await fetch("/sync");
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();
    this.applySync(data);
  }

  connect() {
    if (this.ws) return;
    const proto = location.protocol === "https:" ? "wss" : "ws";
    this.ws = new WebSocket(`${proto}://${location.host}/sync`);
    this.ws.onopen = () => {
      this.lastSyncAt = performance.now();
    };
    this.ws.onmessage = (ev) => {
      try {
        this.applySync(JSON.parse(ev.data));
      } catch (e) {
        console.error("WebSocket parse error:", e);
      }
    };
    this.ws.onerror = (e) => {
      console.error("WebSocket error:", e);
    };
    this.ws.onclose = () => {
      this.ws = null;
      setTimeout(() => this.connect(), 1200);
    };
  }

  startPolling() {
    if (this.pollInterval) return;
    this.pollInterval = setInterval(() => {
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
        this.fetchSync().catch((e) => console.error("Poll error:", e));
      }
    }, 250);
  }

  stopPolling() {
    if (this.pollInterval) {
      clearInterval(this.pollInterval);
      this.pollInterval = null;
    }
  }

  advanceTime(dt) {
    const speed = Number.isFinite(this.sync.speed_factor) ? this.sync.speed_factor : 1.0;
    if (this.sync.playing !== false) {
      this.localClockSec += dt * speed;
    }
    return this.localClockSec;
  }

  staleMs() {
    return performance.now() - this.lastSyncAt;
  }
}

const syncCtrl = new SyncController();

function playAnimation(selector) {
  if (!mixer || actions.length === 0) return;
  let action = actions[0];
  if (typeof selector === "number" && selector >= 0 && selector < actions.length) {
    action = actions[selector];
  } else if (selector && selector.length > 0) {
    const named = actions.find((a) => a.getClip().name === selector);
    if (named) action = named;
  }
  actions.forEach((a) => a.stop());
  action.reset().play();
}

async function init() {
  const stateRes = await fetch("/state");
  if (!stateRes.ok) throw new Error("failed to fetch /state");
  state = await stateRes.json();

  const loader = new GLTFLoader();
  const gltf = await loader.loadAsync(state.glb_url);
  scene.add(gltf.scene);
  if (gltf.animations && gltf.animations.length > 0) {
    mixer = new THREE.AnimationMixer(gltf.scene);
    actions = gltf.animations.map((clip) => mixer.clipAction(clip));
    playAnimation(state.anim_selector);
  }
  hud.textContent = `preview | mode=${state.camera_mode} | glb=${state.glb_name} | profile=${state.sync_profile_hit ? "hit" : "miss"} | offset=${state.sync_offset_ms ?? 0}ms`;
}

function tick() {
  requestAnimationFrame(tick);
  const dt = clock.getDelta();
  const localTime = syncCtrl.advanceTime(dt);
  if (mixer) {
    const speed = Number.isFinite(syncCtrl.sync.speed_factor) ? syncCtrl.sync.speed_factor : 1.0;
    mixer.update(dt * speed);
  }
  const staleMs = syncCtrl.staleMs();
  const profile = state?.sync_profile_hit ? "hit" : "miss";
  const profileKey = state?.sync_profile_key || "none";
  const drift = Number.isFinite(state?.sync_drift_ema) ? state.sync_drift_ema.toFixed(4) : "0.0000";
  const snaps = Number.isFinite(state?.sync_hard_snap_count) ? state.sync_hard_snap_count : 0;
  hud.textContent = `preview | mode=${state?.camera_mode ?? "n/a"} | t=${localTime.toFixed(3)} | sync_seq=${syncCtrl.sync.seq} | stale=${staleMs.toFixed(0)}ms | profile=${profile} | drift=${drift} | snaps=${snaps} | key=${profileKey}`;
  renderer.render(scene, camera);
}

init()
  .then(() => {
    syncCtrl.connect();
    syncCtrl.startPolling();
    tick();
  })
  .catch((e) => {
    err.textContent = String(e);
    console.error(e);
  });
