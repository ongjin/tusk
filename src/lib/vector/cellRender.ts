export function l2Norm(v: number[]): number {
  let s = 0;
  for (const x of v) s += x * x;
  return Math.sqrt(s);
}

export function formatVectorSummary(v: number[]): string {
  return `[${v.length}d, ‖v‖=${l2Norm(v).toFixed(3)}]`;
}

/**
 * Draw a tiny sparkline of the first 32 dimensions (or fewer if shorter)
 * onto an existing canvas. Caller sizes the canvas; we only paint.
 */
export function renderSparkline(canvas: HTMLCanvasElement, v: number[]): void {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);
  if (v.length === 0) return;
  const slice = v.slice(0, 32);
  let min = slice[0];
  let max = slice[0];
  for (const x of slice) {
    if (x < min) min = x;
    if (x > max) max = x;
  }
  const span = max - min || 1;
  ctx.beginPath();
  ctx.lineWidth = 1;
  ctx.strokeStyle = "currentColor";
  for (let i = 0; i < slice.length; i++) {
    const x = (i / Math.max(1, slice.length - 1)) * (w - 1);
    const norm = (slice[i] - min) / span;
    const y = (1 - norm) * (h - 1);
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }
  ctx.stroke();
}
