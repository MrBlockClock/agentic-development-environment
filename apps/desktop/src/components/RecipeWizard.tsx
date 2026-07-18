import { useEffect, useState } from "react";

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

type RecipeWizardProps = {
  recipes: StackRecipe[];
  busy: boolean;
  plan: ScaffoldFilePlan[] | null;
  planError: string | null;
  lastResult: ScaffoldResult | null;
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

export function RecipeWizard({
  recipes,
  busy,
  plan,
  planError,
  lastResult,
  onPreview,
  onInitialize,
}: RecipeWizardProps) {
  const [selected, setSelected] = useState("");
  const [projectName, setProjectName] = useState("");
  const [force, setForce] = useState(false);

  useEffect(() => {
    if (!selected && recipes[0]) setSelected(recipes[0].id);
  }, [recipes, selected]);

  useEffect(() => {
    if (!selected) return;
    onPreview({ recipe: selected, projectName, force });
  }, [selected, projectName, force, onPreview]);

  const recipe = recipes.find((item) => item.id === selected);
  return (
    <div className="grid grid-cols-[1fr_360px] gap-5">
      <section className="rounded-2xl border border-white/7 bg-[#0d121a]/85 p-5 shadow-[0_12px_45px_rgba(0,0,0,0.15)]">
        <div className="mb-5">
          <h2 className="text-sm font-semibold">Stack recipes</h2>
          <p className="mt-1 text-[11px] text-slate-600">
            Choose a safe starting contract for this workspace
          </p>
        </div>
        <div className="grid grid-cols-2 gap-3">
          {recipes.map((item) => (
            <button
              key={item.id}
              onClick={() => setSelected(item.id)}
              className={`rounded-xl border p-4 text-left transition ${
                selected === item.id
                  ? "border-blue-400/40 bg-blue-500/10"
                  : "border-white/7 bg-white/2 hover:border-white/15"
              }`}
            >
              <div className="text-sm font-medium text-slate-200">{item.name}</div>
              <div className="mt-1 font-mono text-[10px] text-blue-300/70">{item.id}</div>
              <p className="mt-3 text-[11px] leading-5 text-slate-500">{item.description}</p>
            </button>
          ))}
        </div>
      </section>

      <section className="rounded-2xl border border-white/7 bg-[#0d121a]/85 p-5 shadow-[0_12px_45px_rgba(0,0,0,0.15)]">
        <div className="mb-5">
          <h2 className="text-sm font-semibold">{recipe?.name ?? "Recipe setup"}</h2>
          <p className="mt-1 text-[11px] text-slate-600">
            Transactional bootstrap with rollback journal under .ade/scaffold
          </p>
        </div>
        {recipe ? (
          <div className="space-y-4">
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
                {Object.entries(recipe.toolchain).map(([name, version]) => (
                  <div key={name} className="flex justify-between gap-3">
                    <span>{name}</span>
                    <span className="font-mono text-slate-500">{version}</span>
                  </div>
                ))}
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
              Replace an existing AGENTS.md. Leave off to preserve repository authority.
            </label>
            <button
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
