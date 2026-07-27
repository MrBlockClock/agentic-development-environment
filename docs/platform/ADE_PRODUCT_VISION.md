---
layout: default
title: ADE PRODUCT VISION
---

# ADE Product Vision: End-Goal & System Design

**Schema:** `ade.product-vision/v1`
**Status:** Comprehensive product specification · 2026-07-17
**Sources:** Market research (pain/desire analysis) + Agent All-in-One v3 + Human Handbook v3 + Competitive analysis (Cursor, Claude Code, JetBrains, Windsurf) + Technical deep-dives

---

## Part 1: The End-Goal Product

### One-Sentence Vision

> An ADE that any professional — solo dev or enterprise team — can open, authenticate, pick a project type, and immediately operate with production-grade security, transparent costs, and agentic workflows that never silently break trust.

### Core Identity Pillars

| Pillar | What It Means | How It Manifests |
|--------|---------------|------------------|
| **Professional** | Built for real work, not prototypes | Compliance-ready (SOC2, HIPAA, GDPR); enterprise SSO; audit logs; SLAs |
| **Robust** | Never loses data, never silently fails | AUDIT→PLAN→EXECUTE phase enforcement; per-change diffs; all mutations tracked |
| **Simple** | Anyone can bootstrap in minutes | Stack recipe wizard; golden-path probe; auto-configure ignores/toolchains/verify scripts |
| **Sleek** | Beautiful, fast, minimal | Tauri 2 (10-40MB); clean UI; progressive disclosure; ≤200ms response target |
| **Transparent** | No surprises in cost, model, or behavior | BYOK; hard spend caps; model provenance displayed; per-action cost visibility |
| **Team-Ready** | Collaboration without chaos | RBAC, workspaces, shared rules, audit trails, handoff capsules |

### Target Audience

| Persona | Needs | ADE Value |
|---------|-------|-----------|
| Solo founder | Ship fast without junk | Stack recipes; automated verify; one-command bootstrap |
| Engineering team | Consistent quality across members | Shared rules; enforced policies; scorecards |
| Enterprise org | Compliance, audit, governance | SSO/SCIM; audit logs; role-based access; BYOK; VPC deployment |
| Agency | Multi-client isolation | Workspace-per-client; separate credentials; per-workspace audit |
| Regulated industry | HIPAA/SOC2/GDPR | Data classification; prompt boundaries; approved model list; no training on data |

---

## Part 2: Teams, Workspaces & Business Logins

### 2.1 Organizational Model

```
ADE Organization (top-level)
├── Organization Settings
│   ├── SSO (SAML 2.0 / OIDC)
│   ├── SCIM provisioning
│   ├── Audit log stream (SIEM: Splunk, Datadog, S3, webhooks)
│   ├── Compliance mode (SOC2 / HIPAA / GDPR / none)
│   ├── IP allowlist / tenant restrictions
│   └── Data retention policy
│
├── Teams (grouping layer)
│   ├── Team: Engineering
│   │   ├── Team-level policies
│   │   ├── Shared MCP servers
│   │   ├── Shared rules (enforced + optional)
│   │   └── Projects...
│   ├── Team: Platform
│   │   └── ...
│   └── Team: Data Science
│       └── ...
│
├── Projects (code + config per repo)
│   ├── Project: my-saas
│   │   ├── Stack recipe: business-saas
│   │   ├── Golden runtime: Dev Container
│   │   ├── Members + roles
│   │   ├── Rules (AGENTS.md + scoped)
│   │   ├── MCP servers per profile
│   │   ├── Secrets (encrypted, per-environment)
│   │   └── Verify scripts
│   └── Project: rust-api
│       └── ...
│
└── Users (cross-organization)
    ├── Workspaces (personal sandboxes)
    ├── API keys (per-user, scoped)
    └── Provider accounts (BYOK)
```

### 2.2 Role-Based Access Control

| Role | Org Level | Team Level | Project Level | Capabilities |
|------|-----------|------------|---------------|--------------|
| **Owner** | Full | Full | Full | Billing, delete org, all settings |
| **Admin** | Manage org, teams, audit | Manage team, members, policies | Full | SSO config, role assignment, rule enforcement |
| **Member** | View org | View team, use shared resources | Edit code, run agents | Default working role |
| **Viewer** | View public data | View team data | Read-only | Audit, compliance, review |
| **Auditor** | Read audit logs only | — | — | Compliance monitoring |
| **External** | — | — | Per-project guest | Limited-time access, specific repos |

### 2.3 Authentication & Identity

