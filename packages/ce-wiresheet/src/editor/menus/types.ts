// Palette shape shared by the left palette and the pane "Add component" menu.
export interface PaletteComponent {
  name: string;
  type: string; // full "vendor-ext::name"
  icon?: string;
}

export interface PaletteExtension {
  id: string; // "vendor-ext"
  vendor: string;
  name: string;
  version?: string;
  components: PaletteComponent[];
}
