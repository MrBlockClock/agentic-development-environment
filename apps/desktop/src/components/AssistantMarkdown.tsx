import type { Components } from "react-markdown";
import type { ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import "katex/dist/katex.min.css";
import {
  splitOptionSegments,
  type OptionItem,
} from "./optionSegments";
import {
  normalizeAssistantMath,
  safeMarkdownHref,
} from "./assistantMarkdownPrep";
import { FileLinkChip } from "./AttachmentChips";
import { isFileLikeHref } from "./fileKind";
import { openChatPath } from "./attachIngest";

export {
  splitOptionSegments,
  parseNextActionsPayload,
  NEXT_ACTIONS_SCHEMA,
} from "./optionSegments";
export type { OptionItem, AssistantSegment } from "./optionSegments";
export { normalizeAssistantMath, safeMarkdownHref } from "./assistantMarkdownPrep";

function labelFromChildren(children: ReactNode): string {
  if (typeof children === "string" || typeof children === "number") {
    return String(children);
  }
  if (Array.isArray(children)) {
    return children.map(labelFromChildren).join("");
  }
  return "";
}

const components: Components = {
  p: ({ children }) => (
    <p className="mb-3 last:mb-0 text-[13px] leading-6 text-slate-200">{children}</p>
  ),
  strong: ({ children }) => (
    <strong className="font-semibold text-slate-50">{children}</strong>
  ),
  em: ({ children }) => <em className="italic text-slate-300">{children}</em>,
  del: ({ children }) => (
    <del className="text-slate-500 line-through decoration-slate-500/80">{children}</del>
  ),
  ul: ({ children }) => (
    <ul className="mb-3 list-disc space-y-1 pl-5 text-[13px] leading-6 text-slate-200 last:mb-0">
      {children}
    </ul>
  ),
  ol: ({ children }) => (
    <ol className="mb-3 list-decimal space-y-1 pl-5 text-[13px] leading-6 text-slate-200 last:mb-0">
      {children}
    </ol>
  ),
  li: ({ children, className }) => {
    const task = typeof className === "string" && className.includes("task-list-item");
    return (
      <li className={`leading-6 ${task ? "list-none" : ""} ${className ?? ""}`.trim()}>
        {children}
      </li>
    );
  },
  input: ({ checked, disabled, type }) => {
    if (type !== "checkbox") return null;
    return (
      <input
        type="checkbox"
        checked={Boolean(checked)}
        disabled={disabled !== false}
        readOnly
        className="mr-2 align-middle accent-blue-400"
        aria-hidden
      />
    );
  },
  a: ({ href, children }) => {
    const safe = safeMarkdownHref(href);
    if (!safe) {
      return <span className="text-slate-300">{children}</span>;
    }
    if (isFileLikeHref(safe)) {
      const label = labelFromChildren(children) || safe;
      return (
        <FileLinkChip
          href={safe}
          label={label}
          onOpen={(target) => void openChatPath(target)}
        />
      );
    }
    const external = /^https?:/i.test(safe);
    return (
      <a
        href={safe}
        className="text-blue-300 underline decoration-blue-400/40 underline-offset-2 hover:text-blue-200"
        target={external ? "_blank" : undefined}
        rel={external ? "noreferrer noopener" : undefined}
      >
        {children}
      </a>
    );
  },
  img: ({ src, alt, title }) => {
    const safe = safeMarkdownHref(typeof src === "string" ? src : undefined);
    if (!safe || !/^https?:/i.test(safe)) {
      return (
        <span className="text-[12px] text-slate-500">
          [image blocked{alt ? `: ${alt}` : ""}]
        </span>
      );
    }
    return (
      <img
        src={safe}
        alt={alt ?? ""}
        title={title}
        loading="lazy"
        referrerPolicy="no-referrer"
        className="my-2 max-h-80 max-w-full rounded-lg border border-white/10 object-contain"
      />
    );
  },
  code: ({ className, children }) => {
    const inline = !className;
    if (inline) {
      return (
        <code className="rounded bg-white/8 px-1 py-0.5 font-mono text-[12px] text-slate-100">
          {children}
        </code>
      );
    }
    return (
      <code className={`block font-mono text-[11px] leading-5 text-slate-300 ${className ?? ""}`}>
        {children}
      </code>
    );
  },
  pre: ({ children }) => (
    <pre className="thin-scrollbar mb-3 overflow-x-auto rounded-lg bg-black/30 p-3 last:mb-0">
      {children}
    </pre>
  ),
  blockquote: ({ children }) => (
    <blockquote className="mb-3 border-l-2 border-white/15 pl-3 text-slate-400 last:mb-0">
      {children}
    </blockquote>
  ),
  h1: ({ children }) => (
    <h1 className="mb-2 mt-1 text-[15px] font-semibold text-slate-50 first:mt-0">{children}</h1>
  ),
  h2: ({ children }) => (
    <h2 className="mb-2 mt-1 text-[14px] font-semibold text-slate-50 first:mt-0">{children}</h2>
  ),
  h3: ({ children }) => (
    <h3 className="mb-1.5 mt-1 text-[13px] font-semibold text-slate-100 first:mt-0">{children}</h3>
  ),
  h4: ({ children }) => (
    <h4 className="mb-1 mt-1 text-[13px] font-semibold text-slate-100 first:mt-0">{children}</h4>
  ),
  h5: ({ children }) => (
    <h5 className="mb-1 mt-1 text-[12px] font-semibold text-slate-200 first:mt-0">{children}</h5>
  ),
  h6: ({ children }) => (
    <h6 className="mb-1 mt-1 text-[12px] font-semibold uppercase tracking-wide text-slate-400 first:mt-0">
      {children}
    </h6>
  ),
  hr: () => <hr className="my-3 border-white/10" />,
  table: ({ children }) => (
    <div className="thin-scrollbar mb-3 overflow-x-auto last:mb-0">
      <table className="w-full min-w-[16rem] border-collapse text-left text-[12px] leading-5">
        {children}
      </table>
    </div>
  ),
  thead: ({ children }) => <thead className="border-b border-white/12 text-slate-400">{children}</thead>,
  tbody: ({ children }) => <tbody className="text-slate-200">{children}</tbody>,
  tr: ({ children }) => <tr className="border-b border-white/6 last:border-0">{children}</tr>,
  th: ({ children }) => (
    <th className="px-2 py-1.5 font-semibold first:pl-0 last:pr-0">{children}</th>
  ),
  td: ({ children }) => <td className="px-2 py-1.5 first:pl-0 last:pr-0">{children}</td>,
  // GFM footnotes
  sup: ({ children }) => (
    <sup className="text-[10px] text-blue-300/90">{children}</sup>
  ),
  section: ({ children, className }) => {
    if (typeof className === "string" && className.includes("footnotes")) {
      return (
        <section className="mt-4 border-t border-white/10 pt-3 text-[11px] leading-5 text-slate-500">
          {children}
        </section>
      );
    }
    return <section className={className}>{children}</section>;
  },
};

function MarkdownChunk({ text }: { text: string }) {
  if (!text.trim()) return null;
  const prepared = normalizeAssistantMath(text);
  return (
    <ReactMarkdown
      remarkPlugins={[
        remarkGfm,
        [remarkMath, { singleDollarTextMath: true }],
      ]}
      rehypePlugins={[[rehypeKatex, { throwOnError: false, strict: "ignore" }]]}
      components={components}
    >
      {prepared}
    </ReactMarkdown>
  );
}

function OptionsBox({
  title,
  items,
  onPickOption,
}: {
  title: string;
  items: OptionItem[];
  onPickOption?: (text: string) => void;
}): ReactNode {
  return (
    <div className="mb-3 border-l-2 border-white/15 py-0.5 pl-3 last:mb-0">
      <div className="mb-1.5 text-[12px] leading-5 text-slate-400">{title}</div>
      <div className="flex flex-col gap-0.5">
        {items.map((item, index) => {
          const body = (
            <>
              <span className="mr-2 tabular-nums text-slate-500">{index + 1}.</span>
              {item.label}
            </>
          );
          if (onPickOption) {
            return (
              <button
                key={`${index}-${item.label.slice(0, 24)}`}
                type="button"
                title={item.prompt !== item.label ? item.prompt : undefined}
                onClick={() => onPickOption(item.prompt)}
                className="rounded px-1.5 py-1 text-left text-[13px] leading-5 text-slate-200 hover:bg-white/5 hover:text-slate-50"
              >
                {body}
              </button>
            );
          }
          return (
            <div
              key={`${index}-${item.label.slice(0, 24)}`}
              className="px-1.5 py-1 text-[13px] leading-5 text-slate-200"
            >
              {body}
            </div>
          );
        })}
      </div>
    </div>
  );
}

/** Render assistant chat markdown (GFM, KaTeX math, option boxes, no raw HTML). */
export function AssistantMarkdown({
  text,
  onPickOption,
}: {
  text: string;
  /** Prefill or run — UI-only parsing, zero extra model tokens. */
  onPickOption?: (text: string) => void;
}) {
  if (!text.trim()) return null;
  const segments = splitOptionSegments(text);
  return (
    <div className="ade-assistant-md">
      {segments.map((segment, index) =>
        segment.kind === "options" ? (
          <OptionsBox
            key={`opt-${index}`}
            title={segment.title}
            items={segment.items}
            onPickOption={onPickOption}
          />
        ) : (
          <MarkdownChunk key={`md-${index}`} text={segment.text} />
        ),
      )}
    </div>
  );
}
