// The field-name picker (editor-parity scope, step 1): every place the editor needs a field name
// OFFERS the draft's real result fields (from `useResultFields` — the live preview's viz.query rows),
// never makes the user remember and retype one. Degrades honestly: with no frames yet it's a labeled
// free-text combobox (`allowCustom` always on, so an author is never blocked). One responsibility:
// pick a field name.

import { Combobox } from "@/components/ui/combobox";
import { useResultFields } from "./FieldsContext";

interface Props {
  value: string;
  onChange: (field: string) => void;
  "aria-label": string;
  placeholder?: string;
  className?: string;
}

export function FieldNamePicker({ value, onChange, "aria-label": ariaLabel, placeholder, className }: Props) {
  const fields = useResultFields();
  return (
    <Combobox
      aria-label={ariaLabel}
      className={className}
      allowCustom
      options={fields.map((f) => ({ value: f }))}
      value={value}
      onChange={onChange}
      placeholder={placeholder ?? (fields.length ? "— pick a field —" : "type a field name (no preview data yet)")}
    />
  );
}
