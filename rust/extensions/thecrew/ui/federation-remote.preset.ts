// The federation-remote vite preset (thecrew finding 5). Two live-only bugs bit thecrew because a Vite
// LIB build behaves unlike an app build, and every bundling extension would otherwise rediscover them:
//
//   1. `process is not defined` — three.js / @react-three/fiber (and many libs) read
//      `process.env.NODE_ENV` at module eval. A Vite APP build injects it; a LIB build does NOT. Left
//      unreplaced, the remote throws the instant the shell imports it and the page never mounts.
//   2. A second React copy → "Invalid hook call". `react`/`react-dom`/`react-dom/client`/
//      `react/jsx-runtime` MUST stay external so their bare imports survive and the shell's import map
//      resolves them to the host's SINGLE React.
//
// This preset carries BOTH defaults so a new federated remote gets them for free:
//
//   import { federationRemote } from "./federation-remote.preset";
//   export default defineConfig(federationRemote({ plugins: [react(), tailwindcss()] }));
//
// It's a plain object builder (no Vite import) so it's trivially copyable into any extension's UI (they
// build standalone with `--ignore-workspace`, so a workspace package wouldn't resolve — a copyable
// reference file is the honest "shared" here). Callers merge extra `build`/`define` on top as needed.

/** The React entry points every remote externalises (resolved by the shell import map to ONE React). */
export const REACT_EXTERNALS = [
  "react",
  "react-dom",
  "react-dom/client",
  "react/jsx-runtime",
] as const;

interface RemoteOptions {
  /** Vite plugins (react(), tailwindcss(), …) — the caller supplies these. */
  plugins?: unknown[];
  /** The remote entry module (default `src/remoteEntry.ts`). */
  entry?: string;
  /** Extra `external` ids to add to the React set (e.g. a peer the shell also provides). */
  external?: string[];
  /** Extra `define` replacements merged over the NODE_ENV default. */
  define?: Record<string, string>;
}

/** Build a Vite config object for a federation remote: NODE_ENV defined, React externalised, one ESM
 *  `remoteEntry.js`, CSS folded into a single injectable string. Pass the result to `defineConfig`. */
export function federationRemote(opts: RemoteOptions = {}) {
  const { plugins = [], entry = "src/remoteEntry.ts", external = [], define = {} } = opts;
  return {
    plugins,
    // A LIB build does not inject NODE_ENV (bug 1) — define it so bundled libs that read it don't throw.
    define: { "process.env.NODE_ENV": JSON.stringify("production"), ...define },
    build: {
      target: "esnext",
      cssCodeSplit: false,
      lib: {
        entry,
        formats: ["es"] as const,
        fileName: () => "remoteEntry.js",
      },
      rollupOptions: {
        // Keep React external (bug 2) so the shell import map supplies the single copy.
        external: [...REACT_EXTERNALS, ...external],
      },
    },
  };
}
