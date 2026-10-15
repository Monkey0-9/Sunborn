"use client";

import React, { useEffect, useRef, useState } from "react";

interface NodePoint {
  x: number;
  y: number;
  z: number;
  vx: number;
  vy: number;
  vz: number;
  baseRadius: number;
  tier: "exact" | "semantic" | "execution" | "origin";
  energy: number;
  label?: string;
}

interface PulseWave {
  x: number;
  y: number;
  radius: number;
  maxRadius: number;
  opacity: number;
  color: string;
}

export function CanvasManifold({
  hitRate = 95.1,
  p99LatencyUs = 124,
  throughputReqs = 10559,
}: {
  hitRate?: number;
  p99LatencyUs?: number;
  throughputReqs?: number;
}) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [hudState, setHudState] = useState({
    fps: 60,
    hoveredTier: "Tier 1: Exact",
    waveCount: 0,
    manifoldAngle: 0,
  });

  const mouseRef = useRef({ x: 0, y: 0, isHovering: false });

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let animationFrameId: number;
    let width = (canvas.width = canvas.parentElement?.clientWidth || 800);
    let height = (canvas.height = canvas.parentElement?.clientHeight || 450);

    const handleResize = () => {
      if (!canvas || !canvas.parentElement) return;
      width = canvas.width = canvas.parentElement.clientWidth;
      height = canvas.height = canvas.parentElement.clientHeight;
    };
    window.addEventListener("resize", handleResize);

    // Initialize 3D Vector Manifold Particles
    const nodes: NodePoint[] = [];
    const tiers: ("exact" | "semantic" | "execution" | "origin")[] = [
      "exact",
      "exact",
      "semantic",
      "semantic",
      "execution",
      "origin",
    ];

    const labels = [
      "W-TinyLFU",
      "DragonflyDB",
      "Qdrant HNSW",
      "Conformal Risk α≤0.001",
      "Merkle DAG Attest",
      "PostgreSQL Outbox",
      "Radix KV Route",
      "BLAKE3 10-D Identity",
    ];

    for (let i = 0; i < 72; i++) {
      const theta = Math.random() * Math.PI * 2;
      const phi = Math.acos(Math.random() * 2 - 1);
      const r = 80 + Math.random() * 140;
      nodes.push({
        x: r * Math.sin(phi) * Math.cos(theta),
        y: r * Math.sin(phi) * Math.sin(theta),
        z: r * Math.cos(phi),
        vx: (Math.random() - 0.5) * 0.3,
        vy: (Math.random() - 0.5) * 0.3,
        vz: (Math.random() - 0.5) * 0.3,
        baseRadius: Math.random() * 2.5 + 1.5,
        tier: tiers[Math.floor(Math.random() * tiers.length)],
        energy: Math.random(),
        label: i < labels.length ? labels[i] : undefined,
      });
    }

    const waves: PulseWave[] = [];
    let rotX = 0;
    let rotY = 0;
    let frameCount = 0;
    let lastTime = performance.now();

    const triggerPulse = (x: number, y: number, color = "#22d3ee") => {
      waves.push({
        x,
        y,
        radius: 5,
        maxRadius: Math.max(width, height) * 0.7,
        opacity: 0.9,
        color,
      });
      setHudState((s) => ({ ...s, waveCount: s.waveCount + 1 }));
    };

    // Auto trigger occasional pulse wave
    const pulseInterval = setInterval(() => {
      triggerPulse(
        width / 2 + (Math.random() - 0.5) * 80,
        height / 2 + (Math.random() - 0.5) * 60,
        Math.random() > 0.3 ? "#06b6d4" : "#8b5cf6"
      );
    }, 3500);

    const onPointerMove = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      mouseRef.current.x = e.clientX - rect.left;
      mouseRef.current.y = e.clientY - rect.top;
      mouseRef.current.isHovering = true;
    };

    const onPointerLeave = () => {
      mouseRef.current.isHovering = false;
    };

    const onClick = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      triggerPulse(e.clientX - rect.left, e.clientY - rect.top, "#34d399");
    };

    canvas.addEventListener("mousemove", onPointerMove);
    canvas.addEventListener("mouseleave", onPointerLeave);
    canvas.addEventListener("click", onClick);

    // Render Loop
    const render = (time: number) => {
      frameCount++;
      if (time - lastTime >= 1000) {
        setHudState((s) => ({ ...s, fps: frameCount }));
        frameCount = 0;
        lastTime = time;
      }

      ctx.clearRect(0, 0, width, height);

      // Subtle Background Grid
      ctx.strokeStyle = "rgba(255, 255, 255, 0.02)";
      ctx.lineWidth = 1;
      const gridSize = 40;
      for (let x = 0; x < width; x += gridSize) {
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, height);
        ctx.stroke();
      }
      for (let y = 0; y < height; y += gridSize) {
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(width, y);
        ctx.stroke();
      }

      // Rotate camera
      if (mouseRef.current.isHovering) {
        const targetRotY = (mouseRef.current.x / width - 0.5) * 1.5;
        const targetRotX = (mouseRef.current.y / height - 0.5) * -1.2;
        rotY += (targetRotY - rotY) * 0.05;
        rotX += (targetRotX - rotX) * 0.05;
      } else {
        rotY += 0.003;
        rotX = Math.sin(time * 0.0008) * 0.2;
      }

      const cx = width / 2;
      const cy = height / 2;
      const fov = 350;

      // Draw Horizon Ring (Exact match boundary)
      ctx.beginPath();
      ctx.ellipse(cx, cy, 140, 45, rotY * 0.5, 0, Math.PI * 2);
      ctx.strokeStyle = "rgba(6, 182, 212, 0.15)";
      ctx.setLineDash([4, 6]);
      ctx.lineWidth = 1.5;
      ctx.stroke();
      ctx.setLineDash([]);

      // Draw Inner Ring (Risk Boundary)
      ctx.beginPath();
      ctx.ellipse(cx, cy, 80, 26, -rotY * 0.3, 0, Math.PI * 2);
      ctx.strokeStyle = "rgba(139, 92, 246, 0.2)";
      ctx.setLineDash([2, 4]);
      ctx.stroke();
      ctx.setLineDash([]);

      // Project Nodes
      const cosY = Math.cos(rotY);
      const sinY = Math.sin(rotY);
      const cosX = Math.cos(rotX);
      const sinX = Math.sin(rotX);

      const projected = nodes.map((n) => {
        // Rotate around Y
        const x1 = n.x * cosY - n.z * sinY;
        const z1 = n.z * cosY + n.x * sinY;

        // Rotate around X
        const y2 = n.y * cosX - z1 * sinX;
        const z2 = z1 * cosX + n.y * sinX;

        // Perspective
        const scale = fov / (fov + z2 + 250);
        const px = cx + x1 * scale;
        const py = cy + y2 * scale;

        return { px, py, pz: z2, scale, node: n };
      });

      // Sort by depth
      projected.sort((a, b) => b.pz - a.pz);

      // Draw Connection Filaments (Cosine Proximity)
      for (let i = 0; i < projected.length; i++) {
        for (let j = i + 1; j < projected.length; j++) {
          const pA = projected[i];
          const pB = projected[j];
          const dx = pA.px - pB.px;
          const dy = pA.py - pB.py;
          const dist = Math.sqrt(dx * dx + dy * dy);

          if (dist < 65 && Math.abs(pA.pz - pB.pz) < 70) {
            const alpha = (1 - dist / 65) * 0.25 * Math.min(pA.scale, pB.scale);
            ctx.beginPath();
            ctx.moveTo(pA.px, pA.py);
            ctx.lineTo(pB.px, pB.py);

            if (pA.node.tier === "exact" && pB.node.tier === "exact") {
              ctx.strokeStyle = `rgba(34, 211, 238, ${alpha})`;
            } else if (pA.node.tier === "semantic" || pB.node.tier === "semantic") {
              ctx.strokeStyle = `rgba(167, 139, 250, ${alpha})`;
            } else {
              ctx.strokeStyle = `rgba(52, 211, 153, ${alpha})`;
            }
            ctx.lineWidth = 1;
            ctx.stroke();
          }
        }
      }

      // Draw Nodes
      projected.forEach(({ px, py, scale, node }) => {
        const radius = node.baseRadius * scale;
        let color = "#22d3ee"; // Exact
        if (node.tier === "semantic") color = "#a78bfa";
        else if (node.tier === "execution") color = "#34d399";
        else if (node.tier === "origin") color = "#f59e0b";

        // Outer Glow
        const gradient = ctx.createRadialGradient(px, py, 0, px, py, radius * 3);
        gradient.addColorStop(0, color);
        gradient.addColorStop(1, "rgba(0, 0, 0, 0)");

        ctx.fillStyle = gradient;
        ctx.beginPath();
        ctx.arc(px, py, radius * 3, 0, Math.PI * 2);
        ctx.fill();

        // Core Dot
        ctx.fillStyle = "#ffffff";
        ctx.beginPath();
        ctx.arc(px, py, radius * 0.7, 0, Math.PI * 2);
        ctx.fill();

        // Label for landmark nodes
        if (node.label && scale > 0.85) {
          ctx.fillStyle = "rgba(226, 232, 240, 0.75)";
          ctx.font = "9px ui-monospace, SFMono-Regular, Menlo, monospace";
          ctx.fillText(node.label, px + 8, py + 3);
        }
      });

      // Draw & Update Pulse Waves
      for (let i = waves.length - 1; i >= 0; i--) {
        const w = waves[i];
        w.radius += 3.5;
        w.opacity *= 0.96;

        ctx.beginPath();
        ctx.arc(w.x, w.y, w.radius, 0, Math.PI * 2);
        ctx.strokeStyle = w.color;
        ctx.globalAlpha = Math.max(0, w.opacity);
        ctx.lineWidth = 1.8;
        ctx.stroke();
        ctx.globalAlpha = 1.0;

        if (w.opacity < 0.02 || w.radius > w.maxRadius) {
          waves.splice(i, 1);
        }
      }

      animationFrameId = requestAnimationFrame(render);
    };

    animationFrameId = requestAnimationFrame(render);

    return () => {
      window.removeEventListener("resize", handleResize);
      clearInterval(pulseInterval);
      cancelAnimationFrame(animationFrameId);
      canvas.removeEventListener("mousemove", onPointerMove);
      canvas.removeEventListener("mouseleave", onPointerLeave);
      canvas.removeEventListener("click", onClick);
    };
  }, []);

  return (
    <div className="relative w-full h-[480px] rounded-2xl overflow-hidden border border-white/[0.08] bg-gradient-to-b from-slate-950/90 via-slate-900/60 to-slate-950/90 backdrop-blur-xl shadow-glass group">
      {/* Canvas */}
      <canvas ref={canvasRef} className="w-full h-full cursor-crosshair" />

      {/* Top HUD Overlay */}
      <div className="absolute top-4 left-4 right-4 flex items-center justify-between pointer-events-none">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-slate-900/80 border border-white/10 text-xs font-mono text-slate-300 backdrop-blur-md">
            <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
            <span>TOPOLOGY: CONFORMAL 768-D MANIFOLD</span>
          </div>
          <div className="hidden sm:flex items-center gap-2 px-3 py-1.5 rounded-lg bg-slate-900/80 border border-white/10 text-xs font-mono text-cyan-400 backdrop-blur-md">
            <span>P99 HOT PATH:</span>
            <span className="text-white font-bold">{p99LatencyUs} µs</span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <div className="px-3 py-1.5 rounded-lg bg-slate-900/80 border border-white/10 text-xs font-mono text-slate-400 backdrop-blur-md">
            <span>FPS: {hudState.fps}</span>
          </div>
          <div className="hidden sm:flex px-3 py-1.5 rounded-lg bg-cyan-950/40 border border-cyan-500/30 text-xs font-mono text-cyan-300 backdrop-blur-md">
            <span>HIT RATIO: {hitRate}%</span>
          </div>
        </div>
      </div>

      {/* Bottom Interactive Legend */}
      <div className="absolute bottom-4 left-4 right-4 flex flex-wrap items-center justify-between gap-3 pointer-events-none">
        <div className="flex items-center gap-4 text-xs font-mono text-slate-400 bg-slate-950/80 px-4 py-2 rounded-xl border border-white/10 backdrop-blur-md">
          <div className="flex items-center gap-1.5">
            <span className="w-2.5 h-2.5 rounded-full bg-cyan-400 shadow-glow" />
            <span>Tier 0/1 Exact</span>
          </div>
          <div className="flex items-center gap-1.5">
            <span className="w-2.5 h-2.5 rounded-full bg-violet-400 shadow-glow-violet" />
            <span>Tier 2 Semantic</span>
          </div>
          <div className="flex items-center gap-1.5">
            <span className="w-2.5 h-2.5 rounded-full bg-emerald-400 shadow-glow-emerald" />
            <span>Tier 3 DAG Reuse</span>
          </div>
          <div className="flex items-center gap-1.5">
            <span className="w-2.5 h-2.5 rounded-full bg-amber-400" />
            <span>Tier 4 Origin</span>
          </div>
        </div>

        <div className="text-[11px] font-mono text-slate-500 bg-slate-950/80 px-3 py-2 rounded-xl border border-white/10">
          <span>Click canvas to emit cache query pulse wave</span>
        </div>
      </div>
    </div>
  );
}
