import { useEffect, useMemo, useState } from "react";
import { invoke } from "../ipc";
import { Disclosure, Hint } from "./ui";

export type StackRecipe = {
  id: string;
  name: string;
  description: string;
  runtimes: string[];
  toolchain: Record<string, string>;
  commands: {
    build: string | string[] | null;
    lint: string | string[] | null;
    format: string | string[] | null;
    test: string | string[] | null;
  };
  era?: string;
  domain?: string;
  tags?: string[];
};

export type ScaffoldFilePlan = {
  relative: string;
  action: "create" | "update" | "preserve";
};

export type ScaffoldResult = {
  recipe_id: string;
  project_name: string;
  agents_path: string;
  recovered_interrupted: boolean;
  files: ScaffoldFilePlan[];
};

type FitAnswers = {
  intent: string;
  primary_runtime: string;
  ui_surface: string;
  evidence: string;
  compliance: string;
  repo_state: string;
  host: string;
};

type ScoredRecipe = {
  id: string;
  name: string;
  score: number;
  why: string[];
  era: string;
  domain: string;
};

type RecipeWizardProps = {
  recipes: StackRecipe[];
  busy: boolean;
  plan: ScaffoldFilePlan[] | null;
  planError: string | null;
  lastResult: ScaffoldResult | null;
  simpleMode?: boolean;
  onPreview: (input: {
    recipe: string;
    projectName: string;
    force: boolean;
  }) => void;
  onInitialize: (input: {
    recipe: string;
    projectName: string;
    force: boolean;
  }) => void;
};

const EMPTY_FIT: FitAnswers = {
  intent: "",
  primary_runtime: "",
  ui_surface: "",
  evidence: "",
  compliance: "",
  repo_state: "",
  host: "",
};

const FIT_FIELDS: {
  key: keyof FitAnswers;
  label: string;
  options: { value: string; label: string }[];
}[] = [
  {
    key: "intent",
    label: "Intent",
    options: [
      { value: "", label: "Any" },
      { value: "product", label: "Product" },
      { value: "lib", label: "Library" },
      { value: "ops", label: "Ops / process" },
    ],
  },
  {
    key: "primary_runtime",
    label: "Runtime",
    options: [
      { value: "", label: "Any" },
      { value: "rust", label: "Rust" },
      { value: "node", label: "Node" },
      { value: "python", label: "Python" },
      { value: "mixed", label: "Mixed" },
    ],
  },
  {
    key: "ui_surface",
    label: "UI",
    options: [
      { value: "", label: "Any" },
      { value: "none", label: "None / API" },
      { value: "web", label: "Web" },
      { value: "desktop", label: "Desktop" },
      { value: "mobile", label: "Mobile" },
      { value: "game", label: "Game" },
    ],
  },
  {
    key: "evidence",
    label: "Done evidence",
    options: [
      { value: "", label: "Any" },
      { value: "http", label: "HTTP contract" },
      { value: "playwright", label: "Playwright" },
      { value: "binary", label: "Binary / install" },
      { value: "device", label: "Device" },
      { value: "hil", label: "Hardware HIL" },
      { value: "plan", label: "Plan checklist" },
    ],
  },
  {
    key: "compliance",
    label: "Compliance",
    options: [
      { value: "", label: "Any" },
      { value: "none", label: "None" },
      { value: "regulated", label: "Regulated" },
    ],
  },
  {
    key: "repo_state",
    label: "Repo",
    options: [
      { value: "", label: "Any" },
      { value: "empty", label: "New / empty" },
      { value: "existing", label: "Existing" },
    ],
  },
  {
    key: "host",
    label: "Host",
    options: [
      { value: "", label: "Any" },
      { value: "windows", label: "Windows" },
      { value: "wsl", label: "WSL" },
      { value: "macos", label: "macOS" },
      { value: "linux", label: "Linux" },
    ],
  },
];

function eraLabel(era?: string): string {
  if (era === "classic") return "Classic";
  if (era === "frontier") return "Frontier";
  return "Modern";
}

