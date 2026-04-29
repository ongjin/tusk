import { useEffect, useRef } from "react";

interface Point {
  x: number;
  y: number;
  pkJson: Record<string, unknown>;
}

interface Props {
  points: Point[];
  selectedIdx?: number;
  onSelect: (idx: number) => void;
}

export function UmapScatter({ points, selectedIdx, onSelect }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef({ tx: 0, ty: 0, scale: 1 });

  useEffect(() => {
    const c = canvasRef.current;
    const wrap = containerRef.current;
    if (!c || !wrap) return;

    const draw = () => {
      const ctx = c.getContext("2d");
      if (!ctx) return;
      const w = c.width;
      const h = c.height;
      ctx.clearRect(0, 0, w, h);
      if (points.length === 0) return;

      let minX = points[0].x, maxX = points[0].x;
      let minY = points[0].y, maxY = points[0].y;
      for (const p of points) {
        if (p.x < minX) minX = p.x;
        if (p.x > maxX) maxX = p.x;
        if (p.y < minY) minY = p.y;
        if (p.y > maxY) maxY = p.y;
      }
      const spanX = maxX - minX || 1;
      const spanY = maxY - minY || 1;
      const pad = 12;
      const v = viewRef.current;
      const project = (px: number, py: number): [number, number] => {
        const nx = ((px - minX) / spanX) * (w - 2 * pad) + pad;
        const ny = (1 - (py - minY) / spanY) * (h - 2 * pad) + pad;
        return [nx * v.scale + v.tx, ny * v.scale + v.ty];
      };

      ctx.fillStyle = "rgba(59,130,246,0.6)";
      for (let i = 0; i < points.length; i++) {
        const [x, y] = project(points[i].x, points[i].y);
        ctx.beginPath();
        ctx.arc(x, y, 2, 0, Math.PI * 2);
        ctx.fill();
      }
      if (selectedIdx !== undefined && points[selectedIdx]) {
        const p = points[selectedIdx];
        const [x, y] = project(p.x, p.y);
        ctx.strokeStyle = "rgb(239,68,68)";
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(x, y, 5, 0, Math.PI * 2);
        ctx.stroke();
      }

      // store projection for hit-testing
      (c as unknown as { _project: typeof project })._project = project;
    };

    const observer = new ResizeObserver(() => {
      const r = wrap.getBoundingClientRect();
      c.width = Math.floor(r.width);
      c.height = Math.floor(r.height);
      draw();
    });
    observer.observe(wrap);

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const v = viewRef.current;
      const factor = Math.exp(-e.deltaY * 0.001);
      const rect = c.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      v.tx = mx - (mx - v.tx) * factor;
      v.ty = my - (my - v.ty) * factor;
      v.scale *= factor;
      draw();
    };
    let dragging = false;
    let lastX = 0,
      lastY = 0;
    const onDown = (e: MouseEvent) => {
      dragging = true;
      lastX = e.clientX;
      lastY = e.clientY;
    };
    const onMove = (e: MouseEvent) => {
      if (!dragging) return;
      const v = viewRef.current;
      v.tx += e.clientX - lastX;
      v.ty += e.clientY - lastY;
      lastX = e.clientX;
      lastY = e.clientY;
      draw();
    };
    const onUp = () => {
      dragging = false;
    };
    const onClick = (e: MouseEvent) => {
      const project = (c as unknown as { _project?: (x: number, y: number) => [number, number] })
        ._project;
      if (!project) return;
      const rect = c.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      let bestIdx = -1;
      let bestDist = Infinity;
      for (let i = 0; i < points.length; i++) {
        const [px, py] = project(points[i].x, points[i].y);
        const d = Math.hypot(px - mx, py - my);
        if (d < bestDist) {
          bestDist = d;
          bestIdx = i;
        }
      }
      if (bestIdx >= 0 && bestDist < 8) onSelect(bestIdx);
    };

    c.addEventListener("wheel", onWheel, { passive: false });
    c.addEventListener("mousedown", onDown);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    c.addEventListener("click", onClick);

    return () => {
      observer.disconnect();
      c.removeEventListener("wheel", onWheel);
      c.removeEventListener("mousedown", onDown);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      c.removeEventListener("click", onClick);
    };
  }, [points, selectedIdx, onSelect]);

  return (
    <div ref={containerRef} className="bg-background relative h-full w-full">
      <canvas ref={canvasRef} className="block h-full w-full" />
    </div>
  );
}
