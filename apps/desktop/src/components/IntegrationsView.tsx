import { useCallback, useEffect, useMemo, useState } from "react";
import {
  HOST_TOOLS,
  INTEGRATIONS,
  INTEGRATION_CATEGORY_LABEL,
  featuredMcpIntegrations,
  formatMcpRecipeCommand,
  integrationsByCategory,
  mcpCommandForPlatform,
  type IntegrationCategory,
  type IntegrationDef,
} from "../integrationsCatalog";
import { invoke, isTauri } from "../ipc";
import { DesktopRequired } from "./DesktopRequired";
import { BrandWell } from "./IntegrationIcons";
import { Disclosure, EmptyState, Hint, SubTabs } from "./ui";

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
  onOpenView: (
    view: "Browser" | "Terminal" | "Workspaces" | "Keys" | "MCP",
  ) => void;
  onConnectMcp: (input: {
    name: string;
    command: string;
    args: string[];
    approved: boolean;
    recipeId?: string | null;
    vaultProvider?: string | null;
    vaultEnvKeys?: string[];
  }) => Promise<void>;
  onRefreshMcp: () => void;
};

const PROFILE = "local";

type StoreTab = "connectors" | "host";

type ConnTone = "ready" | "warn" | "todo" | "info";

/** Ready = MCP live with full vault-injected env. Warn = token only / incomplete env. */
function statusTone(
  item: IntegrationDef,
  configured: boolean,
  mcpLive: boolean,
): ConnTone {
  if (
    item.kind === "builtin" ||
    item.kind === "keys" ||
    item.kind === "surface"
  ) {
    return "info";
  }
  if (item.mcpRecipe && mcpLive) {
    if (item.mcpRecipe.externalEnvKeys?.length) return "warn";
    return "ready";
  }
  if (item.kind === "token" && configured) return "warn";
  return "todo";
}

function statusCaption(
  item: IntegrationDef,
  configured: boolean,
  mcpLive: boolean,
  tone: ConnTone,
): string {
  if (item.kind === "builtin" || item.kind === "keys" || item.kind === "surface") {
    return "Built-in";
  }
  if (mcpLive && tone === "ready") return "MCP connected";
  if (mcpLive && tone === "warn") return "MCP spawned · incomplete env";
  if (configured && item.mcpRecipe) return "Token in vault · MCP not connected";
  if (configured) return "Token in vault · not wired to agent";
  return "Not set up";
}

function matchesQuery(text: string, query: string): boolean {
  if (!query) return true;
  return text.toLowerCase().includes(query);
}

