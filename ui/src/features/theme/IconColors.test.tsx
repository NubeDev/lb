// IconColors — the Settings → Theme accordion that colorizes sidebar icons. Proves the full pointer
// path: enabling auto-assigns a palette color to every rail surface (the 100-color prefilled pool,
// evenly hue-spread), each row's swatch opens the palette grid picker, Clear all fully reverts to
// default-fg icons (presence === ON, absence === OFF). Real ThemeProvider; no fakes.

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ThemeProvider, useTheme, ICON_COLOR_PALETTE } from "@/lib/theme";
import { RAIL_SURFACES } from "@/features/shell";

import { IconColors } from "./IconColors";

function IconColorsProbe() {
  const { theme } = useTheme();
  const count = theme.iconColors ? Object.keys(theme.iconColors).length : 0;
  return <output aria-label="icon-colors-count">{String(count)}</output>;
}

afterEach(() => {
  cleanup();
  localStorage.clear();
});

describe("IconColors", () => {
  it("starts disabled (no iconColors) and offers Auto-assign", async () => {
    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <IconColors />
        <IconColorsProbe />
      </ThemeProvider>,
    );
    expect(screen.getByLabelText("icon-colors-count")).toHaveTextContent("0");
    // The section's controls live inside the accordion — expand it first.
    await user.click(screen.getByRole("button", { name: /icon colors/i }));
    expect(screen.getByRole("button", { name: /auto-assign colors/i })).toBeInTheDocument();
  });

  it("auto-assigns one palette color to every rail surface on enable", async () => {
    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <IconColors />
        <IconColorsProbe />
      </ThemeProvider>,
    );
    await user.click(screen.getByRole("button", { name: /icon colors/i }));
    await user.click(screen.getByRole("button", { name: /auto-assign colors/i }));

    // Every shipped rail surface got a color from the palette — proven by the count + the per-surface
    // swatch rows now showing palette hexes rather than "default".
    const expected = String(RAIL_SURFACES.length);
    expect(screen.getByLabelText("icon-colors-count")).toHaveTextContent(expected);
    // A sample surface's row shows a palette hex (not "default").
    const sampleLabel = RAIL_SURFACES[0].label;
    const row = screen.getByLabelText(new RegExp(`${sampleLabel} icon color:`, "i"));
    const assigned = (row.getAttribute("aria-label") ?? "").split(":")[1].trim();
    expect(ICON_COLOR_PALETTE).toContain(assigned);
  });

  it("opens the 100-swatch palette grid from a row and picks a color", async () => {
    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <IconColors />
        <IconColorsProbe />
      </ThemeProvider>,
    );
    await user.click(screen.getByRole("button", { name: /icon colors/i }));
    await user.click(screen.getByRole("button", { name: /auto-assign colors/i }));

    const sampleLabel = RAIL_SURFACES[0].label;
    await user.click(screen.getByLabelText(new RegExp(`${sampleLabel} icon color:`, "i")));

    const dialog = await screen.findByRole("dialog", { name: `${sampleLabel} icon color` });
    const swatches = within(dialog).getAllByRole("button", { name: /^#[0-9a-f]{6}$/i });
    expect(swatches).toHaveLength(ICON_COLOR_PALETTE.length);

    // Picking the last swatch writes its hex onto the surface.
    const last = ICON_COLOR_PALETTE[ICON_COLOR_PALETTE.length - 1];
    await user.click(within(dialog).getByRole("button", { name: last }));
    // After pick the dialog closes; the row label now carries the chosen hex.
    expect(screen.getByLabelText(new RegExp(`${sampleLabel} icon color: ${last}`, "i"))).toBeInTheDocument();
  });

  it("Clear all fully reverts to default-fg icons (presence === OFF)", async () => {
    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <IconColors />
        <IconColorsProbe />
      </ThemeProvider>,
    );
    await user.click(screen.getByRole("button", { name: /icon colors/i }));
    await user.click(screen.getByRole("button", { name: /auto-assign colors/i }));
    expect(screen.getByLabelText("icon-colors-count")).toHaveTextContent(String(RAIL_SURFACES.length));

    await user.click(screen.getByRole("button", { name: /clear all/i }));
    expect(screen.getByLabelText("icon-colors-count")).toHaveTextContent("0");
    // Back to the disabled state — the enable affordance returns.
    expect(screen.getByRole("button", { name: /auto-assign colors/i })).toBeInTheDocument();
  });
});
