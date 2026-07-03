// The searchable, grouped unit picker (editor-parity scope, step 2) — replaces the flat unsearchable
// `<select>` in the Field tab. Fed by the single `units.ts` table (the source of truth), grouped by
// dimension so "km/h" sits under Speed and "°C" under Temperature, searchable by id/label. `allowCustom`
// lets an author type a `custom:<suffix>` id (the units table already degrades those honestly). One
// responsibility: pick a unit id.

import { Combobox, type ComboboxOption } from "@/components/ui/combobox";
import { unitOptions } from "../../../fieldconfig/units";

interface Props {
  value: string;
  onChange: (unitId: string) => void;
  /** Accessible name (defaults to "field unit"; the override property control passes "<id> value"). */
  "aria-label"?: string;
}

/** A friendly group label per unit kind/dimension for the picker headers. */
function groupOf(u: ReturnType<typeof unitOptions>[number]): string {
  if (u.dimension) {
    return u.dimension.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
  }
  if (u.kind === "datetime") return "Date & time";
  if (u.id.startsWith("currency") || u.suffix?.includes("USD") || u.suffix?.includes("EUR")) return "Currency";
  if (u.id === "percent" || u.id === "percentunit") return "Percent";
  return "Misc";
}

export function UnitPicker({ value, onChange, "aria-label": ariaLabel = "field unit" }: Props) {
  const options: ComboboxOption[] = unitOptions().map((u) => ({
    value: u.id,
    label: u.label ? `${u.id} (${u.label})` : u.id,
    group: groupOf(u),
  }));
  return (
    <Combobox
      aria-label={ariaLabel}
      options={options}
      value={value}
      onChange={onChange}
      allowCustom
      placeholder="— none —"
    />
  );
}
