export interface TopKTable {
  schema: string;
  table: string;
  ddl: string;
  similarity: number;
  forced: boolean;
}

export interface SchemaTopK {
  tables: TopKTable[];
  totalTables: number;
}
