import { useCallback, useEffect, useMemo, useState } from "react";
import Editor, { DiffEditor } from "@monaco-editor/react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke, isTauri } from "../ipc";
import { DesktopRequired } from "./DesktopRequired";

export const ADE_EDITOR_INTENT_KEY = "ade_editor_intent";

type WorkspaceTextFile = {
  path: string;
  absolute: string;
  content: string;
  bytes: number;
  languageHint: string;
};

type WorkspaceTextDiff = {
  path: string;
  absolute: string;
  original: string;
  modified: string;
  languageHint: string;
  baseline: string;
};

type HandoffResume = {
  available: boolean;
  id: string;
  changedPaths: string[];
  turnStatus?: string | null;
  createdAt?: string | null;
};

type EditorMode = "edit" | "diff";

type EditorIntent =
  | { mode: "handoff" }
  | { mode: "diff"; path: string }
  | { mode: "edit"; path: string };

function readIntent(): EditorIntent | null {
  try {
    const raw = window.sessionStorage.getItem(ADE_EDITOR_INTENT_KEY);
    if (!raw) return null;
    window.sessionStorage.removeItem(ADE_EDITOR_INTENT_KEY);
    const parsed = JSON.parse(raw) as EditorIntent;
    if (!parsed || typeof parsed !== "object" || !("mode" in parsed)) return null;
    return parsed;
  } catch {
    return null;
  }
}