```
Identity Providers
├── SAML 2.0 (Okta, Azure AD, Google Workspace, OneLogin)
├── OIDC (any OpenID Connect provider)
├── SCIM 2.0 (auto-provision/deprovision)
├── Domain capture (auto-route users by email domain)
├── Session policies (max duration, idle timeout)
└── MFA enforcement (org-wide or per-team)
```

### 2.4 Secrets Management

```
ADE Secrets Vault (encrypted at rest, per-workspace)
├── Environment variables (per-environment: dev/staging/prod)
├── API keys & tokens (provider API keys, DB tokens)
├── MCP authentication (server credentials)
├── CI/CD secrets (auto-synced to CI platform)
├── SSH keys (agent-accessible, never logged)
└── Audit: every secret access logged (who, when, which agent)
```

**Key principles:**
- Secrets never in prompts, never in handoff capsules, never in rules
- Agent policy: AGENTS.md explicitly forbids reading/quoting secret paths
- Rotate secrets via vault UI, not by editing config files
- Pre-commit hook scans for secret leakage

### 2.5 Audit Logging

| Event Category | Events Captured | Retention |
|----------------|-----------------|-----------|
| Authentication | Login/logout, SSO, MFA, failed attempts | 2 years (configurable) |
| Agent actions | Every file edit, terminal command, MCP call | 1 year |
| Policy changes | Rules added/modified/deleted, role changes | 2 years |
| Secrets | Access events, key rotation, vault changes | 2 years |
| Billing | Subscription changes, spend alerts, cap changes | 7 years |
| Compliance | Export events, data access, audit review | Indefinite |

**SIEM integration:** Splunk, Datadog, S3, webhooks (JSON structured events)

### 2.6 Enterprise Compliance Modes

| Mode | Controls Applied |
|------|-----------------|
| SOC2 | All audit logging; access reviews; change management; incident response |
| HIPAA | Data classification; BAAs; encryption; access controls; audit trails; no training on PHI |
| GDPR | Data residency controls; retention policies; right to deletion; DPA |
| FedRAMP | (Future) IL5-level controls; FIPS 140-3; SCIF-grade isolation |

---

## Part 3: Built-in Best Practices

The ADE ships with **baked-in intelligence** drawn from the 13 stack recipes, the authority order, and the verification ladder — not as optional add-ons but as first-class behaviors.

### 3.1 Stack Recipe Auto-Configuration

When a user selects a recipe, the ADE **automatically**:

1. Generates `.gitignore`, `.cursorignore`, `.dockerignore` with correct patterns for that stack
2. Creates verify scripts (G0-G5) in `scripts/`
3. Writes `AGENTS.md` with canonical authority, commands, and security rules
4. Configures toolchain pins (e.g., `rust-toolchain.toml` for Rust recipes)
5. Sets up the golden-path probe (`scripts/where-am-i.sh`)
6. Adds recipe-specific MCP servers (e.g., Playwright for web recipes)
7. Generates default session profiles (Daily/Ops/Review/Plan)

**Example:** User picks `rust-api-turso` → ADE generates:

```
project/
├── rust-toolchain.toml        # Rust stable pin
├── Cargo.toml                 # axum + turso + tokio + serde
├── .gitignore                 # Rust + Turso + secrets
├── .cursorignore              # mirror + bulky tracked
├── .dockerignore              # no deps in image context
├── AGENTS.md                  # fmt/clippy/test commands, authority, secrets policy
├── scripts/
│   ├── where-am-i.sh          # G0: golden path probe
│   ├── verify-quick.sh        # G2: fmt + clippy
│   ├── verify-full.sh         # G0-G4: + tests + deny
│   └── e2e-smoke.sh           # G5: HTTP contract tests
├── migrations/                # SQLx/refinery migrations
└── .env.example               # DB tokens documented
```

### 3.2 Authority Order as Runtime Enforcement

The ADE **enforces** the authority hierarchy at the agent level:

| Level | Enforcement |
|-------|-------------|
| Law/security/directives | Agent refuses to execute if conflict detected; must escalate to human |
| CI/tests/schemas | Verify scripts are mandatory before "done" — cannot be skipped |
| AGENTS.md | Parsed and loaded as binding instructions; cannot be overridden by chat |
| Scoped rules | Loaded per directory; agent obeys file-level scope |
| Task criteria | Parsed from issue/acceptance criteria |
| Provider adapter | Thin; cannot weaken higher layers |
| Chat memory | Lowest priority; session-scoped only |

**Self-audit:** If agent detects conflicting instructions, it stops and reports the conflict with the authority order resolution.

