// ESLint flat config — enforces the UI standard (scope/frontend/ui-standards-scope.md):
// shadcn/ui primitives only, no parallel control layer. Plus react-hooks correctness.
// Keep this config focused; it is not a general style linter.
//
// Enforcement is FORWARD-ONLY. The standard is an ERROR everywhere, so new/edited code must
// conform. The views that predate the standard are listed in LEGACY_VIEWS, where the standard
// is downgraded to a WARN — visible and counted, but not yet build-breaking. The migration
// (ui-standards-scope.md "Migration") is: convert a view, then delete it from LEGACY_VIEWS so
// it becomes guarded. When the list is empty, flip `lint` to `--max-warnings 0`.

import tseslint from "typescript-eslint";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";

// The legacy globals.css control classes that the shadcn primitives replace. Using any of
// these is the fork this standard closes — use the components/ui/* primitive instead.
// (control-field* → Input, soft-button*/danger-button-sm → Button, scope-pill → Badge,
//  page-header* → AppPageHeader, state-alert → Alert, icon-button → Button size="icon".)
const LEGACY_CLASS =
  /\b(control-field(-sm)?|soft-button(-sm)?|danger-button-sm|icon-button|scope-pill|page-header(-icon)?|page-title|page-subtitle|state-alert)\b/;

// Raw HTML controls that must come from components/ui/* instead.
const RAW_CONTROLS = ["button", "input", "select", "textarea"];

const cap = (s) => s[0].toUpperCase() + s.slice(1);

// The standard, expressed as no-restricted-syntax selectors (severity supplied per-block).
const standardSelectors = [
  ...RAW_CONTROLS.map((name) => ({
    selector: `JSXOpeningElement[name.name='${name}']`,
    message: `Use the shadcn <${cap(name)}> primitive from @/components/ui/* instead of a raw <${name}>. See scope/frontend/ui-standards-scope.md.`,
  })),
  {
    selector: `JSXAttribute[name.name='className'] Literal[value=/${LEGACY_CLASS.source}/]`,
    message:
      "Legacy globals.css control class is banned — use the shadcn primitive (Button/Input/Badge/AppPageHeader/Alert). See scope/frontend/ui-standards-scope.md.",
  },
  {
    selector: `JSXAttribute[name.name='className'] TemplateElement[value.raw=/${LEGACY_CLASS.source}/]`,
    message:
      "Legacy globals.css control class is banned — use the shadcn primitive (Button/Input/Badge/AppPageHeader/Alert). See scope/frontend/ui-standards-scope.md.",
  },
];

// Views that predate the standard. Convert one → remove it here. SHRINKS toward empty.
const LEGACY_VIEWS = [
  "src/features/admin/PeopleAdmin.tsx",
  "src/features/admin/RolesAdmin.tsx",
  "src/features/admin/TeamsAdmin.tsx",
  "src/features/admin/WorkspacesAdmin.tsx",
  "src/features/agent/AgentView.tsx",
  "src/features/dashboard/AddWidget.tsx",
  "src/features/dashboard/DashboardRoster.tsx",
  "src/features/dashboard/DashboardView.tsx",
  "src/features/dashboard/Grid.tsx",
  "src/features/dashboard/vars/VariableEditor.tsx",
  "src/features/data/DataView.tsx",
  "src/features/inbox/InboxView.tsx",
  "src/features/ingest/CreateSeriesWizard.tsx",
  "src/features/ingest/IngestView.tsx",
  "src/features/ingest/SchemaBuilder.tsx",
  "src/features/ingest/SchemaForm.tsx",
  "src/features/outbox/OutboxView.tsx",
  "src/features/session/LoginView.tsx",
  "src/features/studio/StudioView.tsx",
  "src/features/workflow/WorkflowView.tsx",
  "src/features/workspace/WorkspaceSwitcher.tsx",
];

export default tseslint.config(
  {
    ignores: [
      "dist/**",
      "node_modules/**",
      "src-tauri/**",
      "**/*.config.{js,ts}",
      "src/components/ui/**", // the primitives themselves wrap raw elements — by design.
    ],
  },
  {
    files: ["src/**/*.{ts,tsx}"],
    ignores: ["src/**/*.test.{ts,tsx}", "src/**/*.spec.{ts,tsx}"],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    plugins: { react, "react-hooks": reactHooks },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "no-restricted-syntax": ["error", ...standardSelectors],
    },
  },
  // Forward-only: the unmigrated views warn rather than error, so the standard lands without a
  // 153-error wall. Remove a path above as it's migrated to make it build-breaking again.
  {
    files: LEGACY_VIEWS,
    rules: {
      "no-restricted-syntax": ["warn", ...standardSelectors],
    },
  },
);
