import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { invoke, isTauri } from "../ipc";
import {
  ATTACH_MAX_BYTES,
  ATTACH_MAX_COUNT,
  type ChatAttachment,
  looksLikeFilesystemPath,
  makeAttachment,
  refuseAttachmentReason,
} from "./fileKind";
import {
  parseGitHubTicket,
  parseHttpUrl,
  urlChipLabel,
} from "./urlAttach";

type StagedAttachment = {
  name: string;
  path: string;
  absolute: string;
  bytes: number;
  staged: boolean;
  isDir?: boolean;
};

function fileNativePath(file: File): string | null {
  const path = (file as File & { path?: string }).path;
  return typeof path === "string" && path.trim() ? path.trim() : null;
}

async function previewForAbsolute(
  absolute: string,
  kind: string,
): Promise<string | undefined> {
  if (kind !== "image") return undefined;
  try {
    return convertFileSrc(absolute);
  } catch {
    return undefined;
  }
}

function attachmentFromStaged(staged: StagedAttachment): ChatAttachment {
  const base = makeAttachment({
    path: staged.path,
    name: staged.name,
    absolute: staged.absolute,
    size: staged.bytes,
  });
  if (staged.isDir) {
    base.kind = "folder";
  }
  return base;
}

/** Build a url/ticket chip from pasted or dropped http(s) text (no network). */
export function attachmentFromHttpUrl(text: string): ChatAttachment | null {
  const ticket = parseGitHubTicket(text);
  if (ticket) {
    return makeAttachment({
      path: ticket.url,
      name: ticket.label,
      kind: "ticket",
    });
  }
  const parsed = parseHttpUrl(text);
  if (!parsed) return null;
  return makeAttachment({
    path: parsed.url,
    name: urlChipLabel(parsed.url),
    kind: "url",
  });
}

export async function stagePathAsAttachment(
  sourcePath: string,
): Promise<{ ok: true; attachment: ChatAttachment } | { ok: false; error: string }> {
  const refused = refuseAttachmentReason(sourcePath);
  if (refused) return { ok: false, error: refused };
  if (!isTauri()) return { ok: false, error: "Desktop only" };
  try {
    const staged = await invoke<StagedAttachment>("chat_stage_path", {
      sourcePath,
    });
    if (!staged.isDir && staged.bytes > ATTACH_MAX_BYTES) {
      return {
        ok: false,
        error: `File too large (${staged.bytes} bytes; max ${ATTACH_MAX_BYTES})`,
      };
    }
    const attachment = attachmentFromStaged(staged);
    if (attachment.kind === "image") {
      attachment.previewUrl = await previewForAbsolute(staged.absolute, "image");
    }
    return { ok: true, attachment };
  } catch (reason) {
    return { ok: false, error: String(reason) };
  }
}

export async function stageFileBlobAsAttachment(
  file: File,
): Promise<{ ok: true; attachment: ChatAttachment } | { ok: false; error: string }> {
  const refused = refuseAttachmentReason(file.name);
  if (refused) return { ok: false, error: refused };
  if (file.size > ATTACH_MAX_BYTES) {
    return {
      ok: false,
      error: `File too large (${file.size} bytes; max ${ATTACH_MAX_BYTES})`,
    };
  }
  const native = fileNativePath(file);
  if (native) return stagePathAsAttachment(native);
  if (!isTauri()) return { ok: false, error: "Desktop only" };
  // Empty "file" without path often means a folder drop in the webview.
  if (file.size === 0) {
    return {
      ok: false,
      error: `Skipped folder or empty item: ${file.name || "(unnamed)"} — use Attach and pick the folder, or paste its path`,
    };
  }
  try {
    const buffer = new Uint8Array(await file.arrayBuffer());
    const staged = await invoke<StagedAttachment>("chat_stage_bytes", {
      fileName: file.name,
      bytes: Array.from(buffer),
    });
    const attachment = attachmentFromStaged(staged);
    if (attachment.kind === "image") {
      attachment.previewUrl =
        (await previewForAbsolute(staged.absolute, "image")) ??
        URL.createObjectURL(file);
    }
    attachment.mime = file.type || undefined;
    return { ok: true, attachment };
  } catch (reason) {
    return { ok: false, error: String(reason) };
  }
}

