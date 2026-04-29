export interface UmapRunArgs {
  vecs: Float32Array;
  dim: number;
  count: number;
  nNeighbors: number;
  minDist: number;
  onProgress?: (v: number) => void;
}

export function runUmap(args: UmapRunArgs): Promise<Float32Array> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(
      new URL("./umapWorker.entry.ts", import.meta.url),
      { type: "module" },
    );
    worker.addEventListener("message", (ev) => {
      const m = ev.data as
        | { kind: "progress"; value: number }
        | { kind: "done"; coords: Float32Array }
        | { kind: "error"; message: string };
      if (m.kind === "progress") args.onProgress?.(m.value);
      else if (m.kind === "done") {
        resolve(m.coords);
        worker.terminate();
      } else if (m.kind === "error") {
        reject(new Error(m.message));
        worker.terminate();
      }
    });
    worker.postMessage(
      {
        kind: "run",
        vecs: args.vecs,
        dim: args.dim,
        count: args.count,
        nNeighbors: args.nNeighbors,
        minDist: args.minDist,
      },
      [args.vecs.buffer],
    );
  });
}
