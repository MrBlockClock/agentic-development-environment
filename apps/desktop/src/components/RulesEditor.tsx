import { useEffect, useMemo, useState } from "react";
import { invoke } from "../ipc";
import { Disclosure, Hint } from "./ui";

type RuleFile = {
  source: string;
  description: string;
  globs: string[];
  deny_writes: boolean;
  content: string;
  scope?: string;
  pack?: string | null;
};

type SkillFile = {
  name: string;
  description: string;
  always_apply: boolean;
  body: string;
  source: string;
  scope?: string;
  pack?: string | null;
};

type GuidanceProfile = {
  id: string;
  packs: string[];
};

type SkillActivation = "always" | "catalog";

function skillActivation(skill: SkillFile): SkillActivation {
  return skill.always_apply ? "always" : "catalog";
}

function scopeOf(item: { scope?: string }): "global" | "workspace" {
  return item.scope === "global" ? "global" : "workspace";
}

/** Browse Global + workspace rules/skills loaded by the agent runtime. */
export function RulesEditor() {
  const [rules, setRules] = useState<RuleFile[]>([]);
  const [skills, setSkills] = useState<SkillFile[]>([]);
  const [profiles, setProfiles] = useState<GuidanceProfile[]>([]);
  const [activeProfile, setActiveProfile] = useState<string | null>(null);
  const [selected, setSelected] = useState<{ kind: "rule" | "skill"; id: string } | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const reload = async () => {
    setLoading(true);
    setError(null);
    try {
      const [nextRules, nextSkills, nextProfiles, nextActive] = await Promise.all([
        invoke<RuleFile[]>("list_rules"),
        invoke<SkillFile[]>("list_skills"),
        invoke<GuidanceProfile[]>("list_guidance_profiles").catch(() => []),
        invoke<string | null>("get_active_guidance_profile").catch(() => null),
      ]);
      setRules(nextRules);
      setSkills(nextSkills);
      setProfiles(nextProfiles);
      setActiveProfile(nextActive);
      if (!selected && nextRules[0]) {
        setSelected({ kind: "rule", id: nextRules[0].source });
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- load once on mount
  }, []);

  const globalRules = useMemo(
    () => rules.filter((r) => scopeOf(r) === "global"),
    [rules],
  );
  const workspaceRules = useMemo(
    () => rules.filter((r) => scopeOf(r) === "workspace"),
    [rules],
  );
  const globalSkills = useMemo(
    () => skills.filter((s) => scopeOf(s) === "global"),
    [skills],
  );
  const workspaceSkills = useMemo(
    () => skills.filter((s) => scopeOf(s) === "workspace"),
    [skills],
  );

  const activeRule =
    selected?.kind === "rule" ? rules.find((rule) => rule.source === selected.id) : undefined;
  const activeSkill =
    selected?.kind === "skill" ? skills.find((skill) => skill.name === selected.id) : undefined;

  const setProfile = async (id: string | null) => {
    try {
      const next = await invoke<string | null>("set_active_guidance_profile", { id });
      setActiveProfile(next);
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const renderRuleList = (items: RuleFile[]) => (
    <ul className="space-y-1">
      {items.map((rule) => (
        <li key={rule.source}>
          <button
            type="button"
            className={`w-full rounded-lg px-2.5 py-2 text-left transition ${
              selected?.kind === "rule" && selected.id === rule.source
                ? "bg-blue-500/15 text-blue-100"
                : "text-slate-400 hover:bg-white/4 hover:text-slate-200"
            }`}
            onClick={() => setSelected({ kind: "rule", id: rule.source })}
          >
            <div className="truncate text-[12px] font-medium">
              {rule.source.split(/[\\/]/).pop()}
            </div>
            <div className="flex flex-wrap gap-1 text-[10px]">
              <span className="text-slate-500">{scopeOf(rule)}</span>
              {rule.deny_writes && <span className="text-amber-400">deny</span>}
              {rule.pack && <span className="text-violet-300">pack:{rule.pack}</span>}
            </div>
          </button>
        </li>
      ))}
      {items.length === 0 && (
        <li className="px-2.5 py-1 text-[11px] text-slate-600">None</li>
      )}
    </ul>
  );

  const renderSkillList = (items: SkillFile[]) => (
    <ul className="space-y-1">
      {items.map((skill) => {
        const activation = skillActivation(skill);
        return (
          <li key={`${skill.scope}-${skill.name}`}>
            <button
              type="button"
              className={`w-full rounded-lg px-2.5 py-2 text-left transition ${
                selected?.kind === "skill" && selected.id === skill.name
                  ? "bg-blue-500/15 text-blue-100"
                  : "text-slate-400 hover:bg-white/4 hover:text-slate-200"
              }`}
              onClick={() => setSelected({ kind: "skill", id: skill.name })}
            >
              <div className="truncate text-[12px] font-medium">{skill.name}</div>
              <div
                className={`text-[10px] ${
                  activation === "always" ? "text-emerald-400" : "text-sky-400"
                }`}
              >
                {scopeOf(skill)} · {activation}
              </div>
            </button>
          </li>
        );
      })}
      {items.length === 0 && (
        <li className="px-2.5 py-1 text-[11px] text-slate-600">None</li>
      )}
    </ul>
  );

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2 text-[11px] text-slate-400">
        <span className="font-medium text-slate-300">Profile</span>
        <button
          type="button"
          onClick={() => void setProfile(null)}
          className={`rounded-md border px-2 py-1 ${
            !activeProfile
              ? "border-blue-400/40 bg-blue-500/15 text-blue-100"
              : "border-white/10 text-slate-500 hover:text-slate-300"
          }`}
        >
          All packs
        </button>
        {profiles.map((profile) => (
          <button
            key={profile.id}
            type="button"
            onClick={() => void setProfile(profile.id)}
            title={profile.packs.join(", ") || "empty packs"}
            className={`rounded-md border px-2 py-1 ${
              activeProfile === profile.id
                ? "border-blue-400/40 bg-blue-500/15 text-blue-100"
                : "border-white/10 text-slate-500 hover:text-slate-300"
            }`}
          >
            {profile.id}
          </button>
        ))}
        <Hint text="Profiles filter pack-tagged guidance. Untagged items and deny rules still load." />
      </div>

      <div className="grid gap-4 lg:grid-cols-[260px_1fr]">
        <aside className="space-y-3 text-sm">
          <Disclosure
            title={`Global rules (${globalRules.length})`}
            subtitle="Machine ADE home"
            hint="Shared across workspaces. Deny writes union with workspace."
            defaultOpen={globalRules.length > 0}
            storageKey="ade_rules_global_open"
          >
            {renderRuleList(globalRules)}
          </Disclosure>
          <Disclosure
            title={`Workspace rules (${workspaceRules.length})`}
            subtitle="This checkout · .ade/rules"
            hint="Wins on same stem for prompt body; cannot clear a global deny."
            defaultOpen
            storageKey="ade_rules_workspace_open"
          >
            {renderRuleList(workspaceRules)}
          </Disclosure>
          <Disclosure
            title={`Global skills (${globalSkills.length})`}
            subtitle="Machine ADE home"
            defaultOpen={globalSkills.length > 0}
            storageKey="ade_skills_global_open"
          >
            {renderSkillList(globalSkills)}
          </Disclosure>
          <Disclosure
            title={`Workspace skills (${workspaceSkills.length})`}
            subtitle="This checkout · .ade/skills"
            defaultOpen
            storageKey="ade_skills_workspace_open"
          >
            {renderSkillList(workspaceSkills)}
          </Disclosure>
        </aside>

        <section className="min-h-[320px] rounded-2xl border border-white/7 bg-[#0d121a]/85 p-5">
          {loading && <p className="text-sm text-slate-500">Loading guidance…</p>}
          {error && <p className="text-sm text-red-400">{error}</p>}
          {!loading && !error && activeRule && (
            <article className="space-y-3">
              <header>
                <div className="flex flex-wrap items-center gap-2">
                  <h3 className="text-base font-semibold text-slate-100">
                    {activeRule.source.split(/[\\/]/).pop()}
                  </h3>
                  <span className="rounded bg-white/5 px-1.5 py-0.5 text-[10px] uppercase text-slate-400">
                    {scopeOf(activeRule)}
                  </span>
                  <Hint text="Rule bodies are in authority context; globs mainly gate write deny." />
                </div>
                <p className="mt-1 text-sm text-slate-400">{activeRule.description}</p>
                {activeRule.globs.length > 0 && (
                  <p className="mt-1 font-mono text-[11px] text-slate-500">
                    globs: {activeRule.globs.join(", ")}
                  </p>
                )}
              </header>
              <Disclosure
                title="Rule body"
                defaultOpen={false}
                storageKey={`ade_rule_body_${activeRule.source}`}
              >
                <pre className="thin-scrollbar max-h-[60vh] overflow-auto whitespace-pre-wrap rounded-lg bg-black/40 p-3 text-xs leading-relaxed text-slate-200">
                  {activeRule.content}
                </pre>
              </Disclosure>
            </article>
          )}
          {!loading && !error && activeSkill && (
            <article className="space-y-3">
              <header>
                <div className="flex flex-wrap items-center gap-2">
                  <h3 className="text-base font-semibold text-slate-100">{activeSkill.name}</h3>
                  <span className="rounded bg-white/5 px-1.5 py-0.5 text-[10px] uppercase text-slate-400">
                    {scopeOf(activeSkill)}
                  </span>
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
                <p className="mt-1 text-sm text-slate-400">{activeSkill.description}</p>
                <p className="mt-1 font-mono text-[11px] text-slate-500">{activeSkill.source}</p>
              </header>
              <Disclosure
                title="Skill body"
                defaultOpen={false}
                storageKey={`ade_skill_body_${activeSkill.name}`}
              >
                <pre className="thin-scrollbar max-h-[60vh] overflow-auto whitespace-pre-wrap rounded-lg bg-black/40 p-3 text-xs leading-relaxed text-slate-200">
                  {activeSkill.body}
                </pre>
              </Disclosure>
            </article>
          )}
          {!loading && !error && !activeRule && !activeSkill && (
            <p className="text-sm text-slate-500">
              Select a rule or skill. ADE merges Global guidance with this workspace’s `.ade/` pack.
            </p>
          )}
        </section>
      </div>
    </div>
  );
}
