import type { CSSProperties } from "react";

// Shared control styles for the canvas' floating pickers/menus (Add-component
// filter, ActionPicker, ParamField). Named `ac*` after the ActionPicker where
// they originated.
export const acInput: CSSProperties = {
  background: "hsl(var(--background))",
  border: "1px solid hsl(var(--border))",
  borderRadius: 4,
  color: "hsl(var(--foreground))",
  fontSize: 12,
  padding: "4px 6px",
  fontFamily: "inherit",
};
export const acBtn: CSSProperties = {
  width: "100%",
  textAlign: "left",
  background: "transparent",
  color: "hsl(var(--foreground))",
  border: "none",
  borderRadius: 4,
  padding: "6px 8px",
  cursor: "pointer",
  fontFamily: "inherit",
  fontSize: 12,
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: 6,
};
export const acBtnPrimary: CSSProperties = {
  width: "100%",
  background: "hsl(var(--cool))",
  color: "#fff",
  border: "none",
  borderRadius: 4,
  padding: "7px 8px",
  cursor: "pointer",
  fontFamily: "inherit",
  fontSize: 12,
  marginTop: 6,
};
export const acRow: CSSProperties = { display: "flex", justifyContent: "space-between", padding: "2px 0" };
