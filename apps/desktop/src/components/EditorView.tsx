import { useCallback, useMemo, useState } from "react";
import Editor from "@monaco-editor/react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke, isTauri } from "../ipc";
import { DesktopRequired } from "./DesktopRequired";

type WorkspaceTextFile = {
  path: string;
  absolute: string;
  content: string;
  bytes: number;
  languageHint: string;
};

export function EditorView() {
  const [pathInput, setPathInput] = useState(".ade/dogfood/editor-spike.md");
  const [relativePath, setRelativePath] = useState<string | null>(null);
  const [content, setContent] = useState("");
  const [savedContent, setSavedContent] = useState("");
  const [language, setLanguage] = useState("markdown");
  const [bytes, setBytes] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  const dirty = relativePath != null && content !== savedContent;

  const title = useMemo(() => {
    if (!relativePath) return "Editor";
    return dirty ? `${relativePath} · unsaved` : relativePath;
  }, [dirty, relativePath]);

  const applyFile = useCallback((file: WorkspaceTextFile) => {
    setRelativePath(file.path);
    setPathInput(file.path);
    setContent(file.content);
    setSavedContent(file.content);
    setLanguage(file.languageHint || "plaintext");
    setBytes(file.bytes);
    setError(null);
    setNote(null);
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
              Editor · Monaco spike
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
            className="rounded-md border border-blue-400/20 bg-blue-400/8 px-2.5 py-1 text-[11px] text-blue-100 hover:bg-blue-400/15 disabled:opacity-50"
            disabled={busy || (!dirty && relativePath != null)}
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
            className="min-w-[16rem] flex-1 rounded-md border border-white/10 bg-black/30 px-2.5 py-1.5 font-mono text-[11px] text-slate-200 outline-none placeholder:text-slate-600 focus:border-blue-400/40"
          />
          <span className="text-[10px] text-slate-500">
            {language}
            {bytes > 0 ? ` · ${bytes} B` : ""}
            {dirty ? " · dirty" : ""}
          </span>
        </div>
        <p className="mt-2 text-[10px] text-slate-600">
          Light text edit only — secrets and always-ignore paths are blocked. Prefer Cursor/VS Code
          for extensions and large refactors.
        </p>
        {note && <p className="mt-2 text-[11px] text-emerald-200/90">{note}</p>}
        {error && <p className="mt-2 text-[11px] text-red-200">{error}</p>}
      </section>

      <div className="min-h-[320px] flex-1 overflow-hidden rounded-xl border border-white/8 bg-[#0b0f14]">
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
      </div>
    </div>
  );
}
