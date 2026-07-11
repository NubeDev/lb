// Shell i18n tests (release scope, gap d): the CI key-parity gate for the TS catalogs (the twin
// of the host's .mf parity test) + the locale chain + Spanish rendering of the login view.
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { catalogParity } from "@nube/ext-ui-sdk";
import { CATALOGS, ENABLED_LOCALES } from "./i18n";
import { App } from "./App";

afterEach(() => {
  cleanup();
  localStorage.clear();
  vi.unstubAllGlobals();
});

describe("catalog completeness (CI gate)", () => {
  it("en and es carry exactly the same key set", () => {
    expect(catalogParity(CATALOGS)).toEqual([]);
  });
  it("ships every enabled locale", () => {
    for (const loc of ENABLED_LOCALES) expect(CATALOGS[loc]).toBeTruthy();
  });
  it("no catalog message is empty", () => {
    for (const [loc, cat] of Object.entries(CATALOGS))
      for (const [key, msg] of Object.entries(cat))
        expect(msg.trim(), `${loc}/${key}`).not.toBe("");
  });
});

describe("locale resolution on the login view", () => {
  it("renders Spanish when the browser language is es (no session, no pref)", async () => {
    vi.stubGlobal("navigator", { ...navigator, language: "es-MX" });
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Iniciar sesión" })).toBeTruthy();
    expect(screen.getByPlaceholderText("contraseña")).toBeTruthy();
  });
  it("falls back to English for an unshipped browser language", async () => {
    vi.stubGlobal("navigator", { ...navigator, language: "fr-FR" });
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Sign in" })).toBeTruthy();
  });
});
