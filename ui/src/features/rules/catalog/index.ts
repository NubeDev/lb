// The function-palette catalog — the static, typed mirror of the `lb-rules` registered verbs, assembled
// in display order: Data → Grid → Timeseries → AI → Output (rules-editor-ux scope). The barrel only
// re-exports; the data lives in the per-family files.

import { DATA_GROUP } from "./data";
import { GRID_GROUP } from "./grid";
import { TIMESERIES_GROUP } from "./timeseries";
import { AI_GROUP } from "./ai";
import { OUTPUT_GROUP } from "./output";
import type { CatalogGroup } from "./catalog.types";

/** Every palette group, in display order. */
export const CATALOG: CatalogGroup[] = [
  DATA_GROUP,
  GRID_GROUP,
  TIMESERIES_GROUP,
  AI_GROUP,
  OUTPUT_GROUP,
];

export type { CatalogGroup, FnEntry, CatalogCategory } from "./catalog.types";
