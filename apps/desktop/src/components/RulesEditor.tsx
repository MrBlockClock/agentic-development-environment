import { useEffect, useState } from "react";
import { invoke } from "../ipc";

type RuleFile = {
  source: string;
  description: string;
  globs: string[];
  deny_writes: boolean;
  content: string;
};

type SkillFile = {
  name: string;
  description: string;
  always_apply: boolean;
  body: string;
  source: string;
};

type SkillActivation = "always" | "catalog";

function skillActivation(skill: SkillFile): SkillActivation {
  return skill.always_apply ? "always" : "catalog";
}

/** Browse `.ade/rules` and `.ade/skills` loaded by the agent runtime. */
export function RulesEditor() {
  const [rules, setRules] = useState<RuleFile[]>([]);
  const [skills, setSkills] = useState<SkillFile[]>([]);
  const [selected, setSelected] = useState<{ kind: "rule" | "skill"; id: string } | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      setLoading(true);
      setError(null);
      try {
        const [nextRules, nextSkills] = await Promise.all([
          invoke<RuleFile[]>("list_rules"),
          invoke<SkillFile[]>("list_skills"),
        ]);
        if (!cancelled) {
          setRules(nextRules);
          setSkills(nextSkills);
          if (!selected && nextRules[0]) {
            setSelected({ kind: "rule", id: nextRules[0].source });
          }
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- load once on mount
  }, []);

  const activeRule =
    selected?.kind === "rule" ? rules.find((rule) => rule.source === selected.id) : undefined;
  const activeSkill =
    selected?.kind === "skill" ? skills.find((skill) => skill.name === selected.id) : undefined;

  return (
    <div className="grid gap-4 lg:grid-cols-[240px_1fr]">
      <aside className="space-y-4 text-sm">
        <section>
          <h2 className="mb-2 text-xs font-semibold uppercase tracking-wide text-zinc-500">
            Rules ({rules.length})
          </h2>
          <ul className="space-y-1">
            {rules.map((rule) => (
              <li key={rule.source}>
                <button
                  type="button"
                  className={`w-full rounded px-2 py-1.5 text-left ${
                    selected?.kind === "rule" && selected.id === rule.source
                      ? "bg-zinc-800 text-zinc-50"
                      : "hover:bg-zinc-900"
                  }`}
                  onClick={() => setSelected({ kind: "rule", id: rule.source })}
                >
                  <div className="truncate font-medium">
                    {rule.source.split("/").pop()}
                  </div>
                  {rule.deny_writes && (
                    <div className="text-[11px] text-amber-500">write: deny</div>
                  )}
                </button>
              </li>
            ))}
          </ul>
        </section>
        <section>
          <h2 className="mb-2 text-xs font-semibold uppercase tracking-wide text-zinc-500">
            Skills ({skills.length})
          </h2>
          <ul className="space-y-1">
            {skills.map((skill) => {
              const activation = skillActivation(skill);
              return (
                <li key={skill.name}>
                  <button
                    type="button"
                    className={`w-full rounded px-2 py-1.5 text-left ${
                      selected?.kind === "skill" && selected.id === skill.name
                        ? "bg-zinc-800 text-zinc-50"
                        : "hover:bg-zinc-900"
                    }`}
                    onClick={() => setSelected({ kind: "skill", id: skill.name })}
                  >
                    <div className="truncate font-medium">{skill.name}</div>
                    <div
                      className={`text-[11px] ${
                        activation === "always" ? "text-emerald-500" : "text-sky-500"
                      }`}
                    >
                      {activation}
                    </div>
                  </button>
                </li>
              );
            })}
          </ul>
          <p className="mt-3 text-[11px] leading-4 text-zinc-500">
            Full skill bodies load via match or <span className="font-mono">activate_skill</span>;
            catalog is always in T1.
          </p>
        </section>
      </aside>

      <section className="min-h-[420px] rounded border border-zinc-800 bg-zinc-950/60 p-4">
        {loading && <p className="text-sm text-zinc-500">Loading authority pack…</p>}
        {error && <p className="text-sm text-red-400">{error}</p>}
        {!loading && !error && activeRule && (
          <article className="space-y-3">
            <header>
              <h3 className="text-base font-semibold">{activeRule.source}</h3>
              <p className="text-sm text-zinc-400">{activeRule.description}</p>
              {activeRule.globs.length > 0 && (
                <p className="mt-1 font-mono text-[11px] text-zinc-500">
                  globs: {activeRule.globs.join(", ")}
                </p>
              )}
            </header>
            <pre className="overflow-auto whitespace-pre-wrap rounded bg-black/40 p-3 text-xs leading-relaxed text-zinc-200">
              {activeRule.content}
            </pre>
          </article>
        )}
        {!loading && !error && activeSkill && (
          <article className="space-y-3">
            <header>
              <div className="flex flex-wrap items-center gap-2">
                <h3 className="text-base font-semibold">{activeSkill.name}</h3>
                <span
                  className={`rounded px-1.5 py-0.5 text-[10px] uppercase tracking-wide ${
                    skillActivation(activeSkill) === "always"
                      ? "bg-emerald-500/15 text-emerald-400"
                      : "bg-sky-500/15 text-sky-400"
                  }`}
                >
                  {skillActivation(activeSkill)}
                </span>
              </div>
              <p className="text-sm text-zinc-400">{activeSkill.description}</p>
              <p className="mt-1 font-mono text-[11px] text-zinc-500">{activeSkill.source}</p>
              <p className="mt-2 text-[11px] leading-4 text-zinc-500">
                {skillActivation(activeSkill) === "always"
                  ? "Always-on: full body injected into T2 for every turn."
                  : "Catalog: name+description in T1; full body via keyword match or ade__activate_skill."}
              </p>
            </header>
            <pre className="overflow-auto whitespace-pre-wrap rounded bg-black/40 p-3 text-xs leading-relaxed text-zinc-200">
              {activeSkill.body}
            </pre>
          </article>
        )}
        {!loading && !error && !activeRule && !activeSkill && (
          <p className="text-sm text-zinc-500">
            No rules or skills selected. Agent turns load `.ade/rules/*.mdc` and a T1 skill
            catalog; full skill bodies arrive via always-on, match, or `activate_skill`.
          </p>
        )}
      </section>
    </div>
  );
}