### 3.3 Ignore Surface Alignment Engine

The ADE monitors all 6 ignore surfaces and **alerts** when they drift:

```
Surface        File               Status
───────        ────               ──────
Git            .gitignore         ✅ Synced
AI Index       .cursorignore      ⚠️ Missing: *.tursodb (added to gitignore but not cursorignore)
Docker         .dockerignore      ❌ Missing entirely
Agent Policy   AGENTS.md          ✅ Synced
Backup/Sync    (workstation cfg)  ⚠️ Not configured
CI/Publish     (package.json)     ✅ No secrets exposed
```

On drift, ADE offers to auto-sync or shows the exact diff needed.

### 3.4 Plan Mode Enforcement

The ADE **automatically** enters PLAN phase when:

- Multi-package changes detected
- Database schema/migration changes
- API contract changes
- Deployment or infrastructure changes
- Secrets/credentials touched
- Configuration changes (rules, MCP, providers)
- Multi-ADE parallel work detected
- Regulated data in scope (HIPAA/SOC2 mode)

**Small fixes bypass PLAN** (single file, no side effects) — not to annoy.

### 3.5 Dependency Escalation (Adopt → Wrap → Fork → Replace)

Built into the recipe system as a first-class workflow:

```
1. Adopt ──→ Upstream works? Pin + test. Done.
2. Wrap  ──→ API churn? Create thin adapter. Upstream still primary.
3. Fork  ──→ Upstream stalled? Fork with sync plan + owner + CI.
4. Replace ──→ Fork cost > rewrite? Migration plan + ADR.
```

The ADE offers this as a guided workflow when it detects a dependency bottleneck.

### 3.6 Context Budget Warnings

Built-in warnings when context usage exceeds thresholds:

| Level | Threshold | Action |
|-------|-----------|--------|
| Info | >70% of budget | Subtle indicator in UI |
| Warning | >90% of budget | Hints to summarize or prune |
| Block | >100% of budget | Forces model switch or session split |

**Default budgets:**
- Always-on instructions: ≤200 tokens
- Rules + root agent file: ≤600-800 tokens
- Skill catalog: ≤6K tokens
- MCP servers (daily profile): 0-2

### 3.7 Handoff Capsules (Automatic Continuity)

Every session produces a JSON handoff capsule:

```json
{
  "schema": "ade.handoff/v1",
  "goal": "Add OIDC login flow",
  "mode": "evaluate_existing",
  "orchestrating_ade": "ade-desktop",
  "branch": "feature/oidc-login",
  "changed_paths": ["src/auth/oidc.rs", "AGENTS.md"],
  "verify_results": {
    "G0": "PASS", "G2": "PASS", "G3": "PASS"
  },
  "score_before": 18,
  "score_after": 24,
  "decisions_touched": ["DEC-A-001"],
  "next_safe_command": "cargo test -p auth"
}
```

Next agent loads capsule, reconstructs state without re-discovery.

---

## Part 4: Analytics & Per-Model Intelligence

### 4.1 Analytics Architecture

```
ADE Client (local)
├── Usage events (model, tokens, cost, session ID)
├── Performance metrics (latency, error rate)
├── Quality signals (accept/reject, reversion rate)
├── Environment info (OS, runtime, recipe)
└── → Encrypted → ADE Cloud Analytics API

ADE Cloud (analytics backend)
├── Time-series DB (cost + usage by model/user/workspace)
├── Quality aggregator (acceptance rates, reversion patterns)
├── Anomaly detector (cost spikes, quality drops, model regressions)
├── Budget tracker (real-time vs. caps)
└── Dashboard API → Client UI + Admin Portal
```

### 4.2 Per-Model Tracking

| Metric | Collection | Display |
|--------|------------|---------|
| **Tokens consumed** | Per-session, per-model, per-user | Real-time counter |
| **Cost** (estimated) | Tokens × model rate | Per-session, daily, monthly |
| **Latency** (TTFT, TPOT) | Client-measured | Per-model average, trends |
| **Error rate** | Timeouts, refusals, failures | Per-model, per-provider |
| **Acceptance rate** | Lines suggested vs. accepted | Per-model comparison |
| **Reversion rate** | Edits reverted within 7 days | Quality signal |
| **Provider uptime** | Heartbeat checks | SLA dashboards |
| **Model version drift** | Version header tracking | Alert on unexpected changes |

### 4.3 Dashboard Views

