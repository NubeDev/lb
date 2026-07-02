// ortho-top (flat drawing surface, orbit LOCKED) ↔ persp (3D), one ~600 ms spring
// (look-scope.md §motion). The scene must feel like the same object seen differently.

export function CameraRig(_props: { mode: "ortho-top" | "persp" }) {
  // TODO(phase 1): OrthographicCamera top-down + MapControls (pan/zoom only);
  // TODO(phase 4): perspective + OrbitControls, spring transition between the two.
  return null;
}
