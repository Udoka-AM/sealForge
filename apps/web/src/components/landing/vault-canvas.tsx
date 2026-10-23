"use client";

import { useEffect, useRef } from "react";

/**
 * The hero centrepiece: the sealForge mark as a 3D ingot, drawn as a wireframe on a 2D canvas.
 * Gold particles (sealed bids) stream in from every side and vanish into it; a tilted ring of
 * block ticks orbits it. No WebGL and no library: 18 hexagon edges, the extruded S and a few
 * hundred points, projected by hand. It pauses off-screen and in hidden tabs, and draws one still
 * frame when the user prefers reduced motion.
 */

type V3 = [number, number, number];

// The mark on its 64-unit grid (lib/brand.ts), centred on the origin, y up.
const HEX_2D: [number, number][] = [
  [32, 4],
  [56.2, 18],
  [56.2, 46],
  [32, 60],
  [7.8, 46],
  [7.8, 18],
];
const S_2D: [number, number][] = [
  [21, 20],
  [40, 20],
  [43, 23],
  [43, 26],
  [30, 26],
  [43, 38],
  [43, 44],
  [24, 44],
  [21, 41],
  [21, 38],
  [34, 38],
  [21, 26],
];
const DEPTH = 14;
const toV = ([x, y]: [number, number], z: number): V3 => [x - 32, 32 - y, z];

const HEX_FRONT = HEX_2D.map((p) => toV(p, DEPTH / 2));
const HEX_BACK = HEX_2D.map((p) => toV(p, -DEPTH / 2));
const S_FRONT = S_2D.map((p) => toV(p, DEPTH / 2 + 0.01));
const S_BACK = S_2D.map((p) => toV(p, -DEPTH / 2 - 0.01));

interface Particle {
  dir: V3; // unit vector of the spawn direction
  r0: number; // spawn radius
  t: number; // 0..1 progress
  speed: number; // progress per second
  swirl: number; // radians of twist over the flight
  size: number;
}

const rand = (a: number, b: number) => a + Math.random() * (b - a);

function spawn(p?: Particle): Particle {
  const u = rand(-1, 1);
  const th = rand(0, Math.PI * 2);
  const s = Math.sqrt(1 - u * u);
  const q: Particle = p ?? ({} as Particle);
  q.dir = [s * Math.cos(th), u * 0.8, s * Math.sin(th)];
  q.r0 = rand(70, 120);
  q.t = 0;
  q.speed = rand(0.14, 0.32);
  q.swirl = rand(-0.9, 0.9);
  q.size = rand(0.6, 1.8);
  return q;
}

function rotate([x, y, z]: V3, ry: number, rx: number): V3 {
  const cy = Math.cos(ry);
  const sy = Math.sin(ry);
  const x1 = x * cy + z * sy;
  const z1 = -x * sy + z * cy;
  const cx = Math.cos(rx);
  const sx = Math.sin(rx);
  return [x1, y * cx - z1 * sx, y * sx + z1 * cx];
}

