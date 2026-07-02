// Type definitions matching the ce-rest OpenAPI spec, plus the WS protocol described
// in the spec's inline documentation.

// REST -----------------------------------------------------------------------

export type FlexValue = string | number | boolean | null;
// JS Number safely holds int64 only up to 2^53; the engine emits integers in JSON as
// raw numbers, so values beyond that range may lose precision. Acceptable for v1.

// Numeric enums (uint8) — the engine sends these as integers (API_GAPS #15),
// so the client compares integers, never strings. Constants below name the
// values; the types are the numeric unions.

// Property category.
export const CATEGORY_INPUT = 0;
export const CATEGORY_OUTPUT = 1;
export const CATEGORY_CONFIG = 2;
export type PropertyCategory = 0 | 1 | 2;

// WS schema dataType domain: only the value-plane-streamable types appear in the
// flat decode table. CONFIG-category and NULL-typed properties are excluded.
export const DATATYPE_NUMBER = 0;
export const DATATYPE_BOOL = 1;
export const DATATYPE_STRING = 2;
export type PropertyDataType = 0 | 1 | 2;

// Engine-assigned role. NORMAL = user-facing; STATUS / FACETS are engine-managed
// internal slots. The old `system` bool is gone — derive `system` as
// `systemRole !== ROLE_NORMAL`.
export const ROLE_NORMAL = 0;
export const ROLE_STATUS = 1;
export const ROLE_FACETS = 2;
export type PropertySystemRole = 0 | 1 | 2;

export interface Property {
  uid: number;
  componentUid: number;
  category: PropertyCategory; // numeric: 0 input / 1 output / 2 config
  value: FlexValue;
  // uint32 bit mask. Bit positions per spec — see STATUS_* constants below.
  // 0 = NONE.
  statusFlags: number;
  // Engine-assigned role (numeric). Default to ROLE_NORMAL (0) when absent.
  systemRole?: PropertySystemRole;
}

// Status bit positions (uint32). Mirror the engine's PropertyStatus enum.
//   bit 1 = FAULT
//   bit 5 = OVERRIDDEN
// Add new bits here as the engine defines them.
export const STATUS_FAULT = 1 << 1;
export const STATUS_OVERRIDDEN = 1 << 5;

export interface ComponentMetadata {
  position?: { x?: number; y?: number };
  size?: { h?: number; w?: number };
}

export interface Component {
  name: string;
  uid: number;
  type: string;
  path: string;
  parent: number;
  metadata?: ComponentMetadata;
  properties: Record<string, Property>;
  childrenCount?: number;
  children?: Component[];
}

export interface Edge {
  uid: number;
  sourceUid: number;
  sourcePath?: string;
  sourceProperty: string; // property NAME (e.g. "out") — prefer the uid below
  sourcePropertyUid?: number; // engine provides this — use it to avoid name compares
  targetUid: number;
  targetPath?: string;
  targetProperty: string;
  targetPropertyUid?: number;
  loopBack?: boolean;
  hidden?: boolean;
  // Present ONLY on the GET /edges?subtree= (classified folder view) response.
  // View-relative: `internal` = both ends inside the same child folder (the
  // folder-loopback case, dropped); `boundary` = different in-view containers;
  // `external` = at least one end outside the view subtree.
  class?: "internal" | "boundary" | "external";
  // The direct-child uid each endpoint resolves under (0 = the view root itself;
  // absent = outside the view). Only on the subtree response.
  sourceContainer?: number;
  targetContainer?: number;
}

export interface ReadNodesResponse {
  nodes: Component[];
  edges?: Edge[];
  // Only on /copy/nodes: old→new uid remap for the copied subtree, so the client
  // can rewrite uid references in copied values (e.g. __facets). See API_REQUESTS.
  uidMap?: {
    components?: Record<string, number>;
    properties?: Record<string, number>;
    edges?: Record<string, number>;
  };
}

export interface AddComponentRequest {
  type: string;
  name?: string;
  parentUid?: number;
  defaultValues?: {
    properties?: Record<string, { value: FlexValue }>;
    position?: { x?: number; y?: number };
  };
}

export interface UpdateComponentRequest {
  name?: string;
  parentUid?: number;
  properties?: Record<string, { value: FlexValue }>;
  position?: { x?: number; y?: number };
}

