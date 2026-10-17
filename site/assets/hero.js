// Hero animation: OpenCue's agents as soft 3D characters floating around the headline. Their eyes
// follow the pointer, they blink, and they hop when clicked. Falls back to nothing without WebGL;
// renders one still frame when the visitor prefers reduced motion.
import * as THREE from "three";
import { RoundedBoxGeometry } from "three/addons/geometries/RoundedBoxGeometry.js";
import { RoomEnvironment } from "three/addons/environments/RoomEnvironment.js";

const canvas = document.getElementById("hero-canvas");
const stage = document.querySelector(".hero-stage");

function start() {
  if (!canvas || !stage) return;
  let renderer;
  try {
    renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, powerPreference: "low-power" });
  } catch {
    canvas.remove();
    return;
  }
  const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.NeutralToneMapping;
  renderer.toneMappingExposure = 1.0;

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(32, 1, 0.1, 100);
  camera.position.set(0, 0, 16);

  const pmrem = new THREE.PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
  scene.environmentIntensity = 0.75;
  scene.add(new THREE.HemisphereLight(0xffffff, 0xdfe8ff, 0.6));
  const key = new THREE.DirectionalLight(0xffffff, 1.4);
  key.position.set(4, 6, 8);
  scene.add(key);

  // ---------- shapes ----------
  function starShape(outer, inner) {
    const s = new THREE.Shape();
    for (let k = 0; k < 10; k++) {
      const r = k % 2 ? inner : outer;
      const a = (Math.PI / 5) * k + Math.PI / 2;
      const x = r * Math.cos(a);
      const y = r * Math.sin(a);
      if (k === 0) s.moveTo(x, y);
      else s.lineTo(x, y);
    }
    s.closePath();
    return s;
  }
  function heartShape() {
    const s = new THREE.Shape();
    s.moveTo(0, -0.95);
    s.bezierCurveTo(-0.35, -0.65, -1.15, -0.15, -1.1, 0.35);
    s.bezierCurveTo(-1.05, 0.85, -0.45, 1.05, 0, 0.62);
    s.bezierCurveTo(0.45, 1.05, 1.05, 0.85, 1.1, 0.35);
    s.bezierCurveTo(1.15, -0.15, 0.35, -0.65, 0, -0.95);
    return s;
  }
  function extruded(shape, depth) {
    const g = new THREE.ExtrudeGeometry(shape, { depth, bevelEnabled: true, bevelThickness: 0.28, bevelSize: 0.22, bevelSegments: 10, curveSegments: 32 });
    g.center();
    return g;
  }
  function drop() {
    const g = new THREE.SphereGeometry(1, 64, 64);
    const p = g.attributes.position;
    for (let i = 0; i < p.count; i++) {
      let x = p.getX(i), y = p.getY(i), z = p.getZ(i);
      if (y > 0) {
        const k = 1 - Math.pow(y, 1.5) * 0.92;
        x *= k;
        z *= k;
        y *= 1.45;
      }
      p.setXYZ(i, x, y - 0.12, z);
    }
    g.computeVertexNormals();
    return g;
  }
  const GEOMETRY = {
    bubble: () => new THREE.SphereGeometry(1, 64, 64).scale(1.05, 1, 0.92),
    square: () => new RoundedBoxGeometry(1.75, 1.75, 1.35, 8, 0.48),
    capsule: () => new THREE.CapsuleGeometry(0.72, 0.75, 16, 48),
    drop,
    star: () => extruded(starShape(1.15, 0.58), 0.45),
    heart: () => extruded(heartShape(), 0.5),
  };

  // Same palette as the app's avatars.
  const CAST = [
    { shape: "bubble", color: "#2f86eb", at: [-0.8, 0.42], size: 1.15 },
    { shape: "star", color: "#f7c32f", at: [0.8, 0.5], size: 0.95 },
    { shape: "heart", color: "#f04848", at: [0.66, -0.42], size: 0.9 },
    { shape: "drop", color: "#f6ad2b", at: [-0.62, -0.5], size: 0.85 },
    { shape: "square", color: "#8e8e93", at: [-0.95, -0.02], size: 0.7 },
    { shape: "capsule", color: "#34b27b", at: [0.96, 0.02], size: 0.7 },
    { shape: "bubble", color: "#a78bfa", at: [0.36, 0.74], size: 0.55 },
    { shape: "square", color: "#5ec2f2", at: [-0.4, 0.74], size: 0.5 },
  ];

  const whiteMat = new THREE.MeshPhysicalMaterial({ color: 0xffffff, roughness: 0.15, clearcoat: 1, clearcoatRoughness: 0.1 });
  const pupilMat = new THREE.MeshStandardMaterial({ color: 0x1d1d1f, roughness: 0.25 });
  const eyeGeo = new THREE.SphereGeometry(1, 32, 32);
  const ray = new THREE.Raycaster();

  const characters = CAST.map((c, i) => {
    const group = new THREE.Group();
    const body = new THREE.Mesh(
      GEOMETRY[c.shape](),
      new THREE.MeshPhysicalMaterial({ color: c.color, roughness: 0.42, clearcoat: 0.6, clearcoatRoughness: 0.3 }),
    );
    group.add(body);

    // Put the eyes on the front surface: cast a ray at the body from the front.
    body.updateMatrixWorld();
    const eyeY = c.shape === "drop" ? -0.05 : c.shape === "heart" ? 0.12 : c.shape === "star" ? 0.05 : 0.12;
    const eyes = [-0.3, 0.3].map((ex) => {
      ray.set(new THREE.Vector3(ex, eyeY, 5), new THREE.Vector3(0, 0, -1));
      const hit = ray.intersectObject(body)[0];
      const z = hit ? hit.point.z : 0.9;
      const eye = new THREE.Group();
      eye.position.set(ex, eyeY, z - 0.04);
      const white = new THREE.Mesh(eyeGeo, whiteMat);
      white.scale.setScalar(0.2);
      const pupil = new THREE.Mesh(eyeGeo, pupilMat);
      pupil.scale.setScalar(0.105);
      pupil.position.z = 0.13;
      eye.add(white, pupil);
      group.add(eye);
      return { eye, pupil };
    });

    scene.add(group);
    return {
      ...c,
      group,
      eyes,
      phase: i * 1.7,
      speed: 0.6 + (i % 3) * 0.17,
      spin: 0,
      hop: 0,
      appear: 0,
      delay: 0.12 * i,
      nextBlink: 1.5 + Math.random() * 3,
      blink: 0,
    };
  });

  // ---------- layout ----------
  let viewW = 1, viewH = 1, narrow = false;
  function layout() {
    const w = stage.clientWidth;
    const h = stage.clientHeight;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    viewH = 2 * Math.tan(THREE.MathUtils.degToRad(camera.fov / 2)) * camera.position.z;
    viewW = viewH * camera.aspect;
    narrow = w < 760;
    characters.forEach((c, i) => {
      // On phones the cast moves to the top and bottom edges and the smallest ones sit out.
      const [nx, ny] = narrow ? [[-0.72, 0.8], [0.72, 0.82], [0.75, -0.86], [-0.7, -0.88], [-0.15, 0.93], [0.2, -0.95], [0, 0], [0, 0]][i] : c.at;
      c.home = new THREE.Vector3((nx * viewW) / 2, (ny * viewH) / 2, 0);
      c.scale = (narrow ? 0.62 : 1) * c.size * Math.min(1.25, Math.max(0.75, viewW / 26));
      c.group.visible = !(narrow && i >= 6);
    });
  }
  new ResizeObserver(layout).observe(stage);
  layout();

  // ---------- input ----------
  const pointer = new THREE.Vector2(0, 0);
  const pointerWorld = new THREE.Vector3(0, 0, 4);
  let lastMove = 0;
  window.addEventListener(
    "pointermove",
    (e) => {
      const r = canvas.getBoundingClientRect();
      pointer.set(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
      pointerWorld.set((pointer.x * viewW) / 2, (pointer.y * viewH) / 2, 4);
      lastMove = performance.now();
      if (reduceMotion) render(0);
    },
    { passive: true },
  );
  function pick(e) {
    const r = canvas.getBoundingClientRect();
    const p = new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
    ray.setFromCamera(p, camera);
    const hits = ray.intersectObjects(characters.filter((c) => c.group.visible).map((c) => c.group), true);
    if (!hits.length) return null;
    return characters.find((c) => c.group === hits[0].object.parent || c.group === hits[0].object.parent?.parent);
  }
  stage.addEventListener("click", (e) => {
    if (e.target.closest("a, button, code")) return;
    const c = pick(e);
    if (c && !reduceMotion) {
      c.hop = 1;
      c.spin = 1;
    }
  });
  stage.addEventListener("pointermove", (e) => {
    stage.style.cursor = !e.target.closest("a, button, code, h1, p") && pick(e) ? "pointer" : "";
  });

  // ---------- animation ----------
  const clock = new THREE.Clock();
  const tmp = new THREE.Vector3();
  const easeOutBack = (t) => 1 + 2.4 * Math.pow(t - 1, 3) + 1.4 * Math.pow(t - 1, 2);

  function render(dt) {
    const t = clock.elapsedTime;
    const idle = performance.now() - lastMove > 4000;
    // Without a pointer, let the eyes wander slowly.
    const look = idle ? tmp.set(Math.sin(t * 0.5) * viewW * 0.3, Math.cos(t * 0.37) * viewH * 0.2, 4) : pointerWorld;
    const scrollLift = Math.min(window.scrollY / Math.max(stage.clientHeight, 1), 1);

    for (const c of characters) {
      if (!c.group.visible) continue;
      c.appear = reduceMotion ? 1 : Math.min(1, c.appear + Math.max(0, dt) / 0.9 * (t > c.delay ? 1 : 0));
      const s = c.scale * easeOutBack(c.appear);
      c.group.scale.setScalar(Math.max(s, 0.0001));

      const bob = reduceMotion ? 0 : Math.sin(t * c.speed + c.phase) * 0.22;
      c.hop = Math.max(0, c.hop - dt * 1.6);
      const hopY = Math.sin((1 - c.hop) * Math.PI) * (c.hop > 0 ? 1.1 : 0) * c.scale;
      c.group.position.set(c.home.x, c.home.y + bob + hopY + scrollLift * 1.5 * (c.home.y > 0 ? 1 : -1), c.home.z);

      // Turn a little toward the pointer, plus a slow sway and the hop spin.
      c.spin = Math.max(0, c.spin - dt * 1.25);
      const toward = Math.atan2(look.x - c.group.position.x, 9) * 0.6;
      const tilt = -Math.atan2(look.y - c.group.position.y, 12) * 0.5;
      c.yaw = (c.yaw ?? 0) + (toward + Math.sin(t * 0.4 + c.phase) * 0.12 - (c.yaw ?? 0)) * 0.08;
      const spinAngle = c.spin > 0 ? (1 - Math.pow(c.spin, 2)) * Math.PI * 2 : 0;
      c.group.rotation.y = c.yaw + spinAngle;
      c.group.rotation.x += (tilt - c.group.rotation.x) * 0.08;
      c.group.rotation.z = Math.sin(t * 0.3 + c.phase) * 0.06;

      // Pupils slide toward the pointer inside each eye.
      for (const { eye, pupil } of c.eyes) {
        const local = c.group.worldToLocal(look.clone()).sub(eye.position);
        local.z = Math.max(local.z, 0.5);
        local.normalize();
        pupil.position.x += (local.x * 0.085 - pupil.position.x) * 0.25;
        pupil.position.y += (local.y * 0.085 - pupil.position.y) * 0.25;
      }

      // Blink now and then.
      if (!reduceMotion) {
        c.nextBlink -= dt;
        if (c.nextBlink <= 0) {
          c.blink = 1;
          c.nextBlink = 2 + Math.random() * 5;
        }
        c.blink = Math.max(0, c.blink - dt * 7);
        const lid = c.blink > 0 ? Math.max(0.08, Math.abs(c.blink * 2 - 1)) : 1;
        for (const { eye } of c.eyes) eye.scale.y = lid;
      }
    }
    renderer.render(scene, camera);
  }

  let visible = true;
  new IntersectionObserver(([e]) => (visible = e.isIntersecting)).observe(stage);

  if (reduceMotion) {
    render(0);
    return;
  }
  renderer.setAnimationLoop(() => {
    const dt = Math.min(clock.getDelta(), 0.05);
    if (visible && !document.hidden) render(dt);
  });
  canvas.classList.add("ready");
}

start();
