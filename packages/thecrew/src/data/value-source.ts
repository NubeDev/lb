// THE data seam (thecrew-scope.md §reuse #2). Shapes never fetch — they receive
// values resolved through this interface. Playground impl: simulator.ts (the one
// declared fake). Framework impl: the host-mediated bridge (viewer's grant).

export type Unsubscribe = () => void;

export interface ValueSource {
  /** current value, if known */
  get(channel: string): unknown;
  /** live updates; fires immediately with the current value */
  subscribe(channel: string, onValue: (value: unknown) => void): Unsubscribe;
  /** channels available to the PropertyRail's binding picker */
  channels(): string[];
}
