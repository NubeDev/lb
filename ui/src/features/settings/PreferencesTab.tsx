// The Preferences tab (user-prefs scope, the shipped-crate's long-deferred "settings surface"). Edits
// ALL EIGHT preference axes — language, timezone, date/time style, first day of week, number format,
// unit system, and the closed dimension→unit overrides — and writes them through the real host verbs:
//   - "My preferences" → `prefs.set` (member; forced to the caller's own `sub`);
//   - "Workspace defaults" → `prefs.set_default` (admin-only; gated by `mcp:prefs.set_default:call`).
// The editor shows only what's EXPLICITLY set (via `prefs.get`) with the resolved chain as the ghost
// fallback (via `prefs.resolve`), so an unset axis reads "Inherit", never a silent concrete value. The
// unit-override picker is generated from the SAME closed vocabulary the server enforces
// (`dimensions.generated.ts`), so client and server can't disagree on the allowed set.

import { useCallback, useEffect, useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { CAP, hasCap } from "@/lib/session";
import { DIMENSIONS, UNITS, UNIT_DIMENSION } from "@/lib/prefs/dimensions.generated";
import { getPrefs } from "@/lib/prefs/get";
import { resolvePrefs } from "@/lib/prefs/resolve";
import { setDefaultPrefs, setPrefs, type PrefsPatch } from "@/lib/prefs/set";
import type { ResolvedPrefs } from "@/lib/prefs/prefs.types";
import { Field, FieldGroup } from "./Field";

/** The closed option sets for the enum axes (mirror `lb_prefs`'s serde `snake_case` variants). */
const LANGUAGES = [
  { v: "en", label: "English" },
  { v: "es", label: "Español" },
];
const DATE_STYLES = [
  { v: "eu", label: "EU — 31/12/2026" },
  { v: "iso", label: "ISO — 2026-12-31" },
  { v: "usa", label: "USA — 12/31/2026" },
];
const TIME_STYLES = [
  { v: "h24", label: "24-hour" },
  { v: "h12", label: "12-hour (AM/PM)" },
];
const FIRST_DAYS = [
  { v: "monday", label: "Monday" },
  { v: "sunday", label: "Sunday" },
];
const NUMBER_FORMATS = [
  { v: "dot_comma", label: "1,234.56 (dot decimal)" },
  { v: "comma_dot", label: "1.234,56 (comma decimal)" },
  { v: "space_comma", label: "1 234,56 (space group)" },
];
const UNIT_SYSTEMS = [
  { v: "metric", label: "Metric" },
  { v: "imperial", label: "Imperial" },
];
// A short, common timezone list for the picker; free typing is allowed (the host validates the IANA id).
const COMMON_TZS = [
  "UTC",
  "America/New_York",
  "America/Los_Angeles",
  "America/Chicago",
  "Europe/London",
  "Europe/Madrid",
  "Europe/Berlin",
  "Asia/Kolkata",
  "Asia/Singapore",
  "Australia/Sydney",
];

type Scope = "user" | "workspace";

interface Props {
  ws: string;
  caps: string[] | undefined;
}

export function PreferencesTab({ caps }: Props) {
  const canSetDefault = hasCap(caps, CAP.prefsSetDefault);
  const canSet = hasCap(caps, CAP.prefsSet);

  const [scope, setScope] = useState<Scope>("user");
  const [patch, setPatch] = useState<PrefsPatch>({});
  const [resolved, setResolved] = useState<ResolvedPrefs | null>(null);
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [error, setError] = useState<string | null>(null);

  // Load the viewer's explicitly-set axes (the editor's values) + the resolved chain (the ghost
  // fallback shown when an axis is unset). Both are real reads against the live gateway.
  const load = useCallback(() => {
    void getPrefs()
      .then((p) => setPatch(p ?? {}))
      .catch(() => setPatch({}));
    void resolvePrefs()
      .then(setResolved)
      .catch(() => setResolved(null));
  }, []);
  useEffect(load, [load]);

  const update = <K extends keyof PrefsPatch>(key: K, value: PrefsPatch[K] | undefined) => {
    setStatus("idle");
    setPatch((prev) => {
      const next = { ...prev };
      if (value === undefined || value === "") delete next[key];
      else next[key] = value;
      return next;
    });
  };

  const save = async () => {
    setStatus("saving");
    setError(null);
    try {
      if (scope === "workspace") await setDefaultPrefs(patch);
      else await setPrefs(patch);
      setStatus("saved");
      // Re-resolve so the ghost fallbacks reflect a just-changed workspace default.
      void resolvePrefs().then(setResolved).catch(() => {});
    } catch (e) {
      setStatus("error");
      setError(e instanceof Error ? e.message : "save failed");
    }
  };

  // The "inherited" placeholder for an unset axis — the resolved value, so the user sees what they'd
  // get without setting it (e.g. "Inherit — en").
  const ghost = (axis: keyof ResolvedPrefs): string =>
    resolved ? `Inherit — ${String(resolved[axis])}` : "Inherit";

  return (
    <div className="mx-auto max-w-3xl px-4 py-4">
      <ScopeSwitch scope={scope} setScope={setScope} canSetDefault={canSetDefault} onChange={load} />

      <FieldGroup title="Language & region">
        <Field label="Language" htmlFor="pref-language" help="The locale for app text and server-generated content.">
          <EnumSelect id="pref-language" value={patch.language} options={LANGUAGES} placeholder={ghost("language")} onChange={(v) => update("language", v)} />
        </Field>
        <Field label="Timezone" htmlFor="pref-timezone" help="An IANA timezone id; wall-clock times render in it.">
          <>
            <Input
              id="pref-timezone"
              list="tz-list"
              value={patch.timezone ?? ""}
              placeholder={ghost("timezone")}
              onChange={(e) => update("timezone", e.target.value || undefined)}
            />
            <datalist id="tz-list">
              {COMMON_TZS.map((tz) => (
                <option key={tz} value={tz} />
              ))}
            </datalist>
          </>
        </Field>
        <Field label="First day of week" htmlFor="pref-first-day">
          <EnumSelect id="pref-first-day" value={patch.first_day_of_week} options={FIRST_DAYS} placeholder={ghost("first_day_of_week")} onChange={(v) => update("first_day_of_week", v)} />
        </Field>
      </FieldGroup>

      <FieldGroup title="Date, time & numbers">
        <Field label="Date style" htmlFor="pref-date">
          <EnumSelect id="pref-date" value={patch.date_style} options={DATE_STYLES} placeholder={ghost("date_style")} onChange={(v) => update("date_style", v as PrefsPatch["date_style"])} />
        </Field>
        <Field label="Time style" htmlFor="pref-time">
          <EnumSelect id="pref-time" value={patch.time_style} options={TIME_STYLES} placeholder={ghost("time_style")} onChange={(v) => update("time_style", v as PrefsPatch["time_style"])} />
        </Field>
        <Field label="Number format" htmlFor="pref-number">
          <EnumSelect id="pref-number" value={patch.number_format} options={NUMBER_FORMATS} placeholder={ghost("number_format")} onChange={(v) => update("number_format", v)} />
        </Field>
      </FieldGroup>

      <FieldGroup title="Units">
        <Field label="Unit system" htmlFor="pref-units" help="The base system; override individual dimensions below.">
          <EnumSelect id="pref-units" value={patch.unit_system} options={UNIT_SYSTEMS} placeholder={ghost("unit_system")} onChange={(v) => update("unit_system", v)} />
        </Field>
        <UnitOverrides
          overrides={patch.unit_overrides ?? {}}
          onChange={(next) =>
            update("unit_overrides", Object.keys(next).length ? next : undefined)
          }
        />
      </FieldGroup>

      <div className="sticky bottom-0 flex items-center gap-3 border-t border-border bg-bg/95 py-3 backdrop-blur">
        <Button onClick={save} disabled={status === "saving" || (scope === "user" ? !canSet : !canSetDefault)} aria-label="save preferences">
          {status === "saving" ? "Saving…" : scope === "workspace" ? "Save workspace defaults" : "Save my preferences"}
        </Button>
        {status === "saved" && <span className="text-xs text-accent">Saved.</span>}
        {status === "error" && (
          <span role="alert" className="text-xs text-red-500">
            {error}
          </span>
        )}
        <span className="ml-auto text-[11px] text-muted">Unset axes inherit the workspace default, then the built-in fallback.</span>
      </div>
    </div>
  );
}

/** The Apply-to switch: an admin may edit the WORKSPACE default; everyone edits their own. */
function ScopeSwitch({
  scope,
  setScope,
  canSetDefault,
  onChange,
}: {
  scope: Scope;
  setScope: (s: Scope) => void;
  canSetDefault: boolean;
  onChange: () => void;
}) {
  if (!canSetDefault) return null;
  return (
    <div className="mb-4 inline-flex rounded-md border border-border p-0.5 text-xs">
      {(["user", "workspace"] as Scope[]).map((s) => (
        <button
          key={s}
          type="button"
          aria-pressed={scope === s}
          onClick={() => {
            setScope(s);
            onChange();
          }}
          className={
            "rounded-md px-3 py-1 transition-colors " +
            (scope === s ? "bg-accent text-bg" : "text-muted hover:text-fg")
          }
        >
          {s === "user" ? "My preferences" : "Workspace defaults"}
        </button>
      ))}
    </div>
  );
}

/** A token-bound `<select>` with an "inherit" empty option (its label is the resolved ghost value). */
function EnumSelect({
  id,
  value,
  options,
  placeholder,
  onChange,
}: {
  id?: string;
  value: string | undefined;
  options: { v: string; label: string }[];
  placeholder: string;
  onChange: (v: string | undefined) => void;
}) {
  return (
    <Select id={id} value={value ?? ""} onChange={(e) => onChange(e.target.value || undefined)}>
      <option value="">{placeholder}</option>
      {options.map((o) => (
        <option key={o.v} value={o.v}>
          {o.label}
        </option>
      ))}
    </Select>
  );
}

/** The closed dimension→unit override editor — one row per dimension, unit options filtered to that
 *  dimension via the GENERATED `UNIT_DIMENSION` map (client + server share the vocabulary). */
function UnitOverrides({
  overrides,
  onChange,
}: {
  overrides: Record<string, string>;
  onChange: (next: Record<string, string>) => void;
}) {
  const unitsByDim = useMemo(() => {
    const m: Record<string, string[]> = {};
    for (const u of UNITS) {
      const d = UNIT_DIMENSION[u];
      (m[d] ??= []).push(u);
    }
    return m;
  }, []);

  const set = (dim: string, unit: string | undefined) => {
    const next = { ...overrides };
    if (unit) next[dim] = unit;
    else delete next[dim];
    onChange(next);
  };

  return (
    <>
      {DIMENSIONS.map((dim) => (
        <Field key={dim} label={`Override — ${dim}`} htmlFor={`unit-${dim}`}>
          <Select
            id={`unit-${dim}`}
            value={overrides[dim] ?? ""}
            onChange={(e) => set(dim, e.target.value || undefined)}
          >
            <option value="">Use system default</option>
            {(unitsByDim[dim] ?? []).map((u) => (
              <option key={u} value={u}>
                {u}
              </option>
            ))}
          </Select>
        </Field>
      ))}
    </>
  );
}
