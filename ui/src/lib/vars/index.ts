// The shared vars library barrel (widget-config-vars scope). Pure TS — extensions link THIS module (a
// federation-shared singleton, like React). The FROZEN contract: `interpolate`/`interpolateArgs` +
// `VarScope` + `resolveBuiltins`, versioned by `VARS_LIB_V`.

export * from "./types";
export { interpolate, formatValue } from "./interpolate";
export { interpolateArgs } from "./interpolateValue";
export { resolveBuiltins, type BuiltinInputs } from "./builtins";
export { extractVarNames, extractVarNamesDeep, isBuiltinName } from "./parse";
