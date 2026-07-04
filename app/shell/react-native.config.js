// Wire the RN CLI to Re.Pack's Rspack commands (start/bundle) — Re.Pack replaces Metro
// (app-shell scope: Re.Pack 5 + Module Federation 2 is the one production MF path on RN).
module.exports = {
  commands: require('@callstack/repack/commands/rspack'),
};
