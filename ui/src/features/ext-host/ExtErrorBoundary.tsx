// A crash wall around a mounted extension page. A federated extension renders in-process against the
// shell's React (see ExtHost) — so an error THROWN DURING RENDER inside the extension (a bad mount, a
// transport that reads a detached `this`, a component-stack throw) would otherwise unwind the WHOLE
// shell tree: the nav, the sidebar, everything. That is exactly the "main sidebar disappears when I
// open the extension" symptom. This boundary contains the blast radius to the extension surface: the
// extension shows an honest error card, the shell keeps rendering.
//
// It also keys on `resetKey` (the ext id) so navigating to a DIFFERENT extension after one crashed
// re-arms the boundary instead of showing the stale error.

import { Component, type ReactNode } from "react";

interface Props {
  ext: string;
  resetKey: string;
  children: ReactNode;
}

interface State {
  error: Error | null;
  key: string;
}

export class ExtErrorBoundary extends Component<Props, State> {
  state: State = { error: null, key: this.props.resetKey };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error };
  }

  static getDerivedStateFromProps(props: Props, state: State): Partial<State> | null {
    // A new resetKey (the user navigated to another extension) clears a prior crash.
    if (props.resetKey !== state.key) return { error: null, key: props.resetKey };
    return null;
  }

  render() {
    if (this.state.error) {
      return (
        <div className="m-4 rounded-md border border-border bg-panel p-4 text-sm text-muted">
          <span className="text-accent">{this.props.ext}</span> crashed while rendering:{" "}
          {this.state.error.message}
        </div>
      );
    }
    return this.props.children;
  }
}
