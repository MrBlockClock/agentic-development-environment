import type { ReactNode } from "react";

const base = {
  width: 16,
  height: 16,
  viewBox: "0 0 16 16",
  fill: "none",
  "aria-hidden": true as const,
};

/** Monochrome marks for Integrations rows + host tool chips. */
export function IntegrationIcon({
  id,
  className = "size-4 shrink-0 text-slate-300",
}: {
  id: string;
  className?: string;
}): ReactNode {
  switch (id) {
    case "github":
      return (
        <svg {...base} className={className}>
          <path
            fill="currentColor"
            d="M8 1.5A6.5 6.5 0 0 0 1.5 8c0 2.87 1.86 5.3 4.44 6.16.32.06.44-.14.44-.31v-1.1c-1.81.4-2.2-.78-2.2-.78-.3-.76-.73-.96-.73-.96-.6-.41.04-.4.04-.4.66.05 1 .68 1 .68.59 1 1.54.71 1.92.54.06-.42.23-.71.42-.87-1.44-.16-2.96-.72-2.96-3.2 0-.71.25-1.29.66-1.74-.07-.16-.29-.82.06-1.7 0 0 .54-.17 1.76.66A6 6 0 0 1 8 4.2c.55 0 1.1.07 1.62.22 1.22-.83 1.76-.66 1.76-.66.35.88.13 1.54.06 1.7.41.45.66 1.03.66 1.74 0 2.49-1.52 3.04-2.97 3.2.24.2.45.6.45 1.21v1.8c0 .17.12.37.44.31A6.51 6.51 0 0 0 14.5 8 6.5 6.5 0 0 0 8 1.5Z"
          />
        </svg>
      );
    case "gitlab":
      return (
        <svg {...base} className={className}>
          <path
            fill="currentColor"
            d="M8 14.2 5.2 5.6h5.6L8 14.2Zm2.9-8.6.9-2.7c.1-.3.5-.3.6 0l2.3 7.1H11.3l-.5-1.5-.9-2.9ZM4.2 5.6l-.9 2.9-.5 1.5H1.2L3.5 2.9c.1-.3.5-.3.6 0l.9 2.7h-.8Z"
          />
        </svg>
      );
    case "azure":
      return (
        <svg {...base} className={className}>
          <path
            fill="currentColor"
            d="M7.2 2.2 2 12.8h3.6L9.4 6l1.6 6.8H14L9.5 2.2H7.2Z"
          />
        </svg>
      );
    case "aws":
      return (
        <svg {...base} className={className}>
          <path
            fill="currentColor"
            d="M4.2 9.6c.4.3 1 .5 1.7.5.7 0 1.2-.2 1.2-.6 0-.3-.2-.5-.8-.7l-.7-.2c-1.1-.3-1.8-.9-1.8-1.8 0-1.1 1-1.9 2.5-1.9.8 0 1.5.2 2 .5l-.5 1.1c-.4-.2-.9-.4-1.5-.4-.6 0-1 .2-1 .5 0 .3.3.5.9.7l.7.2c1.2.3 1.8.9 1.8 1.9 0 1.2-1 2-2.7 2-.9 0-1.8-.2-2.4-.6l.6-1.2Zm5.5-.1c.3.4.8.7 1.5.7.7 0 1.1-.3 1.1-.7 0-.5-.4-.7-1.2-1l-.5-.2c-1.3-.4-1.9-1-1.9-2 0-1.2 1-2 2.5-2 .8 0 1.5.2 2 .6l-.5 1.1c-.4-.3-.9-.4-1.4-.4-.6 0-1 .3-1 .7 0 .4.3.6 1.1.9l.5.2c1.4.4 2 1 2 2.1 0 1.3-1 2.1-2.6 2.1-.9 0-1.7-.2-2.3-.7l.7-1.4Z"
          />
        </svg>
      );
    case "google-cloud":
      return (
        <svg {...base} className={className}>
          <path
            fill="currentColor"
            d="M8.8 3.2 12.6 9H15L9.6 2.2 8.8 3.2Zm-1.6 0L6.4 2.2 1 9h2.4l3.8-5.8ZM3.8 10.2 5.2 12h5.6l1.4-1.8H3.8Z"
          />
        </svg>
      );
    case "stripe":
      return (
        <svg {...base} className={className}>
          <path
            fill="currentColor"
            d="M7.4 6.2c0-.5.4-.7 1.1-.7.8 0 1.8.2 2.6.6V3.6A8 8 0 0 0 8.4 3C5.9 3 4.2 4.3 4.2 6.4c0 3.2 4.4 2.7 4.4 4.1 0 .6-.5.8-1.2.8-.9 0-2-.3-2.9-.7v2.6c1 .4 2 .6 2.9.6 2.6 0 4.4-1.3 4.4-3.4 0-3.4-4.4-2.8-4.4-4.2Z"
          />
        </svg>
      );
    case "slack":
      return (
        <svg {...base} className={className}>
          <path
            fill="currentColor"
            d="M5.6 9.8a1.2 1.2 0 1 1-1.2-1.2h1.2v1.2Zm.6 0A1.2 1.2 0 0 1 7.4 8.6V4.8a1.2 1.2 0 1 0-1.2 1.2v3.8Zm0  .6a1.2 1.2 0 0 1-1.2 1.2H2.4a1.2 1.2 0 1 0 1.2-1.2h2.6Zm4.6-1.8a1.2 1.2 0 1 1 1.2 1.2V8.6H10.8Zm-.6 0A1.2 1.2 0 0 1 8.6 7.4h3.8a1.2 1.2 0 1 0-1.2-1.2H8.6v1.2Zm0-.6A1.2 1.2 0 0 1 9.8 5.6V2.4a1.2 1.2 0 1 0-1.2 1.2v2.6Zm-4.6 1.8a1.2 1.2 0 1 1-1.2-1.2h1.2v1.2Z"
          />
        </svg>
      );
    case "linear":
      return (
        <svg {...base} className={className}>
          <path
            fill="currentColor"
            d="M3.2 10.8 10.8 3.2A5.4 5.4 0 0 0 3.2 10.8Zm1.2 1.6A5.4 5.4 0 0 0 12.4 4.4L4.4 12.4Z"
          />
        </svg>
      );
    case "jira":
      return (
        <svg {...base} className={className}>
          <path
            fill="currentColor"
            d="M8 12.5 3.5 8 8 3.5 9.4 4.9 6.3 8l3.1 3.1L8 12.5Zm4.5-4.5L8 3.5l1.4-1.4L14 6.7 12.5 8Z"
          />
        </svg>
      );
    case "notion":
      return (
        <svg {...base} className={className}>
          <path
            fill="currentColor"
            stroke="currentColor"
            strokeWidth="0.6"
            d="M4.2 3.2h7.2v9.6H5.6L4.2 11.4V3.2Zm2 .8v7.2h1.2V6.2l2.2 5h1.2V4h-1.2v4.2L7.4 4h-1.2Z"
          />
        </svg>
      );
    case "discord":
      return (
        <svg {...base} className={className}>
          <path
            fill="currentColor"
            d="M12.6 4.4A10 10 0 0 0 10.2 3.6l-.2.4a9 9 0 0 0-4 0l-.2-.4A10 10 0 0 0 3.4 4.4C2 6.6 1.6 8.8 1.8 10.9a10 10 0 0 0 3 1.5l.4-.7a6.5 6.5 0 0 1-1.1-.5l.3-.2c2.2 1 4.6 1 6.8 0l.3.2c-.3.2-.7.4-1.1.5l.4.7a10 10 0 0 0 3-1.5c.3-2.4-.3-4.6-1.2-6.5ZM6.4 9.4c-.5 0-.9-.5-.9-1.1s.4-1.1.9-1.1.9.5.9 1.1-.4 1.1-.9 1.1Zm3.2 0c-.5 0-.9-.5-.9-1.1s.4-1.1.9-1.1.9.5.9 1.1-.4 1.1-.9 1.1Z"
          />
        </svg>
      );
    case "keys":
      return (
        <svg {...base} className={className}>
          <path
            fill="currentColor"
            d="M9.8 2.8a3.4 3.4 0 0 0-3.2 4.5L2.4 11.5V13.6h2.1l.7-.7h1.2v-1.2h1.2V10.5l1.2-1.2a3.4 3.4 0 1 0 1-6.5Zm.2 2.2a1.2 1.2 0 1 1 0 2.4 1.2 1.2 0 0 1 0-2.4Z"
          />
        </svg>
      );
    case "mcp-host":
      return (
        <svg {...base} className={className}>
          <path
            fill="currentColor"
            d="M3 4.5h10v2H3v-2Zm0 5h10v2H3v-2Zm2 2.5h6v1.5H5V12Z"
          />
        </svg>
      );
    case "browser":
      return (
        <svg {...base} className={className}>
          <rect
            x="2"
            y="3"
            width="12"
            height="10"
            rx="1.5"
            stroke="currentColor"
            strokeWidth="1.2"
          />
          <path stroke="currentColor" strokeWidth="1.2" d="M2 6h12" />
          <circle cx="4.2" cy="4.5" r="0.6" fill="currentColor" />
          <circle cx="5.8" cy="4.5" r="0.6" fill="currentColor" />
        </svg>
      );
    case "terminal":
      return (
        <svg {...base} className={className}>
          <path
            stroke="currentColor"
            strokeWidth="1.3"
            strokeLinecap="round"
            d="M3.5 5.5 6.5 8 3.5 10.5M8 10.5h4.5"
          />
        </svg>
      );
    case "fs":
      return (
        <svg {...base} className={className}>
          <path
            fill="currentColor"
            d="M2.5 4.5h4l1.2 1.2H13.5v7.3H2.5V4.5Z"
          />
        </svg>
      );
    case "shell":
      return (
        <svg {...base} className={className}>
          <path
            stroke="currentColor"
            strokeWidth="1.3"
            strokeLinecap="round"
            d="M3.5 5.5 6.5 8 3.5 10.5M8 10.5h4.5"
          />
        </svg>
      );
    case "web_fetch":
      return (
        <svg {...base} className={className}>
          <circle cx="8" cy="8" r="5.2" stroke="currentColor" strokeWidth="1.2" />
          <path
            stroke="currentColor"
            strokeWidth="1.2"
            d="M2.8 8h10.4M8 2.8c1.6 1.6 1.6 8.8 0 10.4M8 2.8c-1.6 1.6-1.6 8.8 0 10.4"
          />
        </svg>
      );
    case "web_search":
      return (
        <svg {...base} className={className}>
          <circle cx="7" cy="7" r="3.8" stroke="currentColor" strokeWidth="1.2" />
          <path
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinecap="round"
            d="m10 10 3 3"
          />
        </svg>
      );
    default:
      return (
        <svg {...base} className={className}>
          <circle cx="8" cy="8" r="5" stroke="currentColor" strokeWidth="1.2" />
          <path
            stroke="currentColor"
            strokeWidth="1.2"
            strokeLinecap="round"
            d="M8 5.5v3.2M8 10.8h.01"
          />
        </svg>
      );
  }
}
