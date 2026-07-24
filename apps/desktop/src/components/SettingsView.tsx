import { useEffect, useState } from "react";
import { PROVIDER_PRESETS } from "../providers";
import { Disclosure, Hint } from "./ui";
import { GearIcon } from "./DarkSelect";

const AUTONOMY_KEY = "ade_autonomy_level";
const AGENT_PROVIDER_KEY = "ade_agent_provider";
const AGENT_BASE_URL_KEY = "ade_agent_base_url";
const AGENT_MODEL_KEY = "ade_agent_model";
const AGENT_EFFORT_KEY = "ade_agent_effort";
const SESSION_CAP_KEY = "ade_session_cap_usd";
const DAILY_CAP_KEY = "ade_daily_cap_usd";

type AutonomyLevel = "observe" | "propose" | "act" | "automate";
type EffortLevel = "low" | "medium" | "high";

type SettingsViewProps = {
  onOpenKeys: () => void;
};

function readAutonomy(): AutonomyLevel {
  const raw = window.localStorage.getItem(AUTONOMY_KEY);
  return raw === "observe" || raw === "act" || raw === "automate" || raw === "propose"
    ? raw
    : "propose";
}

function readEffort(): EffortLevel {
  const raw = window.localStorage.getItem(AGENT_EFFORT_KEY);
  return raw === "low" || raw === "high" || raw === "medium" ? raw : "low";
}

/**
 * One Settings surface (sidebar footer gear only).
 * Tier 0: write mode + spend caps.
 * Tier 1: composer defaults + Keys jump.
 */
