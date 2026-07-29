import { desktopRequiredCopy } from "../capabilities";
import { ADE_CAPABILITY_MATRIX } from "../capabilities";
import { Disclosure } from "./ui";

export function DesktopRequired({
  view,
  simpleMode = false,
}: {
  view: string;
  simpleMode?: boolean;
}) {
  const copy = desktopRequiredCopy(view);
  return (
    <div className="mx-auto max-w-lg space-y-3">
      <section className="rounded-xl border border-white/10 bg-white/[0.03] px-5 py-5">
        <h2 className="text-lg font-semibold text-slate-50">{copy.title}</h2>
        <p className="mt-2 text-sm leading-6 text-slate-400">{copy.body}</p>
        <p className="mt-3 text-[13px] leading-5 text-slate-300">{copy.next}</p>
        {!simpleMode && (
          <Disclosure
            title="How to open Desktop"
            summary="steps"
            defaultOpen={false}
            storageKey={`ade_desktop_required_${view}`}
            className="mt-4 border-white/8 bg-black/20"
          >
            <ol className="list-decimal space-y-1.5 pl-5 text-[12px] text-slate-400">
              <li>
                From the repo:{" "}
                <span className="font-mono text-[11px] text-slate-300">
                  cd apps/desktop && npm run tauri dev
                </span>
              </li>
              <li>Use the same workspace folder as this preview</li>
              <li>
                {view === "Keys"
                  ? "Add your provider key, then chat from Home"
                  : view === "Integrations"
                    ? "Connect GitHub / Stripe / MCP recipes"
                    : view === "MCP"
                      ? "Connect MCP servers"
                      : "Run agent turns in Desktop; Verify still works here"}
              </li>
            </ol>
          </Disclosure>
        )}
      </section>
    </div>
  );
}

/** Full matrix — keep off Home first paint; use under Settings or collapsed. */
export function CapabilityMatrix({ shell }: { shell: "desktop" | "browser" }) {
  return (
    <Disclosure
      title="Desktop vs browser"
      subtitle="What each shell can do"
      summary={shell === "browser" ? "preview limits" : "full shell"}
      hint="Agent, Keys, and MCP need Desktop. Status, Verify, Stack, and Guidance work here when the local API is connected."
      defaultOpen={false}
      storageKey="ade_capability_matrix"
      className="border-white/10 bg-black/20"
    >
      <div className="overflow-x-auto">
        <table className="w-full min-w-[22rem] text-left text-[11px]">
          <thead>
            <tr className="text-[10px] uppercase tracking-wider text-slate-500">
              <th className="py-1.5 pr-3 font-semibold">Capability</th>
              <th className="py-1.5 pr-3 font-semibold">Desktop</th>
              <th className="py-1.5 font-semibold">Browser</th>
            </tr>
          </thead>
          <tbody>
            {ADE_CAPABILITY_MATRIX.map((row) => {
              const here = shell === "desktop" ? row.desktop : row.browser;
              return (
                <tr
                  key={row.id}
                  className={`border-t border-white/6 ${here ? "text-slate-300" : "text-slate-500"}`}
                  title={row.note}
                >
                  <td className="py-1.5 pr-3 font-medium text-slate-200">
                    {row.label}
                  </td>
                  <td className="py-1.5 pr-3">{row.desktop ? "Yes" : "—"}</td>
                  <td className="py-1.5">{row.browser ? "Yes" : "—"}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </Disclosure>
  );
}