export function EditorView({
  initialPath,
  autoPick = false,
  onTitleChange,
}: {
  /** Open this path on mount (skips sessionStorage intent when set). */
  initialPath?: string | null;
  /** Open the file picker once when no initialPath. */
  autoPick?: boolean;
  onTitleChange?: (title: string) => void;
} = {}) {
  const [pathInput, setPathInput] = useState(initialPath?.trim() || "");
  const [relativePath, setRelativePath] = useState<string | null>(null);
  const [content, setContent] = useState("");
  const [savedContent, setSavedContent] = useState("");
  const [original, setOriginal] = useState("");
  const [baseline, setBaseline] = useState<"head" | "empty" | null>(null);
  const [language, setLanguage] = useState("markdown");
  const [bytes, setBytes] = useState(0);
  const [mode, setMode] = useState<EditorMode>("edit");
  const [handoffPaths, setHandoffPaths] = useState<string[]>([]);
  const [handoffMeta, setHandoffMeta] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  const dirty = relativePath != null && mode === "edit" && content !== savedContent;

  const title = useMemo(() => {
    if (!relativePath) return mode === "diff" ? "Diff" : "Editor";
    const tag = mode === "diff" ? "diff" : dirty ? "unsaved" : "edit";
    return `${relativePath} · ${tag}`;
  }, [dirty, mode, relativePath]);

  useEffect(() => {
    if (!onTitleChange) return;
    const short = relativePath
      ? relativePath.replace(/\\/g, "/").split("/").filter(Boolean).pop() ??
        relativePath
      : "Editor";
    onTitleChange(short);
  }, [onTitleChange, relativePath]);

  const applyFile = useCallback((file: WorkspaceTextFile) => {
    setRelativePath(file.path);
    setPathInput(file.path);
    setContent(file.content);
    setSavedContent(file.content);
    setOriginal("");
    setBaseline(null);
    setLanguage(file.languageHint || "plaintext");
    setBytes(file.bytes);
    setMode("edit");
    setError(null);
    setNote(null);
  }, []);

  const applyDiff = useCallback((diff: WorkspaceTextDiff) => {
    setRelativePath(diff.path);
    setPathInput(diff.path);
    setContent(diff.modified);
    setSavedContent(diff.modified);
    setOriginal(diff.original);
    setBaseline(diff.baseline === "head" ? "head" : "empty");
    setLanguage(diff.languageHint || "plaintext");
    setBytes(diff.modified.length);
    setMode("diff");
    setError(null);
    setNote(
      diff.baseline === "head"
        ? `Diff vs HEAD · ${diff.path}`
        : `New / untracked · ${diff.path}`,
    );
  }, []);

  const loadPath = useCallback(
    async (path: string) => {
      if (!isTauri()) return;
      const trimmed = path.trim();
      if (!trimmed) {
        setError("Enter a workspace-relative path");
        return;
      }
      setBusy(true);
      setError(null);
      setNote(null);
      try {
        const file = await invoke<WorkspaceTextFile>("workspace_read_text", {
          path: trimmed,
        });
        applyFile(file);
      } catch (reason) {
        setError(String(reason));
      } finally {
        setBusy(false);
      }
    },
    [applyFile],
  );

  const loadDiff = useCallback(
    async (path: string) => {
      if (!isTauri()) return;
      const trimmed = path.trim();
      if (!trimmed) {
        setError("Enter a workspace-relative path");
        return;
      }
      setBusy(true);
      setError(null);
      try {
        const diff = await invoke<WorkspaceTextDiff>("workspace_text_diff", {
          path: trimmed,
        });
        applyDiff(diff);
      } catch (reason) {
        setError(String(reason));
      } finally {
        setBusy(false);
      }
    },
    [applyDiff],
  );

  const loadHandoff = useCallback(async () => {
    if (!isTauri()) return;
    setBusy(true);
    setError(null);
    try {
      const resume = await invoke<HandoffResume>("handoff_resume", {
        id: null,
        hostRunNext: false,
      });
      if (!resume.available) {
        setHandoffPaths([]);
        setHandoffMeta(null);
        setError("No handoff capsule yet. Run an agent turn or Check first.");
        return;
      }
      const paths = (resume.changedPaths ?? []).filter(Boolean);
      setHandoffPaths(paths);
      setHandoffMeta(
        [
          resume.id.slice(0, 8),
          resume.turnStatus?.replaceAll("_", " "),
          resume.createdAt,
        ]
          .filter(Boolean)
          .join(" · "),
      );
      if (paths.length === 0) {
        setNote("Latest handoff has no changed_paths — open a file manually.");
      } else {
        setNote(`Handoff listed ${paths.length} path${paths.length === 1 ? "" : "s"}`);
        await loadDiff(paths[0]);
      }
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }, [loadDiff]);

  const pickOpen = useCallback(async () => {
    if (!isTauri()) return;
    const selected = await open({
      multiple: false,
      directory: false,
      title: "Open workspace text file",
    });
    if (!selected || Array.isArray(selected)) return;
    await loadPath(selected);
  }, [loadPath]);

  useEffect(() => {
    if (!isTauri()) return;
    if (initialPath?.trim()) {
      void loadPath(initialPath.trim());
      return;
    }
    if (autoPick) {
      void pickOpen();
      return;
    }
    const intent = readIntent();
    if (!intent) return;
    if (intent.mode === "handoff") {
      void loadHandoff();
    } else if (intent.mode === "diff") {
      void loadDiff(intent.path);
    } else if (intent.mode === "edit") {
      void loadPath(intent.path);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount / tab open once
  }, []);

  const save = useCallback(async () => {
    if (!isTauri()) return;
    const target = (relativePath ?? pathInput).trim();
    if (!target) {
      setError("Enter a path to save");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const file = await invoke<WorkspaceTextFile>("workspace_write_text", {
        path: target,
        content,
      });
      applyFile(file);
      setNote(`Saved ${file.path} (${file.bytes} bytes)`);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }, [applyFile, content, pathInput, relativePath]);

  if (!isTauri()) {
    return <DesktopRequired view="Editor" />;
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 p-4">
      <section className="shrink-0 rounded-xl border border-white/8 bg-white/[0.02] px-3 py-2.5">
        <div className="flex flex-wrap items-center gap-2">
          <div className="min-w-0 flex-1">
            <p className="text-[10px] font-medium uppercase tracking-[0.12em] text-slate-500">
              Editor · Monaco
            </p>
            <p className="truncate text-[12px] text-slate-200" title={title}>
              {title}
            </p>
          </div>
          <button
            type="button"
            className="rounded-md border border-white/10 bg-white/5 px-2.5 py-1 text-[11px] text-slate-200 hover:bg-white/10 disabled:opacity-50"
            disabled={busy}
            onClick={() => void pickOpen()}
          >
            Open…
          </button>
          <button
            type="button"
            className="rounded-md border border-white/10 bg-white/5 px-2.5 py-1 text-[11px] text-slate-200 hover:bg-white/10 disabled:opacity-50"
            disabled={busy}
            onClick={() => void loadPath(pathInput)}
          >
            Load
          </button>
          <button
            type="button"
            className="rounded-md border border-white/10 bg-white/5 px-2.5 py-1 text-[11px] text-slate-200 hover:bg-white/10 disabled:opacity-50"
            disabled={busy || !pathInput.trim()}
            onClick={() => void loadDiff(pathInput)}
          >
            Diff vs HEAD
          </button>
          <button
            type="button"
            className="rounded-md border border-amber-400/20 bg-amber-400/8 px-2.5 py-1 text-[11px] text-amber-100 hover:bg-amber-400/15 disabled:opacity-50"
            disabled={busy}
            onClick={() => void loadHandoff()}
          >
            From handoff
          </button>
          <button
            type="button"
            className="rounded-md border border-blue-400/20 bg-blue-400/8 px-2.5 py-1 text-[11px] text-blue-100 hover:bg-blue-400/15 disabled:opacity-50"
            disabled={busy || mode === "diff" || (!dirty && relativePath != null)}
            onClick={() => void save()}
          >
            Save
          </button>
        </div>
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <input
            value={pathInput}
            onChange={(event) => setPathInput(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void loadPath(pathInput);
              }
            }}
            spellCheck={false}
            placeholder="workspace-relative path (e.g. .ade/dogfood/notes.md)"
            className="min-w-[16rem] flex-1 rounded-md border border-white/10 bg-black/30 px-2.5 py-1.5 font-mono text-[11px] text-slate-200 outline-hidden placeholder:text-slate-600 focus:border-blue-400/40"
          />
          <span className="text-[10px] text-slate-500">
            {mode}
            {baseline ? ` · ${baseline}` : ""}
            {` · ${language}`}
            {bytes > 0 ? ` · ${bytes} B` : ""}
            {dirty ? " · dirty" : ""}
          </span>
        </div>
        {handoffPaths.length > 0 && (
          <div className="mt-2 flex flex-wrap items-center gap-1.5">
            <span className="text-[10px] text-slate-500">
              Handoff{handoffMeta ? ` · ${handoffMeta}` : ""}:
            </span>
            {handoffPaths.map((path) => (
              <button
                key={path}
                type="button"
                title={path}
                disabled={busy}
                onClick={() => void loadDiff(path)}
                className={`max-w-[14rem] truncate rounded border px-1.5 py-0.5 font-mono text-[10px] ${
                  relativePath === path && mode === "diff"
                    ? "border-amber-400/40 bg-amber-400/15 text-amber-100"
                    : "border-white/10 bg-white/4 text-slate-300 hover:bg-white/8"
                }`}
              >
                {path}
              </button>
            ))}
            {mode === "diff" && relativePath && (
              <button
                type="button"
                className="rounded border border-white/10 bg-white/4 px-1.5 py-0.5 text-[10px] text-slate-300 hover:bg-white/8"
                disabled={busy}
                onClick={() => void loadPath(relativePath)}
              >
                Edit file
              </button>
            )}
          </div>
        )}
        <p className="mt-2 text-[10px] text-slate-600">
          Workspace text under the attached folder · Diff is git HEAD vs working tree ·
          Secrets and always-ignore paths stay blocked.
        </p>
        {note && <p className="mt-2 text-[11px] text-emerald-200/90">{note}</p>}
        {error && <p className="mt-2 text-[11px] text-red-200">{error}</p>}
      </section>

      <div className="min-h-[320px] flex-1 overflow-hidden rounded-xl border border-white/8 bg-[#0b0f14]">
        {mode === "diff" ? (
          <DiffEditor
            height="100%"
            theme="vs-dark"
            language={language}
            original={original}
            modified={content}
            originalModelPath={`original://${relativePath ?? (pathInput || "untitled")}`}
            modifiedModelPath={`modified://${relativePath ?? (pathInput || "untitled")}`}
            options={{
              fontSize: 13,
              minimap: { enabled: false },
              wordWrap: "on",
              scrollBeyondLastLine: false,
              automaticLayout: true,
              readOnly: true,
              renderSideBySide: true,
              originalEditable: false,
            }}
          />
        ) : (
          <Editor
            height="100%"
            theme="vs-dark"
            language={language}
            value={content}
            path={relativePath ?? (pathInput || "untitled.md")}
            options={{
              fontSize: 13,
              minimap: { enabled: false },
              wordWrap: "on",
              scrollBeyondLastLine: false,
              automaticLayout: true,
              tabSize: 2,
              renderLineHighlight: "line",
              padding: { top: 12 },
            }}
            onChange={(next) => setContent(next ?? "")}
          />
        )}
      </div>
    </div>
  );
}
