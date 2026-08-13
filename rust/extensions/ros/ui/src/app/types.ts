// Response shapes exactly mirroring the ros backend handlers (rust/extensions/ros/src/handlers/*.rs)
// — never invented, never widened. The connection token is deliberately absent from `Connection`:
// `ros.get`/`ros.list` never return it (handlers/ros.rs `public_view`).

export interface Connection {
  uuid: string;
  name: string;
  base_url: string;
  enable: boolean;
  poll_rate?: number;
}

export interface Network {
  uuid: string;
  name: string;
  enable: boolean;
}

export interface Device {
  uuid: string;
  name: string;
  enable: boolean;
  network_uuid: string;
}

export interface Point {
  uuid: string;
  name: string;
  enable: boolean;
  present_value: number | null;
  device_uuid: string;
}

export interface Schedule {
  uuid: string;
  name: string;
  enable: boolean;
  is_active: boolean;
  schedule?: unknown;
}

export interface Page<T> {
  items: T[];
  next_cursor: string | null;
}

export interface NotFound {
  error: "not_found";
  [key: string]: unknown;
}

export function isNotFound(v: unknown): v is NotFound {
  return typeof v === "object" && v !== null && (v as { error?: unknown }).error === "not_found";
}