export interface EdgeRequest {
  // Owning component uids are OPTIONAL — omit (or send 0) and the engine derives
  // the owner from the property's `componentUid` back-reference (API_REQUESTS §1).
  sourceUid?: number;
  sourcePropUid: number;
  targetUid?: number;
  targetPropUid: number;
  loopback?: boolean;
  hidden?: boolean;
}

// WS -----------------------------------------------------------------------

// --- WS schema: a flat decode table, NOT a structural mirror ---
// Lists only the streamable properties (CONFIG and NULL-typed properties are
// excluded — they don't flow on the binary value plane). Structure comes from
// REST GET /nodes.
export interface SchemaPropertyEntry {
  uid: number;
  dataType: PropertyDataType;
  // Initial uint32 status bitmask. Seeds the client's render state at bootstrap;
  // delta updates arrive on the STATUS section of the binary frame thereafter.
  statusFlags: number;
}

export interface SchemaMessage {
  type: "schema";
  sessionId: string;
  resumed: boolean;
  currentSeq: number;
  properties: SchemaPropertyEntry[];
}

// --- Topology event payloads ---
// Topology events still carry full structural descriptors. The client doesn't
// use them to maintain a parallel structural cache — REST is authoritative —
// but it does inspect them to decide whether a `topologyChanged` needs a
// REST refetch (added/removed properties, parent change) or just a local patch
// (position, name).
export interface TopologyPropertyDescriptor {
  uid: number;
  name: string;
  componentUid: number;
  category: PropertyCategory;
  dataType?: PropertyDataType;
  systemRole?: PropertySystemRole;
}

// changeId/actorId are stamped per item: changeId is the engine undo-log id
// (same value the matching write response and the changelog return), actorId is
// the writer op's X-Actor-Id. Both omitted when the causing op was untracked
// (changeId/actorId 0). Let a client correlate a pushed item to its own write.
export interface TopologyComponentDescriptor {
  uid: number;
  kind: string;
  position?: { x: number; y: number };
  size?: { width: number; height: number };
  parent?: number;
  name?: string;
  inputs?: TopologyPropertyDescriptor[];
  outputs?: TopologyPropertyDescriptor[];
  config?: TopologyPropertyDescriptor[];
  changeId?: number;
  actorId?: number;
}

export interface TopologyEdgeDescriptor {
  uid: number;
  sourceProperty: number;
  targetProperty: number;
  changeId?: number;
  actorId?: number;
}

// --- Topology events (structural change push) ---

export interface TopologyAddedMsg {
  type: "topologyAdded";
  seq: number;
  originSessionId: string | null;
  components: TopologyComponentDescriptor[];
  edges: TopologyEdgeDescriptor[];
}

export interface TopologyRemovedMsg {
  type: "topologyRemoved";
  seq: number;
  originSessionId: string | null;
  componentUids: number[];
  edgeUids: number[];
  // Parallel to componentUids/edgeUids (bare-uid arrays kept for existing
  // parsers); carries per-item kind + change correlation. A removed component
  // lists every edge uid that touched it in edgeUids/removed.
  removed?: Array<{ uid: number; kind: "component" | "edge"; changeId?: number; actorId?: number }>;
}

export interface TopologyChangedComponent {
  uid: number;
  name?: string;
  parent?: number;
  position?: { x: number; y: number };
  size?: { width: number; height: number };
  addedProperties?: TopologyPropertyDescriptor[];
  removedProperties?: number[];
  changeId?: number;
  actorId?: number;
}

export interface TopologyChangedMsg {
  type: "topologyChanged";
  seq: number;
  originSessionId: string | null;
  components: TopologyChangedComponent[];
}

export type TopologyMsg = TopologyAddedMsg | TopologyRemovedMsg | TopologyChangedMsg;

// Binary frame typeTags (see spec §"Binary frame layout").
export const TYPE_BOOL = 0x01;
export const TYPE_U32 = 0x10;
export const TYPE_I32 = 0x11;
export const TYPE_F32 = 0x12;
export const TYPE_U64 = 0x20;
export const TYPE_I64 = 0x21;
export const TYPE_F64 = 0x22;
export const TYPE_STR = 0x30;
// STATUS section — runs parallel to the value sections, one uint32 of
// PropertyStatus bits per property keyed by the same uids. Used to push
// OVERRIDDEN / FAULT etc. without a REST round trip.
export const TYPE_STATUS = 0x40;

export const MSG_UPDATE = 0x01;
export const MSG_SNAPSHOT = 0x02;