#### a) Personal Dashboard
```
My Usage Today
├── Sessions: 4
├── Tokens: 142,530 (est. $2.14)
├── Lines accepted: 847 / 1,203 suggested (70%)
├── G5 passes: 3/3
├── Time saved (est.): ~2.3h
└── Model breakdown: Claude 4.5 (65%), GPT-5.5 (25%), Fast model (10%)
```

#### b) Team Dashboard (Admin)
```
Team: Engineering (Last 30 days)
├── Active users: 12/15
├── Total cost: $847.32
│   ├── By model: Claude 4.5 ($412), GPT-5.5 ($289), Fast ($146)
│   └── By user: Avg $70.61/member
├── Quality metrics
│   ├── Avg acceptance rate: 74% (↑2% vs last month)
│   ├── Reversion rate: 8% (↓1% vs last month)
│   ├── Revert cost: $67.79 (8% of total)
│   └── Top reverted models: GPT-5.5 (12%), Claude 4.5 (6%)
├── G5 compliance: 96% (target: 95%+)
├── Top users by output: alice (12K LOC), bob (9K LOC)
└── Anomalies: carol's cost ↑340% (investigate?)
```

#### c) Organization Dashboard (Owner)
```
Organization: Acme Corp (Q3 2026)
├── Total spend: $12,430 (within budget: +$430)
├── By workspace
│   ├── saas-product: $5,200
│   ├── mobile-app: $3,800
│   ├── data-pipeline: $2,100
│   └── internal-tools: $1,330
├── By provider
│   ├── Anthropic: $7,200 (58%)
│   ├── OpenAI: $3,800 (31%)
│   ├── Google: $1,200 (10%)
│   └── Local: $230 (2%)
├── Compliance
│   ├── Audit log completeness: 100%
│   ├── Policy violations: 2 (resolved)
│   └── PII in prompts: 0 (no incidents)
└── Benchmark vs. industry
    ├── Cost per developer: $83 (industry avg: $112)
    ├── Accept rate: 76% (industry avg: 71%)
    └── Velocity index: 142 (industry avg: 100)
```

### 4.4 Budget Controls

| Control | Description |
|---------|-------------|
| **Hard cap** | Absolute max spend per user/workspace/org — agent stops when hit |
| **Soft cap** | Warning at threshold, agent prompts for confirmation |
| **Per-model cap** | Limit spend on specific models (e.g., max $200/month on preview models) |
| **Per-profile cap** | Research/explore profile has lower cap than Daily/Ops |
| **Budget alerts** | Email/Slack/webhook at 50%/75%/90%/100% of budget |
| **Chargeback** | Cost attribution per team/project for internal billing |
| **Auto-approve threshold** | Actions under $X execute without confirmation |

### 4.5 Model Quality Analytics

#### Quality Signals

```
Per-Model Quality Card
├── Model: Claude 4.5 Sonnet
├── Acceptance rate: 78% (avg over 30d)
├── Reversion rate: 6% (within 7 days)
├── Avg edits per session: 14
├── Avg approval time: 12s
├── Lines per dollar: 412
├── G5 pass rate: 97%
├── Best for: Full-stack TS, Rust refactors, architecture
├── Worst for: UI tweaks, regex, legacy PHP
└── Provider uptime: 99.92%
```

#### Model Routing Intelligence

The ADE learns which models perform best on **your specific codebase**:

```json
{
  "model_routing_insights": {
    "fast_model": {
      "best_for": ["lint fixes", "doc strings", "simple refactors", "test stubs"],
      "accept_rate_by_task": {
        "lint": 92,
        "docs": 88,
        "refactor": 65,
        "test": 81
      }
    },
    "strong_model": {
      "best_for": ["architecture", "migrations", "multi-file features", "security review"],
      "accept_rate_by_task": {
        "architecture": 85,
        "migration": 82,
        "feature": 79,
        "security": 91
      }
    }
  }
}
```

Over time, the ADE can **suggest** model routing: "This task looks like an architecture change — switch to Claude 4.5 Sonnet? (estimated cost: +$0.14 vs Fast)".

### 4.6 Anomaly Detection

| Anomaly | Detection | Action |
|---------|-----------|--------|
| Cost spike | >200% of user baseline | Alert admin; suggest model cap |
| Quality drop | Accept rate falls >15% | Suggest model switch; flag session |
| Model version drift | Version header changed | Alert with "model may have changed" |
| Provider outage | >3 consecutive failures | Auto-failover to fallback provider |
| Unusual model routing | Agent using model not in profile | Block; log as policy violation |
| Secret in output | Pattern match on .env/keys | Redact; alert; log to audit |