export function VaultCanvas({ className = "" }: { className?: string }) {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const small = window.matchMedia("(max-width: 767px)").matches;
    const gold = getComputedStyle(document.documentElement).getPropertyValue("--primary").trim();
    const rgb = hexToRgb(gold) ?? [244, 183, 40];
    const c = (a: number) => `rgba(${rgb[0]},${rgb[1]},${rgb[2]},${a})`;

    const particles = Array.from({ length: small ? 110 : 240 }, () => {
      const p = spawn();
      p.t = Math.random(); // start mid-flight so the first frame is already full
      return p;
    });

    let w = 0;
    let h = 0;
    let scale = 1;
    const resize = () => {
      const dpr = Math.min(2, window.devicePixelRatio || 1);
      const rect = canvas.getBoundingClientRect();
      w = rect.width;
      h = rect.height;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      scale = Math.min(w, h) / 150;
    };
    resize();
    const ro = new ResizeObserver(resize);
    ro.observe(canvas);

    // Pointer tilt, eased.
    let tx = 0;
    let ty = 0;
    let px = 0;
    let py = 0;
    const onPointer = (e: PointerEvent) => {
      tx = (e.clientX / window.innerWidth - 0.5) * 0.5;
      ty = (e.clientY / window.innerHeight - 0.5) * 0.35;
    };
    if (!reduce) window.addEventListener("pointermove", onPointer, { passive: true });

    let pulse = 0; // brightens the mark when a bid lands

    const project = (v: V3): [number, number, number] => {
      const f = 260;
      const k = f / (f - v[2]);
      return [w / 2 + v[0] * k * scale, h / 2 - v[1] * k * scale, k];
    };

    const draw = (time: number, dt: number) => {
      ctx.clearRect(0, 0, w, h);
      px += (tx - px) * 0.05;
      py += (ty - py) * 0.05;
      const ry = Math.sin(time * 0.00025) * 0.55 + px;
      const rx = -0.18 + Math.sin(time * 0.0002) * 0.08 + py;
      pulse = Math.max(0, pulse - dt * 1.6);

      // The block ring: a tilted ellipse with ticks, turning slowly.
      const ringR = 46;
      ctx.lineWidth = 1;
      ctx.strokeStyle = c(0.16);
      ctx.beginPath();
      for (let i = 0; i <= 96; i++) {
        const a = (i / 96) * Math.PI * 2;
        const [x, y] = project(
          rotate([Math.cos(a) * ringR, 0, Math.sin(a) * ringR], ry * 0.4, rx + 1.2),
        );
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
      for (let i = 0; i < 24; i++) {
        const a = (i / 24) * Math.PI * 2 + time * 0.00012;
        const v = rotate([Math.cos(a) * ringR, 0, Math.sin(a) * ringR], ry * 0.4, rx + 1.2);
        const [x, y, k] = project(v);
        ctx.fillStyle = c(i % 6 === 0 ? 0.75 : 0.3 * k);
        ctx.fillRect(x - 1, y - 1, i % 6 === 0 ? 2.6 : 1.6, i % 6 === 0 ? 2.6 : 1.6);
      }

      // Particles behind the mark first, then the mark, then those in front.
      const front: [number, number, number, number, number, number][] = [];
      for (const p of particles) {
        if (!reduce) {
          p.t += p.speed * dt;
          if (p.t >= 1) {
            pulse = Math.min(1, pulse + 0.18);
            spawn(p);
          }
        }
        const ease = 1 - (1 - p.t) ** 2;
        const r = p.r0 * (1 - ease) + 6 * ease;
        const tw = p.swirl * ease;
        const d = rotate(p.dir, tw, 0);
        const v = rotate([d[0] * r, d[1] * r, d[2] * r], ry, rx);
        const prevR = r + 9 * (1 - ease) + 2;
        const pv = rotate([d[0] * prevR, d[1] * prevR, d[2] * prevR], ry, rx);
        const [x, y, k] = project(v);
        const [x0, y0] = project(pv);
        const a = Math.min(1, p.t * 4) * Math.min(1, (1 - p.t) * 5);
        const item: [number, number, number, number, number, number] = [
          x,
          y,
          x0,
          y0,
          k,
          a * p.size,
        ];
        if (v[2] > 0) front.push(item);
        else dot(item, 0.55);
      }

      // The ingot.
      const glow = 0.55 + pulse * 0.35;
      ctx.lineJoin = "round";
      const poly = (pts: V3[], stroke: string, fill?: string) => {
        ctx.beginPath();
        pts.forEach((v, i) => {
          const [x, y] = project(rotate(v, ry, rx));
          if (i === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        });
        ctx.closePath();
        if (fill) {
          ctx.fillStyle = fill;
          ctx.fill();
        }
        ctx.strokeStyle = stroke;
        ctx.stroke();
      };
      ctx.lineWidth = 1;
      poly(HEX_BACK, c(0.22));
      poly(S_BACK, c(0.14));
      ctx.beginPath();
      for (let i = 0; i < 6; i++) {
        const [x1, y1] = project(rotate(HEX_FRONT[i] as V3, ry, rx));
        const [x2, y2] = project(rotate(HEX_BACK[i] as V3, ry, rx));
        ctx.moveTo(x1, y1);
        ctx.lineTo(x2, y2);
      }
      for (let i = 0; i < S_FRONT.length; i++) {
        const [x1, y1] = project(rotate(S_FRONT[i] as V3, ry, rx));
        const [x2, y2] = project(rotate(S_BACK[i] as V3, ry, rx));
        ctx.moveTo(x1, y1);
        ctx.lineTo(x2, y2);
      }
      ctx.strokeStyle = c(0.28);
      ctx.stroke();
      ctx.lineWidth = 1.4;
      poly(HEX_FRONT, c(glow), c(0.05 + pulse * 0.05));
      ctx.lineWidth = 1.2;
      poly(S_FRONT, c(glow * 0.95));

      for (const item of front) dot(item, 1);
    };

    function dot(
      [x, y, x0, y0, k, s]: [number, number, number, number, number, number],
      dim: number,
    ) {
      if (s <= 0.01 || !ctx) return;
      ctx.strokeStyle = c(0.18 * s * dim);
      ctx.lineWidth = 0.8;
      ctx.beginPath();
      ctx.moveTo(x0, y0);
      ctx.lineTo(x, y);
      ctx.stroke();
      ctx.fillStyle = c(Math.min(1, 0.85 * s) * dim);
      const rad = Math.max(0.6, 1.3 * k * s);
      ctx.beginPath();
      ctx.arc(x, y, rad, 0, Math.PI * 2);
      ctx.fill();
    }

    let raf = 0;
    let last = performance.now();
    let visible = true;
    const loop = (now: number) => {
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;
      draw(now, dt);
      raf = requestAnimationFrame(loop);
    };
    const start = () => {
      if (raf || reduce || !visible || document.hidden) return;
      last = performance.now();
      raf = requestAnimationFrame(loop);
    };
    const stop = () => {
      cancelAnimationFrame(raf);
      raf = 0;
    };
    const io = new IntersectionObserver(([e]) => {
      visible = e?.isIntersecting ?? true;
      if (visible) start();
      else stop();
    });
    io.observe(canvas);
    const onVis = () => (document.hidden ? stop() : start());
    document.addEventListener("visibilitychange", onVis);

    if (reduce) draw(0, 0);
    else start();

    return () => {
      stop();
      io.disconnect();
      ro.disconnect();
      window.removeEventListener("pointermove", onPointer);
      document.removeEventListener("visibilitychange", onVis);
    };
  }, []);

  return (
    <div aria-hidden="true" className={className}>
      <canvas ref={ref} className="block size-full" />
    </div>
  );
}

function hexToRgb(hex: string): [number, number, number] | null {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  return m
    ? [
        Number.parseInt(m[1] ?? "0", 16),
        Number.parseInt(m[2] ?? "0", 16),
        Number.parseInt(m[3] ?? "0", 16),
      ]
    : null;
}
