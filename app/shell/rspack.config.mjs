// Re.Pack 5 (Rspack) build for the shell — the MF2 HOST. The shell shares `react`, `react-native`,
// and `@nube/app-sdk` as eager singletons (app-shell scope): remotes resolve them from the host and
// can never bundle a second copy. No remotes are wired in THIS slice — extension containers load
// dynamically from the gateway in the app-extensions slice; declaring the host contract now keeps
// that slice additive.

import { createRequire } from 'node:module';
import * as Repack from '@callstack/repack';

const require = createRequire(import.meta.url);

export default (env) => {
  const { mode = 'development', context = Repack.getDirname(import.meta.url), platform } = env;

  return {
    mode,
    context,
    entry: './index.js',
    resolve: {
      ...Repack.getResolveOptions(),
    },
    module: {
      rules: [
        // RN 0.86 ships Flow `enum`s (e.g. react-native/src/.../VirtualView.js).
        // flow-remove-types (what Re.Pack's flow-loader uses) can't transform Flow
        // enums — it only strips types — so SWC then chokes on the `enum` keyword.
        // Run a Babel `pre` pass over RN core that parses via Hermes and lowers the
        // enums to flow-enums-runtime calls, before flow-loader/SWC see the file.
        {
          enforce: 'pre',
          test: /\.jsx?$/,
          include: Repack.getModulePaths(['react-native', '@react-native']),
          use: {
            loader: 'babel-loader',
            options: {
              babelrc: false,
              configFile: false,
              plugins: [
                'babel-plugin-syntax-hermes-parser',
                'babel-plugin-transform-flow-enums',
              ],
            },
          },
        },
        ...Repack.getJsTransformRules(),
        ...Repack.getAssetTransformRules(),
      ],
    },
    plugins: [
      new Repack.RepackPlugin({ platform }),
      new Repack.plugins.ModuleFederationPluginV2({
        name: 'shell',
        // `@module-federation/enhanced` injects a dev-only DTS runtime plugin
        // (`dynamic-remote-type-hints-plugin`) that opens a WebSocket to stream remote
        // type hints. That `createWebsocket` path throws `undefined cannot be used as a
        // constructor` in the React Native runtime and red-screens the app on boot. We
        // don't ship/consume `.d.ts` over the wire on-device — turn DTS off entirely.
        dts: false,
        shared: {
          react: { singleton: true, eager: true, requiredVersion: require('react/package.json').version },
          'react-native': {
            singleton: true,
            eager: true,
            requiredVersion: require('react-native/package.json').version,
          },
          '@nube/app-sdk': { singleton: true, eager: true },
        },
      }),
    ],
  };
};