### 4.7 Privacy & Analytics

| Setting | What's Collected | What's Not |
|---------|-----------------|------------|
| **Anonymous** | Token counts, model names, latency, error rates | Code content, prompts, file names |
| **Usage** | + File extensions, task categories | Code content, prompts |
| **Detailed** | + Aggregated metrics, acceptance rates | Per-user code, prompts |
| **Enterprise** | + Per-user attribution, audit-level detail | Nothing excluded |

- All analytics **opt-in** (personal) or **policy-controlled** (enterprise)
- No training on user code — contractual and technical guarantee
- Self-hosted analytics option for air-gapped deployments

---

## Part 5: Updated Repo Layout (Production-Grade)

```
ade/
├── Cargo.toml                          # Workspace root
├── rust-toolchain.toml                 # Rust version pin
├── flake.nix                           # Nix environment
├── flake.lock                          # Pinned Nix inputs
├── .devcontainer/
│   ├── devcontainer.json               # Dev Container config
│   └── Dockerfile                      # Custom image (optional)
├── AGENTS.md                           # Canonical agent contract for ADE itself
├── .gitignore                          # ADE dev ignores (target/, .env, etc.)
├── .cursorignore                       # AI index ignores
├── .dockerignore                       # Build context ignores
├── .env.example                        # Documented env vars
├── CONTRIBUTING.md                     # How to contribute
├── LICENSE                             # License file
├── SECURITY.md                         # Security policy
├── CODE_OF_CONDUCT.md                  # Community guidelines
│
├── crates/
│   ├── core/                           # Domain types, traits, shared contracts
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── audit.rs               # ade.audit.report/v1 types + scoring
│   │   │   ├── plan.rs                 # ade.plan.report/v1 types + phases
│   │   │   ├── execute.rs             # ade.execute.report/v1 types + verification
│   │   │   ├── handoff.rs             # Session capsule types
│   │   │   ├── recipe.rs              # Stack recipe types
│   │   │   ├── recipe_catalog.rs      # 13 built-in recipes
│   │   │   ├── layer.rs               # L0-L11 model types
│   │   │   ├── authority.rs           # Authority order types
│   │   │   ├── profile.rs             # Session profiles
│   │   │   ├── ignore.rs              # Ignore surface alignment
│   │   │   ├── verify.rs              # G0-G5 gate types
│   │   │   ├── analytics.rs           # Analytics event types
│   │   │   └── error.rs               # Unified error types
│   │   └── Cargo.toml
│   │
│   ├── db/                            # Turso database layer
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── schema.rs             # Migrations, schema
│   │   │   ├── repo.rs               # Repository traits + implementations
│   │   │   ├── workspace.rs          # Workspace CRUD
│   │   │   ├── team.rs               # Team CRUD
│   │   │   ├── user.rs               # User CRUD (local cache of SSO)
│   │   │   ├── secrets.rs            # Encrypted secrets vault
│   │   │   ├── audit.rs              # Audit log storage
│   │   │   ├── analytics.rs          # Analytics event store
│   │   │   ├── rules.rs              # Team/project rules storage
│   │   │   └── sync.rs               # Turso push/pull (optional cloud sync)
│   │   ├── migrations/               # SQL migration files
│   │   └── Cargo.toml
│   │
│   ├── workflow/                      # DAG execution engine
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── dag.rs                # Phase DAG builder
│   │   │   ├── executor.rs           # Phase executor with rollback
│   │   │   ├── token.rs              # Token budget tracking
│   │   │   ├── trigger.rs            # Event triggers (git hook, CI webhook)
│   │   │   ├── verify.rs             # G0-G5 runner
│   │   │   ├── plan_enforcement.rs   # Auto-enter PLAN when criteria met
│   │   │   └── parallel.rs           # Worktree management + path leases
│   │   └── Cargo.toml
│   │
│   ├── agents/                        # LLM orchestration
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── router.rs             # Phase router (AUDIT/PLAN/EXECUTE)
│   │   │   ├── start_prompt.rs       # System prompt assembly
│   │   │   ├── provider.rs           # BYOK provider abstraction
│   │   │   ├── provider_tracker.rs   # Per-model cost + quality tracking
│   │   │   ├── model_selector.rs     # Intelligence: suggest best model for task
│   │   │   ├── mcp.rs                # MCP host + server (rmcp)
│   │   │   ├── context.rs            # Context assembly + budget warnings
│   │   │   ├── tool.rs               # Tool execution + sandbox
│   │   │   ├── authority.rs          # Authority order enforcer
│   │   │   ├── ignore_enforcer.rs    # Policy: refuse secret paths
│   │   │   └── handoff.rs            # Capsule generation + load
│   │   └── Cargo.toml
│   │
│   ├── service/                       # Background daemon
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── daemon.rs             # daemon-kit lifecycle
│   │   │   ├── health.rs             # Health check endpoints
│   │   │   ├── scheduler.rs          # Background task scheduling
│   │   │   ├── analytics_uploader.rs # Batch analytics to cloud
│   │   │   ├── anomaly_detector.rs   # Cost/quality anomaly detection
│   │   │   ├── drift_monitor.rs      # Ignore surface drift checker
│   │   │   └── updater.rs            # Auto-update
│   │   └── Cargo.toml
│   │
│   ├── api/                           # HTTP + WebSocket API
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── routes.rs             # Axum router composition
│   │   │   ├── auth.rs               # Session auth, JWT, SSO callback
│   │   │   ├── workspace_routes.rs   # CRUD + members
│   │   │   ├── team_routes.rs        # CRUD + members + policies
│   │   │   ├── project_routes.rs     # CRUD + rules + configs
│   │   │   ├── agent_routes.rs       # Agent session API
│   │   │   ├── analytics_routes.rs   # Dashboard queries
│   │   │   ├── audit_routes.rs       # Audit log queries
│   │   │   ├── sse.rs                # Real-time event stream
│   │   │   ├── ws.rs                 # WebSocket for agent streams
│   │   │   └── middleware.rs         # Authz, rate limit, audit
│   │   └── Cargo.toml
│   │
│   ├── desktop/                       # Tauri shell + IPC bridge
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── commands.rs           # Tauri IPC commands
│   │   │   ├── menu.rs               # Native menu
│   │   │   ├── tray.rs               # System tray
│   │   │   ├── updater.rs            # Tauri updater integration
│   │   │   └── analytics.rs          # Local analytics collection
│   │   └── Cargo.toml
│   │
│   ├── plugins/                       # Plugin host
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── wasm.rs               # WASM runtime (wasmtime)
│   │   │   ├── mcp_ext.rs            # MCP-based plugin loader
│   │   │   ├── registry.rs           # Plugin discovery
│   │   │   └── sandbox.rs            # Plugin sandbox permissions
│   │   └── Cargo.toml
│   │
│   └── cloud/                         # Cloud service (optional SaaS layer)
│       ├── src/
│       │   ├── lib.rs
│       │   ├── org_api.rs            # Organization management API
│       │   ├── billing.rs            # Subscription, usage metering
│       │   ├── analytics_pipeline.rs # Aggregation, anomaly detection
│       │   ├── sso_handler.rs        # SAML/OIDC assertions
│       │   ├── scim_handler.rs       # SCIM provisioning
│       │   └── audit_stream.rs       # SIEM export
│       └── Cargo.toml
│
├── apps/
│   ├── desktop/                       # Tauri 2 app
│   │   ├── src/                      # Frontend (React/Vue/Svelte + TypeScript)
│   │   │   ├── main.tsx
│   │   │   ├── App.tsx
│   │   │   ├── components/
│   │   │   │   ├── layout/           # Shell, sidebar, tabs
│   │   │   │   ├── workspace/        # Workspace view, project list
│   │   │   │   ├── agent/            # Agent chat, phase visualizer
│   │   │   │   ├── analytics/        # Dashboards, charts, cost viz
│   │   │   │   ├── settings/         # Profile, model, MCP settings
│   │   │   │   ├── rules/            # Rule editor, frontmatter UI
│   │   │   │   ├── recipes/          # Stack recipe wizard
│   │   │   │   ├── verify/           # G0-G5 results view
│   │   │   │   ├── audit/            # Audit log viewer
│   │   │   │   └── admin/            # Org/team management
│   │   │   ├── hooks/
│   │   │   ├── stores/
│   │   │   ├── types/
│   │   │   └── utils/
│   │   ├── src-tauri/                # Tauri Rust backend
│   │   │   ├── src/
│   │   │   │   ├── main.rs
│   │   │   │   └── lib.rs
│   │   │   ├── Cargo.toml
│   │   │   └── tauri.conf.json
│   │   ├── public/
│   │   ├── index.html
│   │   ├── package.json
│   │   ├── tsconfig.json
│   │   ├── vite.config.ts
│   │   └── tailwind.config.js
│   │
│   ├── cli/                           # Standalone CLI
│   │   └── src/
│   │       ├── main.rs
│   │       ├── commands/
│   │       │   ├── audit.rs          # ade audit
│   │       │   ├── plan.rs           # ade plan
│   │       │   ├── execute.rs        # ade execute
│   │       │   ├── init.rs           # ade init (recipe wizard)
│   │       │   ├── verify.rs         # ade verify
│   │       │   ├── workspace.rs      # ade workspace
│   │       │   ├── analytics.rs      # ade analytics
│   │       │   ├── secrets.rs        # ade secrets
│   │       │   └── config.rs         # ade config
│   │       └── Cargo.toml
│   │
│   └── admin-portal/                  # Web-based admin dashboard
│       ├── src/
│       │   ├── components/
│       │   │   ├── org-settings/
│       │   │   ├── team-management/
│       │   │   ├── audit-viewer/
│       │   │   ├── analytics/
│       │   │   ├── billing/
│       │   │   └── policies/
│       │   └── App.tsx
│       ├── package.json
│       └── vite.config.ts
│
├── scripts/                           # ADE's own verify scripts (eat own dogfood)
│   ├── where-am-i.sh                  # G0: golden path probe
│   ├── verify-quick.sh                # G2: fmt + clippy
│   ├── verify-full.sh                 # G0-G4: all gates
│   ├── e2e-smoke.sh                   # G5: Playwright
│   ├── build-docs.sh                  # Assemble docs
│   └── check-ignore-alignment.sh      # Verify ignore surface sync
│
├── docs/
│   ├── platform/                      # Architecture & product docs
│   │   ├── ARCHITECTURE_SYNTHESIS.md  # Unified technical architecture
│   │   ├── ADE_PRODUCT_VISION.md      # ← This document
│   │   ├── IDEAL_ADE_HUMAN.md         # Human handbook (assembled)
│   │   └── IDEAL_ADE_AGENT.md         # Agent packet protocol (assembled)
│   ├── decisions/                     # ADEs (Architecture Decision Records)
│   │   ├── index.md
│   │   ├── DEC-A-001.md               # Canonical truth + thin adapters
│   │   ├── DEC-A-002.md               # Stack recipe contract
│   │   ├── DEC-A-003.md               # Turso/libSQL scope
│   │   ├── DEC-A-004.md               # BYOK architecture (Hybrid pattern)
│   │   ├── DEC-A-005.md               # MCP host + server mode
│   │   ├── DEC-G-001.md               # Wiki vs docs authority
│   │   ├── DEC-P-001.md               # Human + Agent editions
│   │   ├── DEC-P-002.md               # Planning before act
│   │   ├── DEC-P-003.md               # Playwright as G5 evidence
│   │   ├── DEC-P-004.md               # Ignore surfaces
│   │   ├── DEC-P-005.md               # Adopt/wrap/fork/replace
│   │   ├── DEC-P-006.md               # Analytics data model + privacy
│   │   ├── DEC-P-007.md               # Team/workspace RBAC model
│   │   └── DEC-P-008.md               # Secrets vault architecture
│   ├── guides/                        # User documentation
│   │   ├── getting-started.md
│   │   ├── workspace-setup.md
│   │   ├── team-admin.md
│   │   ├── recipe-selection.md
│   │   ├── analytics-dashboard.md
│   │   └── security-best-practices.md
│   └── api/                           # API reference
│       ├── openapi.yaml
│       └── mcp-protocol.md
│
├── .ade/                              # ADE's own configuration (dogfooding)
│   ├── rules/                         # .mdc rule files for ADE development
│   │   ├── rust-style.mdc
│   │   ├── commit-standards.mdc
│   │   └── security-review.mdc
│   ├── skills/                        # Skill definitions
│   ├── handoff/                       # Session capsules (gitignored)
│   ├── brain/                         # Learned patterns (version controlled)
│   └── profiles/                      # Session profiles for ADE devs
│       ├── daily.toml
│       ├── review.toml
│       └── ops.toml
│
├── tests/                             # Integration + E2E tests
│   ├── e2e/
│   │   ├── smoke/                     # Critical path Playwright tests
│   │   │   ├── login.spec.ts
│   │   │   ├── workspace.spec.ts
│   │   │   └── agent-session.spec.ts
│   │   └── helpers/
│   │       └── auth.ts
│   ├── integration/
│   │   ├── api/
│   │   ├── workflow/
│   │   └── plugins/
│   └── fixtures/
│
├── .github/
│   ├── workflows/
│   │   ├── ci.yml                     # CI: fmt + clippy + test + verify
│   │   ├── docs.yml                   # Build + deploy docs
│   │   ├── release.yml                # Build + sign + notarize
│   │   ├── audit.yml                  # Weekly dependency audit
│   │   └── scorecard.yml              # OpenSSF scorecard
│   ├── ISSUE_TEMPLATE/
│   ├── PULL_REQUEST_TEMPLATE.md
│   ├── CODEOWNERS
│   └── dependabot.yml
│
├── docker/
│   ├── Dockerfile                     # Production image
│   ├── docker-compose.yml             # Local dev environment
│   └── .dockerignore
│
├── nix/
│   ├── default.nix
│   ├── shell.nix
│   └── modules/
│
├── package.json                       # Frontend build tooling
├── tsconfig.json                      # TypeScript config
├── vite.config.ts                     # Vite config
├── tailwind.config.js                 # Tailwind CSS
├── postcss.config.js
└── README.md
```

