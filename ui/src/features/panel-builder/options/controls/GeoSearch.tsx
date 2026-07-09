// The Location search control (weather scope) — a keyless city/place autocomplete that auto-fills the
// weather tile's lat/lon. Built on `react-select`'s AsyncSelect (debounced remote options, keyboard nav,
// loading state) backed by the Open-Meteo geocoding client. Picking a place calls `onPick` with its
// label + coordinates; the option row writes all three (`options.label/lat/lon`) atomically. Rendered
// unstyled + themed with the app's token classes so it matches the surrounding shadcn controls in both
// light and dark. One responsibility: place query → picked `GeoPlace`.

import { useRef } from "react";
import AsyncSelect from "react-select/async";

import { searchPlaces, type GeoPlace } from "./geocode";

interface Props {
  /** The current label to show as the selected value (from `options.label`). */
  value?: string;
  /** Called with the picked place — the caller writes label + lat/lon. */
  onPick: (place: GeoPlace) => void;
  /** Accessible name (the option label). */
  label: string;
}

interface Option {
  value: string;
  label: string;
  place: GeoPlace;
}

export function GeoSearch({ value, onPick, label }: Props) {
  // Abort the previous in-flight request when a new keystroke supersedes it (react-select calls the
  // loader per input change) — keeps the dropdown showing the latest query's matches, not a stale race.
  const abortRef = useRef<AbortController | null>(null);

  const loadOptions = async (input: string): Promise<Option[]> => {
    abortRef.current?.abort();
    const ctrl = new AbortController();
    abortRef.current = ctrl;
    const places = await searchPlaces(input, ctrl.signal);
    return places.map((p) => ({ value: `${p.lat},${p.lon}`, label: p.label, place: p }));
  };

  return (
    <AsyncSelect<Option>
      aria-label={label}
      inputId={`geo-${label}`}
      cacheOptions
      defaultOptions={false}
      loadOptions={loadOptions}
      value={value ? { value, label: value, place: { label: value, lat: 0, lon: 0 } } : null}
      onChange={(opt) => opt && onPick(opt.place)}
      placeholder="Search a city…"
      noOptionsMessage={({ inputValue }) =>
        inputValue.trim().length < 2 ? "Type a city name…" : "No matches"
      }
      loadingMessage={() => "Searching…"}
      unstyled
      classNames={{
        control: () =>
          "h-8 min-h-8 rounded-md border border-border bg-bg px-2 text-xs text-fg focus-within:ring-2 focus-within:ring-accent/25",
        valueContainer: () => "gap-1",
        placeholder: () => "text-muted",
        input: () => "text-fg",
        singleValue: () => "text-fg",
        indicatorsContainer: () => "text-muted",
        dropdownIndicator: () => "px-1",
        menu: () => "mt-1 rounded-md border border-border bg-panel text-xs shadow-md z-50",
        menuList: () => "max-h-56 overflow-auto py-1",
        option: ({ isFocused }) =>
          `px-2.5 py-1.5 cursor-pointer ${isFocused ? "bg-accent/15 text-fg" : "text-fg"}`,
        noOptionsMessage: () => "px-2.5 py-2 text-muted",
        loadingMessage: () => "px-2.5 py-2 text-muted",
      }}
    />
  );
}
