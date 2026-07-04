// Paste-to-import — a textarea for a tweakcn/shadcn CSS theme block. On Apply, the theme layer's
// `parseImportedTheme` turns it into base-token light/dark palettes; a malformed paste fails closed
// (the field shows an error, the current theme is untouched). One component per file (FILE-LAYOUT).

import * as React from "react";

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { parseImportedTheme, useTheme } from "@/lib/theme";

export function ImportField() {
  const { setImported } = useTheme();
  const [css, setCss] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);

  const apply = () => {
    const parsed = parseImportedTheme(css);
    if (!parsed) {
      setError("Could not parse a theme from that CSS. Paste a tweakcn :root { … } .dark { … } block.");
      return;
    }
    setError(null);
    setImported(parsed);
  };

  return (
    <div className="space-y-2">
      <Label htmlFor="theme-import">Import theme (paste tweakcn CSS)</Label>
      <Textarea
        id="theme-import"
        aria-label="Import theme CSS"
        rows={4}
        placeholder=":root { --background: …; --primary: …; } .dark { … }"
        value={css}
        onChange={(e) => {
          setCss(e.target.value);
          if (error) setError(null);
        }}
        className="font-mono text-xs"
      />
      {error && (
        <p role="alert" className="text-xs text-red-500">
          {error}
        </p>
      )}
      <Button type="button" variant="outline" size="sm" className="w-full" onClick={apply} disabled={!css.trim()}>
        Apply imported theme
      </Button>
    </div>
  );
}
