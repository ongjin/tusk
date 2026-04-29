export interface VectorColumn {
  schema: string;
  table: string;
  column: string;
  dim: number;
  hasIndex: boolean;
}

export interface VectorIndexParams {
  m?: number;
  efConstruction?: number;
  lists?: number;
  ops?: string;
}

export interface VectorIndex {
  name: string;
  schema: string;
  table: string;
  column: string;
  method: "hnsw" | "ivfflat";
  params: VectorIndexParams;
  sizeBytes: number;
  definition: string;
}

export interface SampledVectorRow {
  pkJson: Record<string, unknown>;
  vec: number[];
}

export interface SampledVectors {
  rows: SampledVectorRow[];
  totalRows: number;
}

export type AnnOperator = "<=>" | "<->" | "<#>";

export const ANN_OPERATOR_LABELS: Record<AnnOperator, string> = {
  "<=>": "cosine distance",
  "<->": "L2 distance",
  "<#>": "negative inner product",
};
