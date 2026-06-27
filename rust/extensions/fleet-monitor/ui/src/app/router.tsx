import { MemoryRouter, Route, Routes } from "react-router-dom";

import { Overview } from "@/pages/Overview";
import { Nodes } from "@/pages/Nodes";
import { Alerts } from "@/pages/Alerts";

/** The nested route tree, rooted in a MemoryRouter so the page never touches the shell's URL.
 *  `Overview` is the parent layout; `Nodes` (index) and `Alerts` render in its Outlet. */
export function Router() {
  return (
    <MemoryRouter initialEntries={["/"]}>
      <Routes>
        <Route path="/" element={<Overview />}>
          <Route index element={<Nodes />} />
          <Route path="alerts" element={<Alerts />} />
        </Route>
      </Routes>
    </MemoryRouter>
  );
}
