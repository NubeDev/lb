// The datasources lib barrel (rules-workbench scope, Phase 3) — the named `datasource.*` verb clients
// + their wire types. Feature code imports from here, never the raw `invoke` seam.

export { listDatasources, addDatasource, removeDatasource, testDatasource } from "./datasource.api";
export type { DatasourceSummary, AddDatasource, ProbeResult } from "./datasource.types";
