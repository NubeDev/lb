// Pre-auth login branding (workspace-branding scope): the sign-in card paints the entered
// workspace's brand from the workspace-keyed localStorage boot cache — heading, sub-heading, and
// login logo — with no token and no gateway. A never-cached workspace falls back to the neutral
// defaults. Pure render over a seeded `localStorage`; the real set/read path is exercised by the
// branding prefs + gateway tests.

import { describe, expect, it, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { LoginView } from "./LoginView";
import { saveCachedBrand, DEFAULT_BRANDING, BRANDING_PLACEHOLDERS } from "@/lib/branding";

beforeEach(() => localStorage.clear());

const noop = async () => {};

describe("LoginView pre-auth branding", () => {
  it("renders the neutral defaults when no brand is cached for the workspace", () => {
    render(<LoginView onSignIn={noop} />);
    // Default workspace is `acme` and has no cache → generic sign-in prompt, no logo image.
    expect(screen.getByRole("heading", { name: BRANDING_PLACEHOLDERS.loginHeading })).toBeInTheDocument();
    expect(screen.getByText(BRANDING_PLACEHOLDERS.loginSubheading)).toBeInTheDocument();
    expect(screen.queryByRole("img")).toBeNull();
  });

  it("paints the cached brand for the entered workspace", async () => {
    const logo = "data:image/png;base64,AAAA";
    saveCachedBrand("acme", {
      ...DEFAULT_BRANDING,
      loginHeading: "Sign in to Acme",
      loginSubheading: "Access your Acme workspace.",
      loginLogoDataUri: logo,
    });

    const { container } = render(<LoginView onSignIn={noop} />);
    expect(screen.getByRole("heading", { name: "Sign in to Acme" })).toBeInTheDocument();
    expect(screen.getByText("Access your Acme workspace.")).toBeInTheDocument();
    // The login logo is decorative (alt=""), so query the element directly rather than by role.
    expect(container.querySelector("img")).toHaveAttribute("src", logo);
  });

  it("re-brands live as the visitor edits the workspace field", async () => {
    saveCachedBrand("beta", {
      ...DEFAULT_BRANDING,
      loginHeading: "Welcome to Beta",
    });
    const user = userEvent.setup();
    render(<LoginView onSignIn={noop} />);

    // Starts on `acme` (no cache) → default heading.
    expect(screen.getByRole("heading", { name: BRANDING_PLACEHOLDERS.loginHeading })).toBeInTheDocument();

    await user.clear(screen.getByLabelText("workspace"));
    await user.type(screen.getByLabelText("workspace"), "beta");
    expect(screen.getByRole("heading", { name: "Welcome to Beta" })).toBeInTheDocument();
  });
});
