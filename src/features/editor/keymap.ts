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
