// Tx shortcuts (handled in App.tsx, not Monaco):
//   Cmd+Shift+C / Ctrl+Shift+C  → tx commit (when active)
//   Cmd+Shift+R / Ctrl+Shift+R  → tx rollback (when active)
// These are global rather than Monaco-scoped because they apply outside
// the editor focus (e.g. when focus is on a result-grid cell).

export type Modifier = "meta" | "ctrl";

export function platformModifier(): Modifier {
  if (typeof navigator !== "undefined" && /Mac/i.test(navigator.platform))
    return "meta";
  return "ctrl";
}

export function isModifier(
  e: KeyboardEvent | React.KeyboardEvent,
  mod: Modifier,
) {
  return mod === "meta" ? e.metaKey : e.ctrlKey;
}
