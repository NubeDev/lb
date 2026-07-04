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
      rules: [...Repack.getJsTransformRules(), ...Repack.getAssetTransformRules()],
    },
    plugins: [
      new Repack.RepackPlugin({ platform }),
      new Repack.plugins.ModuleFederationPluginV2({
        name: 'shell',
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
