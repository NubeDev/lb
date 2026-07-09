// The onboarding wizard (setup scope) — a guided, four-step flow that gets one person from "doesn't
// exist" to "logs in and sees the right pages", orchestrating the SAME real host verbs the People /
// Teams / Roles / Nav tabs already expose (no new backend; the wizard is pure orchestration). The
// steps: (1) pick or create the user, (2) put them on a team, (3) give the team a role + a nav, (4)
// preview the exact access they'll get. Every write is re-checked server-side (rule 5); the nav is a
// lens that grants nothing.
//
// Cap-gated for DISPLAY by the same admin caps the People/Teams/Roles tabs use — the wizard only
// SHOWS controls; the gateway is the boundary. Markup + local step state; data + verbs live in
// `useSetup`. One responsibility per file (FILE-LAYOUT): this file owns the flow; steps delegate to
// small dedicated components (`PickOrCreate`, `AccessPreview`).

import { useMemo, useState } from "react";
import { ArrowLeft, ArrowRight, PartyPopper, Rocket } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { CAP, hasCap } from "@/lib/session";
import type { NavItem } from "@/lib/nav";
import { NavItemsBuilder } from "../nav/NavItemsBuilder";
import { AccessPreview } from "./AccessPreview";
import { PickOrCreate } from "./PickOrCreate";
import { Stepper, type Step } from "./Stepper";
import { useSetup } from "./useSetup";

const STEPS: Step[] = [
  { key: "user", label: "Person", hint: "Who's joining" },
  { key: "team", label: "Team", hint: "The group they're in" },
  { key: "access", label: "Access", hint: "Role & menu" },
  { key: "preview", label: "Preview", hint: "What they'll see" },
];

interface Props {
  ws: string;
  caps: string[] | undefined;
}

