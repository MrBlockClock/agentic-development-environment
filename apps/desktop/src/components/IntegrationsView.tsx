import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke, isTauri } from "../ipc";
import {
  HOST_TOOLS,
  INTEGRATIONS,
  integrationsByCategory,
  mcpCommandForPlatform,
  type IntegrationDef,
} from "../integrationsCatalog";
import { DesktopRequired } from "./DesktopRequired";
import { Disclosure, EmptyState, Hint, Panel } from "./ui";

type ProviderKeyStatus = {
  profile: string;
  provider: string;
  configured: boolean;
};

type IntegrationsViewProps = {
  mcpServers: string[];
  mcpToolCount: number;
  onOpenKeys: () => void;
  onOpenMcp: () => void;
  onOpenView: (view: "Browser" | "Terminal" | "Workspaces" | "Keys" | "MCP") => void;
  onConnectMcp: (input: {
    name: string;
    command: string;
    args: string[];
    approved: boolean;
  }) => void;
  onRefreshMcp: () => void;
};

const PROFILE = "local";

function statusTone(
  configured: boolean,
  mcpConnected: boolean,
): "ready" | "todo" | "info" {
  if (configured || mcpConnected) return "ready";
  return "todo";
}

/**
 * Setup → Integrations: standing connectors + host/MCP tools at a glance.
 * Credentials use the same OS vault as Keys (never shown raw).
 */