/** Stack Fit + browse catalog → preview/initialize trust contract. */
export function RecipeWizard({
  recipes,
  busy,
  plan,
  planError,
  lastResult,
  simpleMode = false,
  onPreview,
  onInitialize,
}: RecipeWizardProps) {
  const [selected, setSelected] = useState("");
  const [projectName, setProjectName] = useState("");
  const [force, setForce] = useState(false);
  const [fit, setFit] = useState<FitAnswers>(EMPTY_FIT);
  const [ranked, setRanked] = useState<ScoredRecipe[]>([]);
  const [eraFilter, setEraFilter] = useState<string>("");
  const [domainFilter, setDomainFilter] = useState<string>("");
  const [query, setQuery] = useState("");

  useEffect(() => {
    if (!selected && recipes[0]) setSelected(recipes[0].id);
  }, [recipes, selected]);

  useEffect(() => {
    if (!selected) return;
    onPreview({ recipe: selected, projectName, force });
  }, [selected, projectName, force, onPreview]);

  useEffect(() => {
    let cancelled = false;
    void invoke<ScoredRecipe[]>("rank_recipes", { answers: fit })
      .then((next) => {
        if (!cancelled) setRanked(next);
      })
      .catch(() => {
        if (!cancelled) setRanked([]);
      });
    return () => {
      cancelled = true;
    };
  }, [fit]);

  const domains = useMemo(() => {
    const set = new Set(
      recipes.map((r) => r.domain).filter((d): d is string => Boolean(d)),
    );
    return [...set].sort();
  }, [recipes]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return recipes.filter((item) => {
      if (item.id === "node-web") return false;
      if (eraFilter && (item.era ?? "modern") !== eraFilter) return false;
      if (domainFilter && item.domain !== domainFilter) return false;
      if (!q) return true;
      const hay = [
        item.name,
        item.id,
        item.description,
        item.domain ?? "",
        item.era ?? "",
        ...(item.tags ?? []),
      ]
        .join(" ")
        .toLowerCase();
      return hay.includes(q);
    });
  }, [recipes, eraFilter, domainFilter, query]);

  const topMatches = ranked.slice(0, 3);
  const recipe = recipes.find((item) => item.id === selected);
  const whyForSelected = ranked.find((r) => r.id === selected)?.why ?? [];

  const selectChip = (
    key: keyof FitAnswers,
    value: string,
    label: string,
    active: boolean,
  ) => (
    <button
      key={`${key}-${value || "any"}`}
      type="button"
      onClick={() => setFit((prev) => ({ ...prev, [key]: value }))}
      className={`rounded-md border px-2 py-1 text-[11px] ${
        active
          ? "border-blue-400/40 bg-blue-500/15 text-blue-100"
          : "border-white/10 text-slate-500 hover:text-slate-300"
      }`}
    >
      {label}
    </button>
  );

  return (
    <div className="grid grid-cols-1 gap-5 lg:grid-cols-[1fr_360px]">
      <div className="space-y-4">
        <section className="rounded-2xl border border-white/7 bg-[#0d121a]/85 p-5">
          <div className="mb-4 flex flex-wrap items-center gap-2">
            <h2 className="text-sm font-semibold text-slate-100">Stack Fit</h2>
            <Hint text="Answer a few variables — ADE ranks trust contracts (verify ladder + G5), not just templates." />
          </div>
          <div className={`space-y-3 ${simpleMode ? "" : ""}`}>
            {FIT_FIELDS.filter((field) =>
              simpleMode
                ? ["intent", "primary_runtime", "ui_surface", "evidence", "compliance"].includes(
                    field.key,
                  )
                : true,
            ).map((field) => (
              <div key={field.key}>
                <div className="mb-1.5 text-[10px] uppercase tracking-wider text-slate-600">
                  {field.label}
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {field.options.map((opt) =>
                    selectChip(
                      field.key,
                      opt.value,
                      opt.label,
                      fit[field.key] === opt.value,
                    ),
                  )}
                </div>
              </div>
            ))}
          </div>

          <div className="mt-5">
            <div className="mb-2 text-[10px] uppercase tracking-wider text-slate-600">
              Best matches
            </div>
            {topMatches.length === 0 ? (
              <p className="text-[11px] text-slate-600">Ranking recipes…</p>
            ) : (
              <div className="space-y-2">
                {topMatches.map((item, index) => (
                  <button
                    key={item.id}
                    type="button"
                    onClick={() => setSelected(item.id)}
                    className={`w-full rounded-xl border p-3 text-left transition ${
                      selected === item.id
                        ? "border-blue-400/40 bg-blue-500/10"
                        : "border-white/7 bg-white/2 hover:border-white/15"
                    }`}
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-sm font-medium text-slate-200">
                        {index + 1}. {item.name}
                      </span>
                      {!simpleMode && (
                        <span className="font-mono text-[10px] text-slate-500">
                          {item.score}
                        </span>
                      )}
                    </div>
                    <div className="mt-1.5 flex flex-wrap gap-1.5">
                      {item.why.slice(0, 2).map((reason) => (
                        <span
                          key={reason}
                          className="rounded border border-white/8 bg-white/4 px-1.5 py-0.5 text-[10px] text-slate-400"
                        >
                          {reason}
                        </span>
                      ))}
                    </div>
                  </button>
                ))}
              </div>
            )}
          </div>
        </section>

        <Disclosure
          title="Browse all recipes"
          subtitle="Eras, domains, search"
          summary={`${filtered.length}`}
          defaultOpen={!simpleMode}
          storageKey="ade_recipes_browse_open"
        >
          <div className="mb-3 flex flex-wrap gap-1.5">
            {["", "classic", "modern", "frontier"].map((era) => (
              <button
                key={era || "all-era"}
                type="button"
                onClick={() => setEraFilter(era)}
                className={`rounded-md border px-2 py-1 text-[11px] ${
                  eraFilter === era
                    ? "border-blue-400/40 bg-blue-500/15 text-blue-100"
                    : "border-white/10 text-slate-500 hover:text-slate-300"
                }`}
              >
                {era ? eraLabel(era) : "All eras"}
              </button>
            ))}
          </div>
          <div className="mb-3 flex flex-wrap gap-1.5">
            <button
              type="button"
              onClick={() => setDomainFilter("")}
              className={`rounded-md border px-2 py-1 text-[11px] ${
                !domainFilter
                  ? "border-blue-400/40 bg-blue-500/15 text-blue-100"
                  : "border-white/10 text-slate-500 hover:text-slate-300"
              }`}
            >
              All domains
            </button>
            {domains.map((domain) => (
              <button
                key={domain}
                type="button"
                onClick={() => setDomainFilter(domain)}
                className={`rounded-md border px-2 py-1 text-[11px] ${
                  domainFilter === domain
                    ? "border-blue-400/40 bg-blue-500/15 text-blue-100"
                    : "border-white/10 text-slate-500 hover:text-slate-300"
                }`}
              >
                {domain}
              </button>
            ))}
          </div>
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search name, tags…"
            className="mb-3 w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 text-xs text-slate-200"
          />
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            {filtered.map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => setSelected(item.id)}
                className={`rounded-xl border p-3 text-left transition ${
                  selected === item.id
                    ? "border-blue-400/40 bg-blue-500/10"
                    : "border-white/7 bg-white/2 hover:border-white/15"
                }`}
              >
                <div className="text-sm font-medium text-slate-200">{item.name}</div>
                {!simpleMode && (
                  <div className="mt-0.5 font-mono text-[10px] text-blue-300/70">{item.id}</div>
                )}
                <div className="mt-1 flex flex-wrap gap-1 text-[10px] text-slate-500">
                  <span>{eraLabel(item.era)}</span>
                  {item.domain ? <span>· {item.domain}</span> : null}
                </div>
                <p className="mt-2 text-[11px] leading-5 text-slate-500">{item.description}</p>
              </button>
            ))}
          </div>
        </Disclosure>
      </div>

      <section className="rounded-2xl border border-white/7 bg-[#0d121a]/85 p-5">
        <div className="mb-5">
          <h2 className="text-sm font-semibold">{recipe?.name ?? "Recipe setup"}</h2>
          <p className="mt-1 text-[11px] text-slate-600">
            Preview files, then initialize the trust contract
          </p>
        </div>
        {recipe ? (
          <div className="space-y-4">
            {whyForSelected.length > 0 && (
              <div className="flex flex-wrap gap-1.5">
                {whyForSelected.map((reason) => (
                  <span
                    key={`sel-${reason}`}
                    className="rounded border border-emerald-400/20 bg-emerald-400/5 px-1.5 py-0.5 text-[10px] text-emerald-100/80"
                  >
                    {reason}
                  </span>
                ))}
              </div>
            )}
            <label className="block text-[11px] text-slate-500">
              Project name (optional)
              <input
                value={projectName}
                onChange={(event) => setProjectName(event.target.value)}
                className="mt-1.5 w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 text-xs text-slate-200"
              />
            </label>
            <div>
              <div className="text-[10px] uppercase tracking-wider text-slate-600">Toolchain</div>
              <div className="mt-2 space-y-1 text-xs text-slate-400">
                {Object.entries(recipe.toolchain).length === 0 ? (
                  <div className="text-slate-600">No toolchain pins</div>
                ) : (
                  Object.entries(recipe.toolchain).map(([name, version]) => (
                    <div key={name} className="flex justify-between gap-3">
                      <span>{name}</span>
                      <span className="font-mono text-slate-500">{version}</span>
                    </div>
                  ))
                )}
              </div>
            </div>

            <div>
              <div className="text-[10px] uppercase tracking-wider text-slate-600">
                Planned file set
              </div>
              {planError ? (
                <p className="mt-2 text-[11px] leading-5 text-amber-300/90">{planError}</p>
              ) : plan && plan.length > 0 ? (
                <div className="mt-2 space-y-1">
                  {plan.map((file) => (
                    <div
                      key={file.relative}
                      className="flex items-center justify-between text-[11px] text-slate-400"
                    >
                      <span className="font-mono text-slate-300">{file.relative}</span>
                      <span
                        className={
                          file.action === "create"
                            ? "text-emerald-400"
                            : file.action === "update"
                              ? "text-blue-300"
                              : "text-slate-500"
                        }
                      >
                        {file.action}
                      </span>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="mt-2 text-[11px] text-slate-600">Computing plan…</p>
              )}
            </div>

            <label className="flex items-start gap-2 text-[11px] leading-5 text-slate-500">
              <input
                type="checkbox"
                checked={force}
                onChange={(event) => setForce(event.target.checked)}
                className="mt-1 size-3.5 accent-red-400"
              />
              Replace existing AGENTS.md (keep off to preserve authority)
            </label>
            <button
              type="button"
              onClick={() => onInitialize({ recipe: recipe.id, projectName, force })}
              disabled={busy || !!planError}
              className="w-full rounded-lg bg-violet-500 px-4 py-2.5 text-xs font-semibold hover:bg-violet-400 disabled:opacity-50"
            >
              {busy ? "Initializing…" : `Initialize ${recipe.name}`}
            </button>

            {lastResult ? (
              <div className="rounded-lg border border-white/8 bg-white/2 p-3">
                <div className="text-[10px] uppercase tracking-wider text-slate-600">
                  Last transaction
                </div>
                <div className="mt-2 text-[11px] text-slate-300">
                  {lastResult.recipe_id} · {lastResult.files.length} file(s)
                  {lastResult.recovered_interrupted ? " · recovered prior journal" : ""}
                </div>
                <div className="mt-2 space-y-1">
                  {lastResult.files.map((file) => (
                    <div
                      key={`result-${file.relative}`}
                      className="flex justify-between font-mono text-[10px] text-slate-500"
                    >
                      <span>{file.relative}</span>
                      <span>{file.action}</span>
                    </div>
                  ))}
                </div>
              </div>
            ) : null}
          </div>
        ) : (
          <div className="py-16 text-center text-xs text-slate-500">Loading recipes…</div>
        )}
      </section>
    </div>
  );
}