---

## Part 6: Competitive Positioning

### Where We Win

| Dimension | Cursor | Claude Code | JetBrains AI | **ADE (Our Product)** |
|-----------|--------|-------------|--------------|----------------------|
| **Trust/Reliability** | ❌ Silent reversions, data loss | ✅ CLI-native, stable | ✅ Mature IDE | **✅ AUDIT→PLAN→EXECUTE; all mutations tracked; never silent** |
| **Cost Transparency** | ❌ Opaque, model swapped | ✅ BYOK, per-user tracking | ❌ Credits opaque | **✅ BYOK native; hard caps; per-action cost; model provenance** |
| **Rules Enforcement** | ❌ Rules often ignored | ✅ CLAUDE.md respected | ✅ Settings enforced | **✅ Authority order as runtime; self-audit on conflicts** |
| **Multi-Agent** | ❌ Partial (agents window) | ❌ Single session | 🟡 Junie + agents | **✅ Git worktree orchestration; path leases; ownership model** |
| **Team Governance** | 🟡 Enterprise-locked | 🟡 Enterprise features | 🟡 Maturing | **✅ Built from ground up; SSO/RBAC/audit in all tiers** |
| **Security** | ❌ 0-days unpatched | ✅ VPC deployment | ✅ On-prem option | **✅ Prompt injection protection; sandbox exec; 6-layer ignores** |
| **Analytics** | 🟡 Basic dashboard | ✅ Good per-user | 🟡 Credits opaque | **✅ Per-model quality; anomaly detection; cross-tool view** |
| **Simplicity** | 🟡 Feature-bloated | ❌ Terminal-only | 🟡 IDE-centric | **✅ Recipe wizard; auto-configure; progressive disclosure** |
| **Cross-Platform** | ✅ VS Code fork | ✅ CLI (all OS) | ✅ JetBrains stack | **✅ Tauri 2 native (10-40MB); Win/Mac/Linux** |

