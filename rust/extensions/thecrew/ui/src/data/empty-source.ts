// The inert default ValueSource — the seam's null object. Replaces the deleted playground
// simulator (CLAUDE §9: no fakes inside the extension). It knows no channels and resolves every
// binding to `null`, so a shape with no injected source renders its no-access/empty state instead
// of crashing. The REAL source is always provided by the mount shell (createBridgeSource) or a
// test (a seeded/stub source through the ValueSource seam).

import type { ValueSource, Unsubscribe } from "./value-source";

export function createEmptySource(): ValueSource {
  return {
    get(): unknown {
      return null;
    },
    subscribe(_channel: string, onValue: (value: unknown) => void): Unsubscribe {
      onValue(null); // fire immediately with null, per the seam contract
      return () => {};
    },
    channels(): string[] {
      return [];
    },
  };
}
