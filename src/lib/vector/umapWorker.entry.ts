import { UMAP } from "umap-js";

interface RunMsg {
  kind: "run";
  vecs: Float32Array;
  dim: number;
  count: number;
  nNeighbors: number;
  minDist: number;
}

self.addEventListener("message", (ev: MessageEvent<RunMsg>) => {
  const msg = ev.data;
  if (msg.kind !== "run") return;
  try {
    const data: number[][] = new Array(msg.count);
    for (let i = 0; i < msg.count; i++) {
      data[i] = Array.from(
        msg.vecs.subarray(i * msg.dim, (i + 1) * msg.dim),
      );
    }
    const umap = new UMAP({
      nComponents: 2,
      nNeighbors: msg.nNeighbors,
      minDist: msg.minDist,
    });
    const nEpochs = umap.initializeFit(data);
    for (let e = 0; e < nEpochs; e++) {
      umap.step();
      if (e % Math.max(1, Math.floor(nEpochs / 20)) === 0) {
        self.postMessage({ kind: "progress", value: e / nEpochs });
      }
    }
    const coords = umap.getEmbedding();
    const out = new Float32Array(coords.length * 2);
    for (let i = 0; i < coords.length; i++) {
      out[i * 2] = coords[i][0];
      out[i * 2 + 1] = coords[i][1];
    }
    self.postMessage({ kind: "done", coords: out }, { transfer: [out.buffer] });
  } catch (e) {
    self.postMessage({
      kind: "error",
      message: e instanceof Error ? e.message : String(e),
    });
  }
});

export {};