export function SettingsView({ onOpenKeys }: SettingsViewProps) {
  const [autonomy, setAutonomy] = useState<AutonomyLevel>(readAutonomy);
  const [effort, setEffort] = useState<EffortLevel>(readEffort);
  const [sessionCap, setSessionCap] = useState(
    () => window.localStorage.getItem(SESSION_CAP_KEY) || "1",
  );
  const [dailyCap, setDailyCap] = useState(
    () => window.localStorage.getItem(DAILY_CAP_KEY) || "5",
  );
  const [providerId, setProviderId] = useState(
    () => window.localStorage.getItem(AGENT_PROVIDER_KEY) || "opencode",
  );
  const [note, setNote] = useState<string | null>(null);

  useEffect(() => {
    window.localStorage.setItem(AUTONOMY_KEY, autonomy);
  }, [autonomy]);
  useEffect(() => {
    window.localStorage.setItem(AGENT_EFFORT_KEY, effort);
  }, [effort]);
  useEffect(() => {
    window.localStorage.setItem(SESSION_CAP_KEY, sessionCap);
  }, [sessionCap]);
  useEffect(() => {
    window.localStorage.setItem(DAILY_CAP_KEY, dailyCap);
  }, [dailyCap]);

  const flash = (msg: string) => {
    setNote(msg);
    window.setTimeout(() => setNote(null), 1800);
  };

  const applyPreset = (id: string) => {
    const preset = PROVIDER_PRESETS.find((item) => item.id === id);
    if (!preset) return;
    window.localStorage.setItem(AGENT_PROVIDER_KEY, preset.id);
    window.localStorage.setItem(AGENT_BASE_URL_KEY, preset.baseUrl);
    window.localStorage.setItem(AGENT_MODEL_KEY, preset.models[0] || "");
    setProviderId(preset.id);
    flash(`Provider → ${preset.label}`);
  };

  const providerLabel =
    PROVIDER_PRESETS.find((p) => p.id === providerId)?.label ?? providerId;

  return (
    <div className="mx-auto max-w-md space-y-5 px-1">
      <div className="flex items-start gap-2.5">
        <div className="mt-0.5 grid size-8 place-items-center rounded-lg border border-white/10 bg-white/3 text-slate-300">
          <GearIcon className="size-4" />
        </div>
        <div className="min-w-0">
          <h2 className="text-sm font-semibold text-slate-100">Settings</h2>
          <p className="mt-0.5 text-[11px] leading-5 text-slate-500">
            Defaults for Home. API keys live under Keys.
          </p>
          {note && <p className="mt-1.5 text-[11px] text-emerald-200/90">{note}</p>}
        </div>
      </div>

      <section className="space-y-3">
        <div className="flex items-center gap-2">
          <h3 className="text-[11px] font-semibold uppercase tracking-wider text-slate-500">
            When ADE writes
          </h3>
          <Hint text="Suggest drafts only. Apply may write inside approved owned paths." />
        </div>
        <div className="grid grid-cols-2 gap-1.5">
          <button
            type="button"
            onClick={() => setAutonomy("propose")}
            className={`rounded-lg border px-3 py-2.5 text-left transition ${
              autonomy === "propose" || autonomy === "observe"
                ? "border-blue-400/40 bg-blue-500/15"
                : "border-white/8 bg-white/2 hover:border-white/15"
            }`}
          >
            <div className="text-[12px] font-semibold text-slate-100">Suggest</div>
            <div className="mt-0.5 text-[10px] text-slate-500">Plan & diffs only</div>
          </button>
          <button
            type="button"
            onClick={() => setAutonomy("act")}
            className={`rounded-lg border px-3 py-2.5 text-left transition ${
              autonomy === "act" || autonomy === "automate"
                ? "border-blue-400/40 bg-blue-500/15"
                : "border-white/8 bg-white/2 hover:border-white/15"
            }`}
          >
            <div className="text-[12px] font-semibold text-slate-100">Apply</div>
            <div className="mt-0.5 text-[10px] text-slate-500">Write when approved</div>
          </button>
        </div>
        <Disclosure
          title="Debug modes"
          summary={
            autonomy === "observe" || autonomy === "automate"
              ? autonomy
              : "Observe · Automate"
          }
          defaultOpen={autonomy === "observe" || autonomy === "automate"}
          storageKey="ade_settings_more_autonomy"
        >
          <div className="flex flex-wrap gap-1.5">
            <button
              type="button"
              onClick={() => setAutonomy("observe")}
              className={`rounded-md border px-2.5 py-1 text-[11px] ${
                autonomy === "observe"
                  ? "border-blue-400/40 bg-blue-500/15 text-blue-100"
                  : "border-white/10 text-slate-500"
              }`}
            >
              Observe
            </button>
            <button
              type="button"
              onClick={() => setAutonomy("automate")}
              className={`rounded-md border px-2.5 py-1 text-[11px] ${
                autonomy === "automate"
                  ? "border-blue-400/40 bg-blue-500/15 text-blue-100"
                  : "border-white/10 text-slate-500"
              }`}
            >
              Automate
            </button>
          </div>
        </Disclosure>
      </section>

      <section className="space-y-3 border-t border-white/6 pt-5">
        <div className="flex items-center gap-2">
          <h3 className="text-[11px] font-semibold uppercase tracking-wider text-slate-500">
            Spend limits
          </h3>
          <Hint text="Hard stops. Agent turns halt when a cap is hit." />
        </div>
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="text-[11px] text-slate-400">Per session</span>
            <div className="relative mt-1">
              <span className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[12px] text-slate-500">
                $
              </span>
              <input
                inputMode="decimal"
                value={sessionCap}
                onChange={(e) => setSessionCap(e.target.value)}
                className="w-full rounded-lg border border-white/10 bg-[#101620] py-2 pl-6 pr-3 text-xs text-slate-200 outline-hidden focus:border-blue-400/40"
              />
            </div>
          </label>
          <label className="block">
            <span className="text-[11px] text-slate-400">Per day</span>
            <div className="relative mt-1">
              <span className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[12px] text-slate-500">
                $
              </span>
              <input
                inputMode="decimal"
                value={dailyCap}
                onChange={(e) => setDailyCap(e.target.value)}
                className="w-full rounded-lg border border-white/10 bg-[#101620] py-2 pl-6 pr-3 text-xs text-slate-200 outline-hidden focus:border-blue-400/40"
              />
            </div>
          </label>
        </div>
      </section>

      <section className="space-y-2 border-t border-white/6 pt-5">
        <Disclosure
          title="Composer defaults"
          summary={`${providerLabel} · ${effort}`}
          defaultOpen={false}
          storageKey="ade_settings_composer_defaults"
        >
          <div className="mb-3">
            <div className="mb-1.5 text-[10px] uppercase tracking-wider text-slate-600">
              Effort (turn gas tank)
            </div>
            <div className="mb-1.5 text-[10px] text-slate-500">
              How many tool steps this turn can take. Low = quick look · Med = normal work ·
              High = long runs
            </div>
            <div className="flex flex-wrap gap-1.5">
              {(["low", "medium", "high"] as EffortLevel[]).map((level) => (
                <button
                  key={level}
                  type="button"
                  onClick={() => setEffort(level)}
                  className={`rounded-md border px-2.5 py-1 text-[11px] capitalize ${
                    effort === level
                      ? "border-blue-400/40 bg-blue-500/15 text-blue-100"
                      : "border-white/10 text-slate-500"
                  }`}
                >
                  {level === "medium" ? "Med" : level}
                </button>
              ))}
            </div>
          </div>
          <div>
            <div className="mb-1.5 text-[10px] uppercase tracking-wider text-slate-600">
              Provider preset
            </div>
            <div className="flex flex-wrap gap-1.5">
              {PROVIDER_PRESETS.map((preset) => (
                <button
                  key={preset.id}
                  type="button"
                  onClick={() => applyPreset(preset.id)}
                  className={`rounded-md border px-2.5 py-1 text-[11px] ${
                    providerId === preset.id
                      ? "border-blue-400/40 bg-blue-500/15 text-blue-100"
                      : "border-white/10 text-slate-500"
                  }`}
                >
                  {preset.label}
                </button>
              ))}
            </div>
          </div>
        </Disclosure>

        <button
          type="button"
          onClick={onOpenKeys}
          className="flex w-full items-center justify-between rounded-lg border border-white/8 bg-white/2 px-3 py-2.5 text-left transition hover:border-white/15 hover:bg-white/4"
        >
          <div>
            <div className="text-[12px] font-medium text-slate-200">API keys</div>
            <div className="text-[10px] text-slate-500">OS vault — not stored here</div>
          </div>
          <span className="text-[11px] font-semibold text-blue-300">Open →</span>
        </button>
      </section>
    </div>
  );
}

export { SESSION_CAP_KEY, DAILY_CAP_KEY };
