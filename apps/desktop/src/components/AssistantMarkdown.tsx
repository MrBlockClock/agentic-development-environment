import type { Components } from "react-markdown";
import type { ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  splitOptionSegments,
  type OptionItem,
} from "./optionSegments";

export {
  splitOptionSegments,
  parseNextActionsPayload,
  NEXT_ACTIONS_SCHEMA,
} from "./optionSegments";
export type { OptionItem, AssistantSegment } from "./optionSegments";

const components: Components = {
  p: ({ children }) => (
    <p className="mb-3 last:mb-0 text-[13px] leading-6 text-slate-200">{children}</p>
  ),
  strong: ({ children }) => (
    <strong className="font-semibold text-slate-50">{children}</strong>
  ),
  em: ({ children }) => <em className="italic text-slate-300">{children}</em>,
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
  li: ({ children }) => <li className="leading-6">{children}</li>,
  a: ({ href, children }) => (
    <a
      href={href}
      className="text-blue-300 underline decoration-blue-400/40 underline-offset-2 hover:text-blue-200"
      target="_blank"
      rel="noreferrer"
    >
      {children}
    </a>
  ),
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
};

function MarkdownChunk({ text }: { text: string }) {
  if (!text.trim()) return null;
  return (
    <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
      {text}
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

/** Render assistant chat markdown (GFM tables, option boxes, no raw HTML). */
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