export function SetupWizard({ caps }: Props) {
  const setup = useSetup();
  const { sources } = setup;

  const [step, setStep] = useState(0);
  const [reached, setReached] = useState(0);
  const [msg, setMsg] = useState<string | null>(null);

  // The onboarding target, accumulated across steps.
  const [user, setUser] = useState("");
  const [team, setTeam] = useState("");
  const [role, setRole] = useState("");
  const [navId, setNavId] = useState("");
  // Nav mode: reuse an existing nav ("pick"), build a fresh one inline ("build"), or none.
  const [navMode, setNavMode] = useState<"none" | "pick" | "build">("none");
  const [newNavTitle, setNewNavTitle] = useState("");
  const [newNavItems, setNewNavItems] = useState<NavItem[]>([]);
  // The nav's items that will actually APPLY for the user (built or picked), captured on Apply so the
  // preview renders the real stripped menu (not the whole cap-allowed set). `null` = built-in sidebar.
  const [appliedNavItems, setAppliedNavItems] = useState<NavItem[] | null>(null);
  // Bumped after any grant/share so the preview re-resolves the user's effective caps.
  const [previewNonce, setPreviewNonce] = useState(0);

  const canWrite =
    hasCap(caps, CAP.userManage) || hasCap(caps, CAP.teamsManage) || hasCap(caps, CAP.grantsAssign);

  const userOptions = useMemo(
    () => sources.users.filter((u) => u.active).map((u) => ({ value: u.user, label: u.user })),
    [sources.users],
  );
  const teamOptions = useMemo(
    () => sources.teams.map((t) => ({ value: t.team, label: t.name || t.team })),
    [sources.teams],
  );

  const goto = (i: number) => {
    setMsg(null);
    setStep(i);
    setReached((r) => Math.max(r, i));
  };
  const next = () => goto(step + 1);

  // The nav that will apply for the team — either the picked one or the freshly-built one (its title
  // for the preview/summary line). `null`/undefined when the built-in sidebar is kept.
  const navTitle =
    navMode === "build"
      ? newNavTitle
      : navMode === "pick"
        ? sources.navs.find((n) => n.id === navId)?.title
        : undefined;

  // ── Per-step "advance" — each runs the real verb(s) for that step, then moves on. Errors surface on
  //    `setup.error` (the banner) and keep us on the step (the verb re-threw). ──

  const advanceUser = async () => {
    if (!user) return;
    // Ensure they're a member of the workspace roster is implicit in user.create; picking an existing
    // active user needs nothing. Just advance.
    next();
  };

  const advanceTeam = async () => {
    if (!team) return;
    try {
      // Idempotent: joining a team you're already on is a no-op relate. Always (re)assert the edge so
      // picking an existing user + existing team still wires them together.
      await setup.joinTeam(team, user);
      setMsg(`${user} is on ${team}.`);
      next();
    } catch {
      /* banner shows it */
    }
  };

  const advanceAccess = async () => {
    try {
      if (role) await setup.grantRole(team, role);
      // Resolve which nav to share: build a new one now (the shared composer's output), reuse the
      // picked one, or none (keep the built-in sidebar).
      let shareId = "";
      if (navMode === "build" && newNavTitle && newNavItems.length) {
        shareId = await setup.makeNav(newNavTitle, newNavItems);
        setNavId(shareId);
        setAppliedNavItems(newNavItems); // preview renders exactly what was built (stripped by caps)
      } else if (navMode === "pick" && navId) {
        shareId = navId;
        // Fetch the picked nav's items so the preview renders the real (stripped) menu, not the
        // whole cap-allowed set — the applied nav REPLACES the surface list in the real rail.
        const { getNav } = await import("@/lib/nav");
        setAppliedNavItems((await getNav(navId)).items ?? []);
      } else {
        setAppliedNavItems(null); // built-in sidebar → preview shows the fallback surface set
      }
      if (shareId) await setup.giveNavToTeam(shareId, team);
      setPreviewNonce((n) => n + 1);
      setMsg("Access applied.");
      next();
    } catch {
      /* banner shows it */
    }
  };

  if (!canWrite) {
    return (
      <div className="p-6 text-sm text-muted" role="status">
        You need people, teams, or grants management capabilities to run onboarding.
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col" data-testid="setup-wizard">
      {setup.error && (
        <div
          role="alert"
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-xs text-destructive"
        >
          {setup.error}
        </div>
      )}

      {/* The step rail */}
      <div className="border-b border-border bg-panel-2/40 px-4 py-3">
        <Stepper steps={STEPS} current={step} reached={reached} onJump={goto} />
      </div>

      {/* The step body */}
      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-5">
        <div className="mx-auto max-w-2xl space-y-5">
          {/* ── Step 1 — Person ── */}
          {step === 0 && (
            <StepShell
              icon={Rocket}
              title="Who are we onboarding?"
              blurb="Pick someone already in the workspace, or create a new person. A new person gets a dev login you can hand off."
            >
              <PickOrCreate
                noun="user"
                options={userOptions}
                value={user}
                onSelect={setUser}
                onCreate={async (id) => {
                  await setup.makeUser(id);
                  return id;
                }}
                slugify={(raw) => raw.trim().toLowerCase().replace(/\s+/g, "")}
              />
            </StepShell>
          )}

          {/* ── Step 2 — Team ── */}
          {step === 1 && (
            <StepShell
              icon={Rocket}
              title={`Put ${user || "them"} on a team`}
              blurb="Access is given to teams, so everyone in the team inherits it — and future members do too. Pick a team or make a new one."
            >
              <PickOrCreate
                noun="team"
                options={teamOptions}
                value={team}
                onSelect={setTeam}
                onCreate={async (id) => {
                  await setup.makeTeam(id, id);
                  return id;
                }}
              />
              {team && (
                <p className="text-xs text-muted">
                  <span className="font-medium text-fg">{user}</span> will be added to{" "}
                  <span className="font-medium text-fg">{team}</span> when you continue.
                </p>
              )}
            </StepShell>
          )}

          {/* ── Step 3 — Access (role + nav) ── */}
          {step === 2 && (
            <StepShell
              icon={Rocket}
              title={`What can ${team || "the team"} do and see?`}
              blurb="A role grants the capabilities (what they can do); a nav shapes the menu (what they see). The nav never grants — it's a lens over the role."
            >
              <div className="space-y-4">
                <label className="block space-y-1.5">
                  <span className="text-xs font-medium text-muted">Role — grants capabilities</span>
                  <Select aria-label="Role" value={role} onChange={(e) => setRole(e.target.value)}>
                    <option value="">No role (keep current access)</option>
                    {sources.roles.map((r) => (
                      <option key={r.name} value={r.name}>
                        {r.name} · {r.caps.length} cap{r.caps.length === 1 ? "" : "s"}
                      </option>
                    ))}
                  </Select>
                </label>

                <div className="space-y-2">
                  <span className="text-xs font-medium text-muted">
                    Menu — the pages the team sees (page access)
                  </span>
                  {/* Three ways to shape the menu: keep the built-in sidebar, reuse an existing nav,
                      or build a brand-new one right here with the SAME composer the Nav tab uses. */}
                  <div className="inline-flex rounded-md border border-border bg-panel p-0.5 text-xs">
                    {(
                      [
                        ["none", "Built-in sidebar"],
                        ["pick", "Existing nav"],
                        ["build", "Build a new nav"],
                      ] as const
                    ).map(([m, label]) => (
                      <Button
                        key={m}
                        size="sm"
                        variant={navMode === m ? "solid" : "ghost"}
                        className="h-7 rounded px-3 text-xs"
                        onClick={() => setNavMode(m)}
                        aria-label={label}
                        aria-pressed={navMode === m}
                      >
                        {label}
                      </Button>
                    ))}
                  </div>

                  {navMode === "pick" && (
                    <Select aria-label="Nav" value={navId} onChange={(e) => setNavId(e.target.value)}>
                      <option value="">
                        {sources.navs.length === 0 ? "No navs yet — build one instead" : "Choose a nav…"}
                      </option>
                      {sources.navs.map((n) => (
                        <option key={n.id} value={n.id}>
                          {n.title}
                        </option>
                      ))}
                    </Select>
                  )}

                  {navMode === "build" && (
                    // The shared composer, reused verbatim from the Nav tab — pick surfaces/dashboards/
                    // ext pages, reorder, group. The wizard names + shares it to the team on Apply.
                    <div className="space-y-3 rounded-md border border-border bg-panel px-3 py-3">
                      <NavItemsBuilder
                        title={newNavTitle}
                        onTitleChange={setNewNavTitle}
                        items={newNavItems}
                        onItemsChange={setNewNavItems}
                        sources={{
                          dashboards: sources.dashboards,
                          extensions: sources.extensions,
                        }}
                      />
                    </div>
                  )}
                </div>

                {navMode === "pick" && navId && (
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-xs"
                    onClick={() => void setup.makeNavDefault(navId)}
                    aria-label="Also make this the workspace default nav"
                  >
                    Also make <span className="mx-1 font-medium">{navTitle}</span> the workspace default
                  </Button>
                )}
              </div>
            </StepShell>
          )}

          {/* ── Step 4 — Preview ── */}
          {step === 3 && (
            <StepShell
              icon={PartyPopper}
              title={`${user} is ready`}
              blurb="Here's the exact sidebar they'll see on first login, resolved live from their effective access."
            >
              <AccessPreview
                user={user}
                resolve={setup.effectiveCaps}
                nonce={previewNonce}
                navItems={appliedNavItems}
                navTitle={navTitle}
              />
              <div className="mt-4 flex flex-wrap items-center gap-2 rounded-lg border border-border bg-panel px-3 py-2.5 text-sm">
                <Badge variant="secondary">{user}</Badge>
                <span className="text-muted">on</span>
                <Badge variant="outline">{team}</Badge>
                {role && (
                  <>
                    <span className="text-muted">·</span>
                    <Badge variant="secondary">role: {role}</Badge>
                  </>
                )}
                {navTitle && (
                  <>
                    <span className="text-muted">·</span>
                    <Badge variant="outline">menu: {navTitle}</Badge>
                  </>
                )}
              </div>
            </StepShell>
          )}
        </div>
      </div>

      {/* The footer nav — Back / (message) / primary advance. */}
      <div className="flex items-center gap-3 border-t border-border bg-panel-2/40 px-4 py-3">
        <Button
          variant="outline"
          size="sm"
          disabled={step === 0}
          onClick={() => goto(step - 1)}
          aria-label="Back"
        >
          <ArrowLeft size={14} /> Back
        </Button>
        {msg && <span className="text-xs text-muted">{msg}</span>}
        <div className="ml-auto">
          {step === 0 && (
            <Button size="sm" disabled={!user} onClick={() => void advanceUser()} aria-label="Continue">
              Continue <ArrowRight size={14} />
            </Button>
          )}
          {step === 1 && (
            <Button size="sm" disabled={!team} onClick={() => void advanceTeam()} aria-label="Add to team and continue">
              Add &amp; continue <ArrowRight size={14} />
            </Button>
          )}
          {step === 2 && (
            <Button size="sm" onClick={() => void advanceAccess()} aria-label="Apply access and preview">
              Apply &amp; preview <ArrowRight size={14} />
            </Button>
          )}
          {step === 3 && (
            <Button
              size="sm"
              onClick={() => {
                // Reset for the next person; keep the team/role/nav so onboarding a cohort is fast.
                setUser("");
                setStep(0);
                setReached(0);
                setMsg(null);
                void setup.reload();
              }}
              aria-label="Onboard another person"
            >
              <PartyPopper size={14} /> Onboard another
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}

// A consistent step frame — an icon-badged title, a one-line blurb, then the step's controls. Keeps
// every step visually identical so the wizard reads as one flow.
function StepShell({
  icon: Icon,
  title,
  blurb,
  children,
}: {
  icon: LucideIcon;
  title: string;
  blurb: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-4">
      <header className="flex items-start gap-3">
        <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
          <Icon size={18} />
        </span>
        <div className="min-w-0">
          <h3 className="text-base font-semibold text-fg">{title}</h3>
          <p className="mt-0.5 text-sm text-muted">{blurb}</p>
        </div>
      </header>
      {children}
    </section>
  );
}