export function IntegrationsView({
  mcpServers,
  mcpToolCount,
  onOpenKeys,
  onOpenMcp,
  onOpenView,
  onConnectMcp,
  onRefreshMcp,
}: IntegrationsViewProps) {
  const [vault, setVault] = useState<Record<string, boolean>>({});
  const [expandedId, setExpandedId] = useState<string | null>("github");
  const [secret, setSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const tokenIds = useMemo(
    () =>
      INTEGRATIONS.filter((item) => item.kind === "token" && item.vaultId).map(
        (item) => item.vaultId!,
      ),
    [],
  );

  const refreshVault = useCallback(async () => {
    if (!isTauri()) return;
    setBusy(true);
    setMessage(null);
    try {
      const entries = await Promise.all(
        tokenIds.map(async (id) => {
          try {
            const status = await invoke<ProviderKeyStatus>("key_status", {
              provider: id,
              profile: PROFILE,
            });
            return [id, status.configured] as const;
          } catch {
            return [id, false] as const;
          }
        }),
      );
      setVault(Object.fromEntries(entries));
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }, [tokenIds]);

  useEffect(() => {
    void refreshVault();
  }, [refreshVault]);

  const connectedTokens = tokenIds.filter((id) => vault[id]).length;
  const mcpConnected = mcpServers.length;

  const saveToken = async (vaultId: string) => {
    if (!secret.trim()) {
      setMessage("Paste a token first.");
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      await invoke<ProviderKeyStatus>("key_set", {
        provider: vaultId,
        profile: PROFILE,
        secret: secret.trim(),
      });
      setSecret("");
      setMessage(`Saved ${vaultId} to the OS vault.`);
      await refreshVault();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const clearToken = async (vaultId: string) => {
    setBusy(true);
    setMessage(null);
    try {
      await invoke("key_delete", { provider: vaultId, profile: PROFILE });
      setMessage(`Removed ${vaultId} from the vault.`);
      await refreshVault();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const connectRecipe = (item: IntegrationDef) => {
    const recipe = item.mcpRecipe;
    if (!recipe) return;
    onConnectMcp({
      name: recipe.name,
      command: mcpCommandForPlatform(recipe),
      args: recipe.args,
      approved: true,
    });
    setMessage(
      recipe.envHint
        ? `Connecting ${recipe.name}… Ensure ${recipe.envHint} is set in the Desktop process env (token in vault alone may not inject yet).`
        : `Connecting ${recipe.name}…`,
    );
    onRefreshMcp();
  };

  if (!isTauri()) {
    return <DesktopRequired view="Integrations" />;
  }

  const groups = integrationsByCategory();

  return (
    <div className="mx-auto flex min-h-0 w-full max-w-3xl flex-col gap-4">
      <header className="space-y-1">
        <h1 className="text-lg font-semibold tracking-tight text-slate-50">
          Integrations
        </h1>
        <p className="text-[12px] leading-5 text-slate-400">
          Standing connectors for GitHub, cloud, payments, and MCP — plus the
          host tools every Desktop turn can use. Model API keys stay under Keys.
        </p>
      </header>

      <div className="flex flex-wrap items-center gap-2 rounded-xl border border-white/8 bg-white/[0.03] px-3 py-2.5">
        <span
          className={`rounded-md border px-2 py-1 text-[11px] font-medium ${
            connectedTokens > 0
              ? "border-emerald-400/30 bg-emerald-500/10 text-emerald-100"
              : "border-amber-400/25 bg-amber-500/10 text-amber-100"
          }`}
        >
          {connectedTokens} token{connectedTokens === 1 ? "" : "s"}
        </span>
        <span
          className={`rounded-md border px-2 py-1 text-[11px] font-medium ${
            mcpConnected > 0
              ? "border-violet-400/30 bg-violet-500/10 text-violet-100"
              : "border-white/10 bg-white/4 text-slate-400"
          }`}
        >
          {mcpConnected} MCP · {mcpToolCount} tools
        </span>
        <div className="min-w-0 flex-1" />
        <button
          type="button"
          onClick={() => onOpenKeys()}
          className="rounded-md border border-white/10 bg-white/4 px-2.5 py-1 text-[11px] font-semibold text-slate-200 hover:bg-white/8"
        >
          Keys
        </button>
        <button
          type="button"
          onClick={() => onOpenMcp()}
          className="rounded-md border border-blue-400/30 bg-blue-500/15 px-2.5 py-1 text-[11px] font-semibold text-blue-100 hover:bg-blue-500/25"
        >
          MCP console
        </button>
      </div>

      {message && (
        <p className="rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-[11px] text-slate-300">
          {message}
        </p>
      )}

      <Panel
        title="Tools this turn"
        subtitle="Built into the Desktop harness — always available when Apply allows them"
      >
        <ul className="flex flex-wrap gap-2">
          {HOST_TOOLS.map((tool) => (
            <li key={tool.id}>
              <span
                title={tool.note}
                className="inline-flex items-center rounded-md border border-white/10 bg-white/4 px-2 py-1 text-[11px] text-slate-300"
              >
                {tool.label}
              </span>
            </li>
          ))}
          {mcpServers.map((name) => (
            <li key={`mcp-${name}`}>
              <span
                title="Connected MCP server"
                className="inline-flex items-center rounded-md border border-violet-400/25 bg-violet-500/10 px-2 py-1 text-[11px] text-violet-100"
              >
                MCP · {name}
              </span>
            </li>
          ))}
        </ul>
        {mcpServers.length === 0 && (
          <p className="mt-2 text-[11px] text-slate-500">
            No MCP servers connected yet — expand GitHub / Stripe below for
            one-click recipes, or open the MCP console.
          </p>
        )}
      </Panel>

      {groups.map((group) => (
        <section key={group.category} className="space-y-2">
          <h2 className="px-0.5 text-[10px] font-semibold uppercase tracking-wider text-slate-500">
            {group.label}
          </h2>
          <ul className="space-y-2">
            {group.items.map((item) => {
              const configured = item.vaultId ? Boolean(vault[item.vaultId]) : false;
              const mcpLive = item.mcpRecipe
                ? mcpServers.includes(item.mcpRecipe.name)
                : false;
              const open = expandedId === item.id;
              const tone = statusTone(
                configured || item.kind === "builtin" || item.kind === "keys",
                mcpLive,
              );
              return (
                <li
                  key={item.id}
                  className="overflow-hidden rounded-xl border border-white/8 bg-[#0f141c]"
                >
                  <button
                    type="button"
                    onClick={() =>
                      setExpandedId((prev) => (prev === item.id ? null : item.id))
                    }
                    className="flex w-full items-center gap-3 px-3 py-2.5 text-left hover:bg-white/[0.03]"
                  >
                    <span
                      className={`size-1.5 shrink-0 rounded-full ${
                        tone === "ready"
                          ? "bg-emerald-400"
                          : tone === "info"
                            ? "bg-sky-400"
                            : "bg-amber-400/80"
                      }`}
                      aria-hidden
                    />
                    <span className="min-w-0 flex-1">
                      <span className="block text-[13px] font-semibold text-slate-100">
                        {item.label}
                      </span>
                      <span className="block truncate text-[11px] text-slate-500">
                        {item.blurb}
                      </span>
                    </span>
                    <span className="shrink-0 text-[10px] font-medium uppercase tracking-wide text-slate-500">
                      {mcpLive
                        ? "MCP on"
                        : configured
                          ? "Token"
                          : item.kind === "builtin" || item.kind === "keys"
                            ? "Built-in"
                            : "Add"}
                    </span>
                    <span className="text-[10px] text-slate-600" aria-hidden>
                      {open ? "▴" : "▾"}
                    </span>
                  </button>
                  {open && (
                    <div className="space-y-3 border-t border-white/6 px-3 py-3">
                      <p className="text-[11px] leading-5 text-slate-400">
                        {item.blurb}
                      </p>

                      {(item.kind === "keys" ||
                        item.kind === "surface" ||
                        item.kind === "builtin") &&
                        item.openView && (
                          <button
                            type="button"
                            onClick={() => onOpenView(item.openView!)}
                            className="rounded-md border border-white/12 bg-white/5 px-2.5 py-1.5 text-[11px] font-semibold text-slate-100 hover:bg-white/8"
                          >
                            Open {item.label}
                          </button>
                        )}

                      {item.kind === "token" && item.vaultId && (
                        <div className="space-y-2">
                          <div className="flex flex-wrap items-center gap-2 text-[11px]">
                            <span
                              className={
                                configured
                                  ? "font-medium text-emerald-300"
                                  : "text-amber-200/90"
                              }
                            >
                              {configured ? "•••••••• in vault" : "No token yet"}
                            </span>
                            {item.getKeyUrl && (
                              <a
                                href={item.getKeyUrl}
                                target="_blank"
                                rel="noreferrer"
                                className="text-blue-300/90 hover:underline"
                              >
                                Get token
                              </a>
                            )}
                            <Hint text="Stored in the same OS vault as Keys. ADE never shows the raw secret after save." />
                          </div>
                          <div className="flex flex-wrap gap-2">
                            <input
                              type="password"
                              autoComplete="off"
                              spellCheck={false}
                              placeholder={
                                configured ? "Replace token…" : "Paste token…"
                              }
                              value={secret}
                              onChange={(event) => setSecret(event.target.value)}
                              className="min-w-0 flex-1 rounded-lg border border-white/10 bg-[#101620] px-3 py-2 font-mono text-[12px] text-slate-200"
                            />
                            <button
                              type="button"
                              disabled={busy}
                              onClick={() => void saveToken(item.vaultId!)}
                              className="rounded-lg border border-blue-400/35 bg-blue-500/20 px-3 py-2 text-[11px] font-semibold text-blue-50 hover:bg-blue-500/30 disabled:opacity-40"
                            >
                              Save
                            </button>
                            {configured && (
                              <button
                                type="button"
                                disabled={busy}
                                onClick={() => void clearToken(item.vaultId!)}
                                className="rounded-lg border border-white/10 px-3 py-2 text-[11px] font-semibold text-slate-400 hover:bg-white/5 disabled:opacity-40"
                              >
                                Remove
                              </button>
                            )}
                          </div>
                          {item.envVars && item.envVars.length > 0 && (
                            <p className="text-[10px] text-slate-500">
                              Env: {item.envVars.join(" · ")}
                            </p>
                          )}
                        </div>
                      )}

                      {item.mcpRecipe && (
                        <Disclosure
                          title="MCP recipe"
                          summary={mcpLive ? "Connected" : "One-click connect"}
                          hint="Spawns a reviewed stdio server. Export the env hint in the Desktop process if the recipe needs it."
                          defaultOpen={!mcpLive}
                        >
                          <div className="space-y-2 pt-1">
                            <p className="font-mono text-[10px] leading-4 text-slate-500">
                              {mcpCommandForPlatform(item.mcpRecipe)}{" "}
                              {item.mcpRecipe.args.join(" ")}
                            </p>
                            {item.mcpRecipe.envHint && (
                              <p className="text-[11px] text-amber-100/85">
                                Needs env: {item.mcpRecipe.envHint}
                              </p>
                            )}
                            <div className="flex flex-wrap gap-2">
                              <button
                                type="button"
                                disabled={busy || mcpLive}
                                onClick={() => connectRecipe(item)}
                                className="rounded-md border border-violet-400/35 bg-violet-500/15 px-2.5 py-1.5 text-[11px] font-semibold text-violet-100 hover:bg-violet-500/25 disabled:opacity-40"
                              >
                                {mcpLive ? "Already connected" : "Connect MCP"}
                              </button>
                              <button
                                type="button"
                                onClick={() => onOpenMcp()}
                                className="rounded-md border border-white/10 px-2.5 py-1.5 text-[11px] text-slate-300 hover:bg-white/5"
                              >
                                Open console
                              </button>
                              {item.mcpRecipe.docsUrl && (
                                <a
                                  href={item.mcpRecipe.docsUrl}
                                  target="_blank"
                                  rel="noreferrer"
                                  className="rounded-md border border-white/10 px-2.5 py-1.5 text-[11px] text-blue-300/90 hover:bg-white/5"
                                >
                                  Docs
                                </a>
                              )}
                            </div>
                          </div>
                        </Disclosure>
                      )}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      ))}

      {INTEGRATIONS.length === 0 && (
        <EmptyState
          title="No integrations"
          body="Catalog is empty — check integrationsCatalog.ts."
        />
      )}
    </div>
  );
}
