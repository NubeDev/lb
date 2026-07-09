// The datasources lib barrel (rules-workbench scope, Phase 3) — the named `datasource.*` verb clients
// + their wire types. Feature code imports from here, never the raw `invoke` seam.

export {
  listDatasources,
  addDatasource,
  removeDatasource,
  testDatasource,
  runFederationQuery,
  discoverTables,
  describeTable,
} from "./datasource.api";
export type {
  DatasourceSummary,
  AddDatasource,
  ProbeResult,
  FederationQueryResult,
  DbTable,
  DbColumn,
} from "./datasource.types";

// schema-designer scope — the `dbschema.*` CRUD + `federation.write`/`migrate`/`export` clients.
export {
  saveDbSchema,
  getDbSchema,
  listDbSchemas,
  deleteDbSchema,
  migrateSchema,
  federationWrite,
  federationExport,
  encodeRecord,
} from "./dbschema.api";
export type {
  DbSchemaRecord,
  DbSchemaSummary,
  DesignTable,
  DesignColumn,
  DesignFk,
  LayoutPos,
  MigrateStatement,
  MigrateResult,
  WriteResult,
  ExportResult,
} from "./dbschema.types";
export { NEUTRAL_TYPES } from "./dbschema.types";