### Pricing Model (Suggestion)

| Tier | Price | Key Features |
|------|-------|--------------|
| **Free** | $0 | 1 workspace, 3 recipes, 1 model provider per day, basic analytics |
| **Pro** | $20/mo | Unlimited workspaces, all recipes, BYOK, per-model analytics, handoff capsules |
| **Team** | $15/user/mo | + Shared rules, team dashboard, MCP server sharing, workspace isolation |
| **Enterprise** | Custom | + SSO/SCIM, audit logs, compliance modes, VPC deployment, admin API, SLA |

---

## Part 7: Key Product Principles

### UX Principles

1. **Progressive disclosure** — Show complexity only when needed. Recipe wizard → expert mode.
2. **Default to safe** — All security features on by default. Opt-out, not opt-in.
3. **Fail visibly** — Never silently fail. Every error has a message, a cause, and a suggested action.
4. **Human in the loop** — 53% of devs want approval gates. Respect that.
5. **Local-first** — Work offline. Sync when connected. No vendor lock-in.
6. **No training on your data** — Contractual. Technical. Non-negotiable.

### Development Principles

1. **Eat your own dogfood** — ADE development uses the ADE itself. ALL phases documented.
2. **Security is not optional** — SDL from day one. Fuzzing, audit, pen testing.
3. **Open core** — Core protocol open source. Cloud analytics and enterprise features are commercial.
4. **Backward compatibility** — Packet schemas versioned. No breaking changes without migration.
5. **Community-driven recipes** — 13 built-in, extensible by community. PR-based contributions.

---

*End of Product Vision. This document defines the complete end-goal product combining the Agent All-in-One v3 protocol, Human Handbook v3 practices, competitive analysis (Cursor/Claude Code/JetBrains/Windsurf), and market research into a single product specification. Ready to begin implementation.*
