// Route search-param schemas for the shell URL grammar (routing scope). These validators are
// deliberately small and hand-written: malformed shared links degrade to defaults, never exceptions.

export const DEFAULT_CHANNEL = "general";

export interface ChannelSearch {
  c: string;
}

export interface DashboardSearch {
  from: string;
  to: string;
}

function scalar(value: unknown): string | null {
  if (typeof value === "string") return value;
  if (Array.isArray(value) && typeof value[0] === "string") return value[0];
  return null;
}

export function validateChannelSearch(search: Record<string, unknown>): ChannelSearch {
  const c = scalar(search.c)?.trim();
  return { c: c || DEFAULT_CHANNEL };
}

function isoDate(value: unknown): string | null {
  const raw = scalar(value);
  if (!raw || !/^\d{4}-\d{2}-\d{2}$/.test(raw)) return null;
  const d = new Date(`${raw}T00:00:00.000Z`);
  return Number.isNaN(d.getTime()) || d.toISOString().slice(0, 10) !== raw ? null : raw;
}

function addDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setUTCDate(next.getUTCDate() + days);
  return next;
}

export function defaultDashboardSearch(today = new Date()): DashboardSearch {
  const end = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate()));
  return {
    from: addDays(end, -30).toISOString().slice(0, 10),
    to: end.toISOString().slice(0, 10),
  };
}

export function validateDashboardSearch(search: Record<string, unknown>): DashboardSearch {
  const fallback = defaultDashboardSearch();
  const from = isoDate(search.from) ?? fallback.from;
  const to = isoDate(search.to) ?? fallback.to;
  return from <= to ? { from, to } : fallback;
}
