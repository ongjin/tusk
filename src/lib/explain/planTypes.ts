export type ExplainMode =
  | "select-analyze"
  | "dml-plan-only"
  | "ddl-plan-only"
  | "passthrough"
  | "analyze-anyway-rolled-back"
  | "analyze-anyway-in-tx";

export interface RawExplainPlan {
  Plan: RawPlanNode;
  "Planning Time"?: number;
  "Execution Time"?: number;
}

export interface RawPlanNode {
  "Node Type": string;
  "Parallel Aware"?: boolean;
  "Join Type"?: string;
  "Relation Name"?: string;
  Schema?: string;
  Alias?: string;
  "Startup Cost": number;
  "Total Cost": number;
  "Plan Rows": number;
  "Plan Width": number;
  "Actual Startup Time"?: number;
  "Actual Total Time"?: number;
  "Actual Rows"?: number;
  "Actual Loops"?: number;
  "Rows Removed by Filter"?: number;
  Filter?: string;
  "Index Cond"?: string;
  "Hash Cond"?: string;
  "Merge Cond"?: string;
  "Recheck Cond"?: string;
  Output?: string[];
  "Shared Hit Blocks"?: number;
  "Shared Read Blocks"?: number;
  "Shared Written Blocks"?: number;
  Plans?: RawPlanNode[];
  Buffers?: {
    "Shared Hit Blocks"?: number;
    "Shared Read Blocks"?: number;
    "Shared Written Blocks"?: number;
  };
}

export interface PlanNode {
  nodeType: string;
  relationName?: string;
  schema?: string;
  alias?: string;
  startupCost: number;
  totalCost: number;
  planRows: number;
  planWidth: number;
  actualStartupTime: number | null;
  actualTotalTime: number | null;
  actualLoops: number | null;
  actualRows: number | null;
  rowsRemovedByFilter: number | null;
  filter?: string;
  indexCond?: string;
  joinType?: string;
  hashCond?: string;
  mergeCond?: string;
  output?: string[];
  buffers: { hit: number; read: number; written: number } | null;
  children: PlanNode[];
  selfMs: number | null;
  selfTimeRatio: number | null;
  selfCostRatio: number;
}

export interface IndexCandidate {
  schema: string;
  table: string;
  columns: string[];
  reason: "seq-scan-filter" | "rows-removed-by-filter" | "lossy-index-cond";
  verdict: "likely" | "maybe";
  selectivityEstimate: number | null;
  nDistinct: number | null;
  nullFrac: number | null;
}

export interface ExplainResult {
  mode: ExplainMode;
  planJson: RawExplainPlan;
  plan: PlanNode;
  warnings: string[];
  verifiedCandidates: IndexCandidate[];
  totalMs: number | null;
  executedAt: number;
}

export interface AiInterpretation {
  summary: string;
  recommendations: AiIndexRecommendation[];
  promptTokens?: number;
  completionTokens?: number;
  durationMs: number;
}

export interface AiIndexRecommendation {
  schema: string;
  table: string;
  columns: string[];
  type: "btree" | "composite" | "partial";
  where?: string;
  reason: string;
  priority: "high" | "medium" | "low";
}
