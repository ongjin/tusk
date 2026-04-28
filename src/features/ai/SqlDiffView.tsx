import { DiffEditor } from "@monaco-editor/react";

import { useTheme } from "@/hooks/use-theme";

interface Props {
  original: string;
  modified: string;
  height?: number;
}

export function SqlDiffView({ original, modified, height = 240 }: Props) {
  const { theme } = useTheme();
  return (
    <DiffEditor
      original={original}
      modified={modified}
      language="sql"
      theme={theme === "dark" ? "vs-dark" : "vs"}
      height={height}
      options={{
        renderSideBySide: true,
        readOnly: true,
        minimap: { enabled: false },
        scrollBeyondLastLine: false,
        fontSize: 12,
      }}
    />
  );
}