/**
 * Setup → Integrations: ChatGPT-style store layout (search, Connected strip,
 * category grids) over the standing connectors + host tools catalog.
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
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [secrets, setSecrets] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [connectingMcp, setConnectingMcp] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [tab, setTab] = useState<StoreTab>("connectors");
  const [collapsedCats, setCollapsedCats] = useState<
    Partial<Record<IntegrationCategory, boolean>>
  >({});
  /** Per-recipe explicit approval (same gate as MCP console). */
  const [mcpApproved, setMcpApproved] = useState<Record<string, boolean>>({});

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

  const q = query.trim().toLowerCase();

  const itemMeta = useCallback(
    (item: IntegrationDef) => {
      const configured = item.vaultId ? Boolean(vault[item.vaultId]) : false;
      const mcpLive = item.mcpRecipe
        ? mcpServers.includes(item.mcpRecipe.name)
        : false;
      const tone = statusTone(item, configured, mcpLive);
      return {
        configured,
        mcpLive,
        tone,
        caption: statusCaption(item, configured, mcpLive, tone),
        ready: tone === "ready",
      };
    },
    [vault, mcpServers],
  );

  /** Proven MCP connections only — never day-zero built-ins. */
  const connected = useMemo(() => {
    const rows: {
      id: string;
      label: string;
      brandId: string;
      tone: ConnTone;
    }[] = [];
    for (const item of INTEGRATIONS) {
      const meta = itemMeta(item);
      if (!meta.mcpLive || !item.mcpRecipe) continue;
      rows.push({
        id: item.id,
        label: item.label,
        brandId: item.id,
        tone: meta.tone,
      });
    }
    for (const name of mcpServers) {
      if (
        rows.some(
          (row) =>
            row.id === name ||
            INTEGRATIONS.some(
              (item) => item.mcpRecipe?.name === name && row.id === item.id,
            ),
        )
      ) {
        continue;
      }
      const catalog = INTEGRATIONS.find((item) => item.mcpRecipe?.name === name);
      if (catalog) continue;
      rows.push({
        id: `mcp:${name}`,
        label: name,
        brandId: "mcp-host",
        tone: "ready",
      });
    }
    return rows;
  }, [itemMeta, mcpServers]);

  const recipeQuickAdd = useMemo(() => {
    return featuredMcpIntegrations().filter((item) => {
      const live = item.mcpRecipe
        ? mcpServers.includes(item.mcpRecipe.name)
        : false;
      return !live;
    });
  }, [mcpServers]);

  const connectorGroups = useMemo(() => {
    return integrationsByCategory()
      .map((group) => ({
        ...group,
        items: group.items.filter(
          (item) =>
            item.category !== "host" &&
            matchesQuery(`${item.label} ${item.blurb}`, q),
        ),
      }))
      .filter((group) => group.items.length > 0);
  }, [q]);

  const hostItems = useMemo(() => {
    const fromCatalog = INTEGRATIONS.filter(
      (item) =>
        item.category === "host" &&
        matchesQuery(`${item.label} ${item.blurb}`, q),
    );
    const tools = HOST_TOOLS.filter((tool) =>
      matchesQuery(`${tool.label} ${tool.note}`, q),
    );
    return { fromCatalog, tools };
  }, [q]);

  const hostBadge = String(
    hostItems.fromCatalog.length + hostItems.tools.length + mcpServers.length,
  );

  const saveToken = async (vaultId: string) => {
    const draft = (secrets[vaultId] ?? "").trim();
    if (!draft) {
      setMessage("Paste a token first.");
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      await invoke<ProviderKeyStatus>("key_set", {
        provider: vaultId,
        profile: PROFILE,
        secret: draft,
      });
      setSecrets((prev) => ({ ...prev, [vaultId]: "" }));
      setMessage(
        `Saved ${vaultId} to the OS vault. Connect MCP (if available) before the agent can use it.`,
      );
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

  const connectRecipe = async (
    item: IntegrationDef,
    opts?: { approvedOverride?: boolean },
  ) => {
    const recipe = item.mcpRecipe;
    if (!recipe) return;

    const approved = opts?.approvedOverride ?? mcpApproved[recipe.name];
    if (!approved) {
      setExpandedId(item.id);
      setMessage(
        `Approve the ${recipe.name} MCP command below before connecting.`,
      );
      return;
    }

    if (recipe.envKeys?.length && item.vaultId && !vault[item.vaultId]) {
      setMessage(
        `Save a ${item.label} token first, then Connect MCP — ADE injects it into ${recipe.envKeys.join(" / ")}.`,
      );
      return;
    }

    setConnectingMcp(recipe.name);
    setMessage(`Connecting ${recipe.name}…`);
    try {
      await onConnectMcp({
        name: recipe.name,
        command: mcpCommandForPlatform(recipe),
        args: recipe.args,
        approved: true,
        recipeId: item.id,
        vaultProvider: item.vaultId ?? null,
        vaultEnvKeys: recipe.envKeys ?? [],
      });
      onRefreshMcp();
      if (recipe.externalEnvKeys?.length) {
        setMessage(
          `Spawned ${recipe.name} (incomplete env). Also set ${recipe.externalEnvKeys.join(" and ")} in the Desktop process env — vault only covers ${recipe.envKeys?.join(" / ") ?? "injected keys"}.`,
        );
      } else {
        setMessage(
          `Spawned ${recipe.name}. Confirm tools appear under Host tools before treating it as ready.`,
        );
      }
    } catch (reason) {
      setMessage(`Failed to connect ${recipe.name}: ${String(reason)}`);
    } finally {
      setConnectingMcp(null);
    }
  };

  /** One-click: confirm spawn line → approve + connect (no expand/checkbox). */
  const addMcpFromRecipe = async (item: IntegrationDef) => {
    const recipe = item.mcpRecipe;
    if (!recipe) return;

    if (mcpServers.includes(recipe.name)) {
      setMessage(`${recipe.name} is already connected this session.`);
      return;
    }

    if (recipe.envKeys?.length && item.vaultId && !vault[item.vaultId]) {
      setExpandedId(item.id);
      setMessage(
        `Save a ${item.label} token first, then Add MCP — ADE injects it into ${recipe.envKeys.join(" / ")}.`,
      );
      return;
    }

    const line = formatMcpRecipeCommand(recipe);
    const ok = window.confirm(
      `Add MCP from recipe "${recipe.name}"?\n\n${line}\n\nVault tokens (if saved) are injected into the process env for this session.`,
    );
    if (!ok) return;

    setMcpApproved((prev) => ({ ...prev, [recipe.name]: true }));
    await connectRecipe(item, { approvedOverride: true });
  };

  const focusItem = (id: string) => {
    const bare = id.startsWith("mcp:") ? id.slice(4) : id;
    const match =
      INTEGRATIONS.find((item) => item.id === bare) ||
      INTEGRATIONS.find((item) => item.mcpRecipe?.name === bare);
    if (match?.category === "host") setTab("host");
    else setTab("connectors");
    setExpandedId(match?.id ?? bare);
  };

  const rowAction = (item: IntegrationDef) => {
    const { configured, mcpLive } = itemMeta(item);
    if (item.openView) {
      return {
        label: "Open",
        onClick: () => onOpenView(item.openView!),
        primary: false,
      };
    }
    if (item.mcpRecipe && !mcpLive) {
      return {
        label: "+",
        title: "Add MCP from recipe",
        onClick: () => void addMcpFromRecipe(item),
        primary: true,
      };
    }
    if (item.kind === "token" && !configured) {
      return {
        label: "+",
        title: "Add token",
        onClick: () => setExpandedId(item.id),
        primary: true,
      };
    }
    return {
      label: "···",
      title: "Details",
      onClick: () =>
        setExpandedId((prev) => (prev === item.id ? null : item.id)),
      primary: false,
    };
  };

  if (!isTauri()) {
    return <DesktopRequired view="Integrations" />;
  }

  return (
    <div
      className="mx-auto flex min-h-0 w-full max-w-4xl flex-col gap-6"
      data-testid="ade-integrations"
    >
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div className="min-w-0 max-w-xl space-y-1.5">
          <h1 className="text-[22px] font-semibold tracking-tight text-ink">
            Integrations
          </h1>
          <p className="text-[13px] leading-5 text-ink-dim">
            Vault tokens are storage only. Green means MCP is live this session.
            Host tools stay on every Desktop turn; model keys live under Keys.
          </p>
        </div>
        <label className="relative w-full max-w-[240px] shrink-0">
          <span className="sr-only">Search integrations</span>
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search"
            className="w-full rounded-lg border border-line bg-surface-2 py-2 pl-3 pr-3 text-[13px] text-ink placeholder:text-ink-faint outline-hidden ring-accent/25 focus:ring-2"
          />
        </label>
      </header>

      <div className="flex flex-wrap items-center gap-3">
        <SubTabs
          className="w-auto"
          ariaLabel="Integrations sections"
          items={[
            { id: "connectors", label: "Connectors" },
            {
              id: "host",
              label: "Host tools",
              badge: hostBadge,
            },
          ]}
          activeId={tab}
          onSelect={(id) => setTab(id as StoreTab)}
        />
        <div className="min-w-0 flex-1" />
        <button
          type="button"
          onClick={() => onOpenKeys()}
          className="rounded-lg border border-line px-3 py-1.5 text-[12px] font-medium text-ink-dim hover:bg-white/5 hover:text-ink"
        >
          Keys
        </button>
        <button
          type="button"
          onClick={() => onOpenMcp()}
          className="rounded-lg border border-accent/35 bg-accent/15 px-3 py-1.5 text-[12px] font-semibold text-blue-100 hover:bg-accent/25"
        >
          MCP console
        </button>
      </div>

      {message && (
        <p className="rounded-lg border border-line bg-surface-2 px-3.5 py-2.5 text-[12px] text-ink-dim">
          {message}
        </p>
      )}

      {tab === "connectors" && recipeQuickAdd.length > 0 && (
        <section
          className="space-y-3"
          data-testid="ade-mcp-recipe-quick-add"
          aria-labelledby="ade-mcp-recipe-heading"
        >
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <div>
              <h2
                id="ade-mcp-recipe-heading"
                className="text-[13px] font-semibold text-ink"
              >
                Add from recipe
              </h2>
              <p className="mt-0.5 text-[12px] text-ink-faint">
                One confirm spawns the reviewed stdio server. Save a vault token
                first when the recipe needs env injection.
              </p>
            </div>
            <a
              href="https://github.com/MrBlockClock/agentic-development-environment/blob/main/docs/guides/mcp-recipes.md"
              target="_blank"
              rel="noreferrer"
              className="text-[12px] text-accent hover:underline"
            >
              Recipe docs
            </a>
          </div>
          <ul className="flex flex-wrap gap-2">
            {recipeQuickAdd.map((item) => (
              <li key={item.id}>
                <button
                  type="button"
                  disabled={busy || connectingMcp === item.mcpRecipe?.name}
                  title={
                    item.mcpRecipe
                      ? formatMcpRecipeCommand(item.mcpRecipe)
                      : item.label
                  }
                  onClick={() => void addMcpFromRecipe(item)}
                  className="inline-flex items-center gap-2 rounded-xl border border-authority/35 bg-authority/10 px-3 py-2 text-[12px] font-semibold text-violet-100 hover:bg-authority/20 disabled:opacity-40"
                >
                  <BrandWell id={item.id} size="sm" status="todo" />
                  {connectingMcp === item.mcpRecipe?.name
                    ? `Adding ${item.label}…`
                    : `Add ${item.label} MCP`}
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}

      {connected.length > 0 ? (
        <section className="space-y-3">
          <div className="flex items-baseline gap-2">
            <h2 className="text-[13px] font-semibold text-ink">Connected</h2>
            <span className="text-[12px] text-ink-faint">
              {connected.length} MCP · {mcpToolCount} tools
            </span>
          </div>
          <ul className="flex flex-wrap gap-2.5">
            {connected.map((row) => (
              <li key={row.id}>
                <button
                  type="button"
                  title={`${row.label} — ${row.tone === "ready" ? "MCP connected" : "MCP spawned (check env)"}`}
                  onClick={() => focusItem(row.id)}
                  className="rounded-[12px] transition hover:opacity-90 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
                >
                  <BrandWell
                    id={row.brandId}
                    size="md"
                    status={row.tone}
                    title={row.label}
                  />
                </button>
              </li>
            ))}
          </ul>
        </section>
      ) : (
        <p className="text-[12px] text-ink-faint">
          No MCP servers connected this session. Built-in host tools are under
          Host tools — they are not listed as “installed.”
        </p>
      )}

      {tab === "connectors" && (
        <div className="space-y-8">
          {connectorGroups.map((group) => {
            const collapsed = Boolean(collapsedCats[group.category]);
            const panelId = `integrations-cat-${group.category}`;
            return (
              <section key={group.category} className="space-y-3">
                <button
                  type="button"
                  aria-expanded={!collapsed}
                  aria-controls={panelId}
                  onClick={() =>
                    setCollapsedCats((prev) => ({
                      ...prev,
                      [group.category]: !collapsed,
                    }))
                  }
                  className="flex items-center gap-1.5 text-[13px] font-semibold text-ink hover:text-accent"
                >
                  {group.label}
                  <span className="text-ink-faint" aria-hidden>
                    {collapsed ? "›" : "▾"}
                  </span>
                </button>
                {!collapsed && (
                  <ul
                    id={panelId}
                    className="grid grid-cols-1 gap-2 md:grid-cols-2 md:gap-x-4 md:gap-y-2"
                  >
                    {group.items.map((item) => (
                      <IntegrationRow
                        key={item.id}
                        item={item}
                        open={expandedId === item.id}
                        meta={itemMeta(item)}
                        action={rowAction(item)}
                        busy={busy}
                        connectingMcp={connectingMcp}
                        approved={Boolean(
                          item.mcpRecipe && mcpApproved[item.mcpRecipe.name],
                        )}
                        onApprovedChange={(value) => {
                          if (!item.mcpRecipe) return;
                          setMcpApproved((prev) => ({
                            ...prev,
                            [item.mcpRecipe!.name]: value,
                          }));
                        }}
                        secret={
                          item.vaultId ? (secrets[item.vaultId] ?? "") : ""
                        }
                        onToggle={() =>
                          setExpandedId((prev) =>
                            prev === item.id ? null : item.id,
                          )
                        }
                        onSecretChange={(value) => {
                          if (!item.vaultId) return;
                          setSecrets((prev) => ({
                            ...prev,
                            [item.vaultId!]: value,
                          }));
                        }}
                        onSave={() =>
                          item.vaultId && void saveToken(item.vaultId)
                        }
                        onClear={() =>
                          item.vaultId && void clearToken(item.vaultId)
                        }
                        onConnect={() => void connectRecipe(item)}
                        onOpenMcp={onOpenMcp}
                        onOpenView={onOpenView}
                      />
                    ))}
                  </ul>
                )}
              </section>
            );
          })}
          {connectorGroups.length === 0 && (
            <EmptyState
              title="No connectors match"
              body="Try another search, or clear the filter."
            />
          )}
        </div>
      )}

      {tab === "host" && (
        <div className="space-y-8">
          <section className="space-y-3">
            <h2 className="text-[13px] font-semibold text-ink">
              {INTEGRATION_CATEGORY_LABEL.host}
            </h2>
            <ul className="grid grid-cols-1 gap-2 md:grid-cols-2 md:gap-x-4 md:gap-y-2">
              {hostItems.fromCatalog.map((item) => (
                <IntegrationRow
                  key={item.id}
                  item={item}
                  open={expandedId === item.id}
                  meta={itemMeta(item)}
                  action={rowAction(item)}
                  busy={busy}
                  connectingMcp={connectingMcp}
                  approved={false}
                  onApprovedChange={() => {}}
                  secret=""
                  onToggle={() =>
                    setExpandedId((prev) => (prev === item.id ? null : item.id))
                  }
                  onSecretChange={() => {}}
                  onSave={() => {}}
                  onClear={() => {}}
                  onConnect={() => {}}
                  onOpenMcp={onOpenMcp}
                  onOpenView={onOpenView}
                />
              ))}
            </ul>
          </section>

          <section className="space-y-3">
            <h2 className="text-[13px] font-semibold text-ink">
              Tools this turn
            </h2>
            <ul className="grid grid-cols-1 gap-2 md:grid-cols-2 md:gap-x-4 md:gap-y-2">
              {hostItems.tools.map((tool) => (
                <li key={tool.id}>
                  <div className="flex items-center gap-3 rounded-xl border border-line bg-surface-2/70 px-3 py-3">
                    <BrandWell id={tool.id} size="md" status="info" />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[13px] font-semibold text-ink">
                        {tool.label}
                      </span>
                      <span className="mt-0.5 block truncate text-[12px] leading-4 text-ink-faint">
                        {tool.note}
                      </span>
                    </span>
                    <span className="shrink-0 rounded-md border border-line px-2 py-1 text-[10px] font-semibold uppercase tracking-wide text-ink-faint">
                      Built-in
                    </span>
                  </div>
                </li>
              ))}
              {mcpServers.map((name) => (
                <li key={`mcp-${name}`}>
                  <div className="flex items-center gap-3 rounded-xl border border-line bg-surface-2/70 px-3 py-3">
                    <BrandWell id="mcp-host" size="md" status="ready" />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[13px] font-semibold text-ink">
                        MCP · {name}
                      </span>
                      <span className="mt-0.5 block truncate text-[12px] leading-4 text-ink-faint">
                        Connected this session
                      </span>
                    </span>
                    <button
                      type="button"
                      onClick={() => onOpenMcp()}
                      className="shrink-0 rounded-lg border border-line px-2.5 py-1.5 text-[12px] font-medium text-ink-dim hover:bg-white/5 hover:text-ink"
                    >
                      Open
                    </button>
                  </div>
                </li>
              ))}
            </ul>
            {hostItems.tools.length === 0 &&
              hostItems.fromCatalog.length === 0 &&
              mcpServers.length === 0 && (
                <EmptyState
                  title="No host tools match"
                  body="Try another search, or clear the filter."
                />
              )}
          </section>
        </div>
      )}
    </div>
  );
}

function IntegrationRow({
  item,
  open,
  meta,
  action,
  busy,
  connectingMcp,
  approved,
  onApprovedChange,
  secret,
  onToggle,
  onSecretChange,
  onSave,
  onClear,
  onConnect,
  onOpenMcp,
  onOpenView,
}: {
  item: IntegrationDef;
  open: boolean;
  meta: {
    configured: boolean;
    mcpLive: boolean;
    tone: ConnTone;
    caption: string;
  };
  action: {
    label: string;
    title?: string;
    onClick: () => void;
    primary: boolean;
  };
  busy: boolean;
  connectingMcp: string | null;
  approved: boolean;
  onApprovedChange: (value: boolean) => void;
  secret: string;
  onToggle: () => void;
  onSecretChange: (value: string) => void;
  onSave: () => void;
  onClear: () => void;
  onConnect: () => void;
  onOpenMcp: () => void;
  onOpenView: (
    view: "Browser" | "Terminal" | "Workspaces" | "Keys" | "MCP",
  ) => void;
}) {
  const { configured, mcpLive, tone, caption } = meta;
  const actionLabel =
    action.label === "+"
      ? "Add"
      : action.label === "···"
        ? "Details"
        : action.label === "…"
          ? "…"
          : action.label;

  return (
    <li className="min-w-0">
      <div
        className={`overflow-hidden rounded-xl border transition ${
          open
            ? "border-line-strong bg-surface-2"
            : "border-line bg-surface-2/60 hover:border-line-strong hover:bg-surface-2/90"
        }`}
      >
        <div className="flex items-center gap-3 px-3 py-3">
          <button
            type="button"
            onClick={onToggle}
            className="flex min-w-0 flex-1 items-center gap-3 text-left"
          >
            <BrandWell id={item.id} size="md" status={tone} />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[13px] font-semibold text-ink">
                {item.label}
              </span>
              <span className="mt-0.5 block truncate text-[12px] leading-4 text-ink-faint">
                {caption}
              </span>
            </span>
          </button>
          <button
            type="button"
            title={action.title ?? actionLabel}
            onClick={(event) => {
              event.stopPropagation();
              action.onClick();
            }}
            className={`shrink-0 rounded-lg border px-2.5 py-1.5 text-[12px] font-semibold transition ${
              action.primary
                ? "border-line bg-surface-3 text-ink hover:bg-white/8"
                : "border-line text-ink-dim hover:bg-white/5 hover:text-ink"
            }`}
          >
            {actionLabel}
          </button>
        </div>

        {open && (
          <div className="space-y-3 border-t border-line bg-surface-0/40 px-3 py-3.5">
            <p className="text-[12px] leading-4 text-ink-dim">{item.blurb}</p>

            {(item.kind === "keys" ||
              item.kind === "surface" ||
              item.kind === "builtin") &&
              item.openView && (
                <button
                  type="button"
                  onClick={() => onOpenView(item.openView!)}
                  className="rounded-lg border border-line bg-surface-3 px-3 py-2 text-[12px] font-semibold text-ink hover:bg-white/8"
                >
                  Open {item.label}
                </button>
              )}

            {item.kind === "token" && item.vaultId && (
              <div className="space-y-2.5">
                <div className="flex flex-wrap items-center gap-2 text-[12px]">
                  <span
                    className={
                      configured ? "font-medium text-warn" : "text-warn"
                    }
                  >
                    {configured
                      ? "•••••••• in vault (not agent-ready alone)"
                      : "No token yet"}
                  </span>
                  {item.getKeyUrl && (
                    <a
                      href={item.getKeyUrl}
                      target="_blank"
                      rel="noreferrer"
                      className="text-accent hover:underline"
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
                    onChange={(event) => onSecretChange(event.target.value)}
                    className="min-w-0 flex-1 rounded-lg border border-line bg-surface-0 px-3 py-2.5 font-mono text-[12px] text-ink"
                  />
                  <button
                    type="button"
                    disabled={busy}
                    onClick={onSave}
                    className="rounded-lg border border-accent/35 bg-accent/20 px-3 py-2.5 text-[12px] font-semibold text-blue-50 hover:bg-accent/30 disabled:opacity-40"
                  >
                    Save
                  </button>
                  {configured && (
                    <button
                      type="button"
                      disabled={busy}
                      onClick={onClear}
                      className="rounded-lg border border-line px-3 py-2.5 text-[12px] font-semibold text-ink-dim hover:bg-white/5 disabled:opacity-40"
                    >
                      Remove
                    </button>
                  )}
                </div>
              </div>
            )}

            {item.mcpRecipe && (
              <Disclosure
                title="MCP recipe"
                summary={
                  mcpLive
                    ? tone === "ready"
                      ? "Connected"
                      : "Spawned · incomplete env"
                    : "Review command, then approve"
                }
                hint="Spawns a reviewed stdio server. Vault tokens are injected into the process env on connect."
                defaultOpen={!mcpLive}
              >
                <div className="space-y-2.5 pt-1">
                  <p className="rounded-md border border-line bg-surface-0 px-2.5 py-2 font-mono text-[11px] leading-4 text-ink-faint">
                    {mcpCommandForPlatform(item.mcpRecipe)}{" "}
                    {item.mcpRecipe.args.join(" ")}
                  </p>
                  {item.mcpRecipe.envHint && (
                    <p className="text-[12px] text-warn/90">
                      Env: {item.mcpRecipe.envHint}
                      {item.vaultId && item.mcpRecipe.envKeys?.length
                        ? configured
                          ? " · vault token will be injected"
                          : " · save a vault token first"
                        : ""}
                      {item.mcpRecipe.externalEnvKeys?.length
                        ? ` · you must also set ${item.mcpRecipe.externalEnvKeys.join(", ")}`
                        : ""}
                    </p>
                  )}
                  {!mcpLive && (
                    <label className="flex cursor-pointer items-start gap-2 text-[12px] text-ink-dim">
                      <input
                        type="checkbox"
                        className="mt-0.5"
                        checked={approved}
                        onChange={(event) =>
                          onApprovedChange(event.target.checked)
                        }
                      />
                      <span>
                        I reviewed this command and approve spawning{" "}
                        <span className="font-mono text-ink">
                          {item.mcpRecipe.name}
                        </span>{" "}
                        for this session.
                      </span>
                    </label>
                  )}
                  <div className="flex flex-wrap gap-2">
                    <button
                      type="button"
                      disabled={
                        busy ||
                        mcpLive ||
                        !approved ||
                        connectingMcp === item.mcpRecipe.name
                      }
                      onClick={onConnect}
                      className="rounded-lg border border-authority/35 bg-authority/15 px-3 py-2 text-[12px] font-semibold text-violet-100 hover:bg-authority/25 disabled:opacity-40"
                    >
                      {mcpLive
                        ? tone === "ready"
                          ? "Already connected"
                          : "Spawned (incomplete)"
                        : connectingMcp === item.mcpRecipe.name
                          ? "Connecting…"
                          : "Connect MCP"}
                    </button>
                    <button
                      type="button"
                      onClick={onOpenMcp}
                      className="rounded-lg border border-line px-3 py-2 text-[12px] text-ink-dim hover:bg-white/5"
                    >
                      Open console
                    </button>
                    {item.mcpRecipe.docsUrl && (
                      <a
                        href={item.mcpRecipe.docsUrl}
                        target="_blank"
                        rel="noreferrer"
                        className="rounded-lg border border-line px-3 py-2 text-[12px] text-accent hover:bg-white/5"
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
      </div>
    </li>
  );
}

