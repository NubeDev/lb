// The incoming-preset shape — a shadcn/tweakcn theme as it ships in the packs: a `styles.{light,dark}`
// map of shadcn-vocabulary CSS custom-property names to color strings (`oklch`/`#hex`/`hsl`). This is
// the wire shape the ADAPTER consumes; it is NOT what the app applies (the adapter maps it back onto
// base tokens). One responsibility: the preset type.

/** One mode's shadcn-vocabulary style map. Only the keys the adapter reads are typed; a preset carries
 *  more (chart-*, sidebar-*, fonts) which we ignore — an index signature keeps them without error. */
export interface PresetStyles {
  background?: string;
  foreground?: string;
  card?: string;
  popover?: string;
  primary?: string;
  muted?: string;
  "muted-foreground"?: string;
  accent?: string;
  border?: string;
  input?: string;
  ring?: string;
  radius?: string;
  [k: string]: string | undefined;
}

/** A full preset as it appears in the shadcn/tweakcn packs. */
export interface ThemePreset {
  label: string;
  styles: {
    light: PresetStyles;
    dark: PresetStyles;
  };
}

/** A preset with its stable id, for the picker library. */
export interface PresetEntry {
  value: string;
  name: string;
  preset: ThemePreset;
}