export async function ingestFiles(
  files: FileList | File[],
  currentCount: number,
): Promise<{ attachments: ChatAttachment[]; errors: string[] }> {
  const list = Array.from(files);
  const attachments: ChatAttachment[] = [];
  const errors: string[] = [];
  for (const file of list) {
    if (currentCount + attachments.length >= ATTACH_MAX_COUNT) {
      errors.push(`Max ${ATTACH_MAX_COUNT} attachments per message`);
      break;
    }
    const result = await stageFileBlobAsAttachment(file);
    if (result.ok) attachments.push(result.attachment);
    else errors.push(result.error);
  }
  return { attachments, errors };
}

export async function ingestPathText(
  text: string,
  currentCount: number,
): Promise<{ attachments: ChatAttachment[]; errors: string[] }> {
  const trimmed = text.trim().replace(/^["']|["']$/g, "");
  if (!looksLikeFilesystemPath(trimmed)) {
    return { attachments: [], errors: [] };
  }
  if (currentCount >= ATTACH_MAX_COUNT) {
    return {
      attachments: [],
      errors: [`Max ${ATTACH_MAX_COUNT} attachments per message`],
    };
  }
  const result = await stagePathAsAttachment(trimmed);
  if (result.ok) return { attachments: [result.attachment], errors: [] };
  return { attachments: [], errors: [result.error] };
}

/** Paste/drop a single http(s) URL → url or ticket chip (no fetch yet). */
export function ingestUrlText(
  text: string,
  currentCount: number,
): { attachments: ChatAttachment[]; errors: string[] } {
  if (currentCount >= ATTACH_MAX_COUNT) {
    return {
      attachments: [],
      errors: [`Max ${ATTACH_MAX_COUNT} attachments per message`],
    };
  }
  const attachment = attachmentFromHttpUrl(text);
  if (!attachment) return { attachments: [], errors: [] };
  return { attachments: [attachment], errors: [] };
}

/** Fetch URL/ticket page text into `.ade/inbox/fetch-*.md` and annotate the chip. */
export async function fetchUrlAttachment(
  item: ChatAttachment,
): Promise<
  { ok: true; attachment: ChatAttachment } | { ok: false; error: string }
> {
  if (item.kind !== "url" && item.kind !== "ticket") {
    return { ok: false, error: "Only URL/ticket chips can be fetched" };
  }
  if (!isTauri()) return { ok: false, error: "Desktop only" };
  try {
    const staged = await invoke<StagedAttachment>("chat_fetch_url", {
      url: item.path,
    });
    return {
      ok: true,
      attachment: {
        ...item,
        fetchedPath: staged.path,
        size: staged.bytes,
      },
    };
  } catch (reason) {
    return { ok: false, error: String(reason) };
  }
}

/** Extract first pages of a PDF into `.ade/inbox/*.extract.md`. */
export async function extractPdfAttachment(
  item: ChatAttachment,
): Promise<
  { ok: true; attachment: ChatAttachment } | { ok: false; error: string }
> {
  if (item.kind !== "pdf") {
    return { ok: false, error: "Only PDF chips can be extracted" };
  }
  if (!isTauri()) return { ok: false, error: "Desktop only" };
  try {
    const staged = await invoke<StagedAttachment>("chat_extract_pdf", {
      sourcePath: item.absolute ?? item.path,
      maxPages: 8,
    });
    return {
      ok: true,
      attachment: {
        ...item,
        extractedPath: staged.path,
      },
    };
  } catch (reason) {
    return { ok: false, error: String(reason) };
  }
}

/** Extract `.docx` / `.xlsx` into `.ade/inbox/*.extract.md`. */
export async function extractOfficeAttachment(
  item: ChatAttachment,
): Promise<
  { ok: true; attachment: ChatAttachment } | { ok: false; error: string }
> {
  if (item.kind !== "office") {
    return { ok: false, error: "Only Office chips (.docx/.xlsx) can be extracted" };
  }
  if (!isTauri()) return { ok: false, error: "Desktop only" };
  try {
    const staged = await invoke<StagedAttachment>("chat_extract_office", {
      sourcePath: item.absolute ?? item.path,
    });
    return {
      ok: true,
      attachment: {
        ...item,
        extractedPath: staged.path,
      },
    };
  } catch (reason) {
    return { ok: false, error: String(reason) };
  }
}

/** PDF or Office extract into inbox markdown. */
export async function extractDocumentAttachment(
  item: ChatAttachment,
): Promise<
  { ok: true; attachment: ChatAttachment } | { ok: false; error: string }
> {
  if (item.kind === "pdf") return extractPdfAttachment(item);
  if (item.kind === "office") return extractOfficeAttachment(item);
  return { ok: false, error: "Only PDF and Office chips can be extracted" };
}

/** Whisper-class audio → `.ade/inbox/*.transcript.md` (Debug/Advanced). */
export async function transcribeAudioAttachment(
  item: ChatAttachment,
  opts?: { provider?: string },
): Promise<
  { ok: true; attachment: ChatAttachment } | { ok: false; error: string }
> {
  if (item.kind !== "audio") {
    return { ok: false, error: "Only audio chips can be transcribed" };
  }
  if (!isTauri()) return { ok: false, error: "Desktop only" };
  try {
    const staged = await invoke<StagedAttachment>("chat_transcribe_audio", {
      sourcePath: item.absolute ?? item.path,
      provider: opts?.provider ?? null,
      baseUrl: null,
      model: null,
    });
    return {
      ok: true,
      attachment: {
        ...item,
        transcriptPath: staged.path,
      },
    };
  } catch (reason) {
    return { ok: false, error: String(reason) };
  }
}

async function stageSelectedPaths(paths: string[]): Promise<{
  attachments: ChatAttachment[];
  errors: string[];
}> {
  const attachments: ChatAttachment[] = [];
  const errors: string[] = [];
  for (const path of paths) {
    if (attachments.length >= ATTACH_MAX_COUNT) {
      errors.push(`Max ${ATTACH_MAX_COUNT} attachments per message`);
      break;
    }
    const result = await stagePathAsAttachment(path);
    if (result.ok) attachments.push(result.attachment);
    else errors.push(result.error);
  }
  return { attachments, errors };
}

export async function pickAttachmentFiles(): Promise<{
  attachments: ChatAttachment[];
  errors: string[];
}> {
  if (!isTauri()) return { attachments: [], errors: ["Desktop only"] };
  try {
    const selected = await openDialog({
      multiple: true,
      directory: false,
      title: "Attach files for ADE",
    });
    if (!selected) return { attachments: [], errors: [] };
    const paths = Array.isArray(selected) ? selected : [selected];
    return stageSelectedPaths(paths);
  } catch (reason) {
    return {
      attachments: [],
      errors: [
        `File picker failed: ${String(reason)}. Try drop/paste, or use the fallback file input.`,
      ],
    };
  }
}

/** Alt/option on the paperclip: attach a folder as a single path chip. */
export async function pickAttachmentFolder(): Promise<{
  attachments: ChatAttachment[];
  errors: string[];
}> {
  if (!isTauri()) return { attachments: [], errors: ["Desktop only"] };
  try {
    const selected = await openDialog({
      multiple: false,
      directory: true,
      title: "Attach folder path for ADE",
    });
    if (!selected) return { attachments: [], errors: [] };
    const path = Array.isArray(selected) ? selected[0] : selected;
    if (!path) return { attachments: [], errors: [] };
    return stageSelectedPaths([path]);
  } catch (reason) {
    return {
      attachments: [],
      errors: [`Folder picker failed: ${String(reason)}. Paste the folder path instead.`],
    };
  }
}

/** Browser/HTML fallback when the native dialog ACL is unavailable. */
export function pickAttachmentFilesViaInput(): Promise<{
  attachments: ChatAttachment[];
  errors: string[];
}> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = true;
    input.hidden = true;
    document.body.appendChild(input);
    input.addEventListener("change", () => {
      const files = input.files;
      document.body.removeChild(input);
      if (!files?.length) {
        resolve({ attachments: [], errors: [] });
        return;
      }
      void ingestFiles(files, 0).then(resolve);
    });
    input.click();
  });
}

export async function openChatPath(path: string): Promise<void> {
  if (!isTauri()) {
    if (/^https?:/i.test(path)) window.open(path, "_blank", "noopener,noreferrer");
    return;
  }
  await invoke("chat_open_path", { path });
}
