import { ADE_CAPABILITY_MATRIX, desktopRequiredCopy } from "../capabilities";
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
    <div className="mx-auto max-w-xl space-y-4">
      <section className="rounded-2xl border border-amber-400/25 bg-amber-400/8 px-5 py-6">
        <p className="text-[10px] font-semibold uppercase tracking-wider text-amber-200/80">
          Browser preview
        </p>
        <h2 className="mt-2 text-xl font-semibold text-slate-50">{copy.title}</h2>
        <p className="mt-2 text-sm leading-6 text-slate-300">{copy.body}</p>
        <p className="mt-3 text-[12px] leading-5 text-amber-100/85">{copy.next}</p>
        <ol className="mt-4 list-decimal space-y-1.5 pl-5 text-[12px] text-slate-400">
          <li>
            From the repo:{" "}
            <span className="font-mono text-[11px] text-slate-300">
              cargo run -p ade-cli --quiet -- desktop
            </span>{" "}
            (or your usual ADE Desktop launch)
          </li>
          <li>Same workspace root as this preview</li>
          <li>
            {view === "Keys"
              ? "Add your provider key, then use Agent in Desktop"
              : view === "MCP"
                ? "Connect MCP servers in Desktop"
                : "Run Agent turns in Desktop; keep Verify here if you like"}
          </li>
        </ol>
        {!simpleMode && (
          <p className="mt-4 text-[11px] text-slate-500">
            Nav labels match Desktop — only capability differs by shell.
          </p>
        )}
      </section>
    </div>
  );
}

export function CapabilityMatrix({ shell }: { shell: "desktop" | "browser" }) {
  return (
    <Disclosure
      title="What works here"
      subtitle="Same labels · different capability by shell"
      summary={shell === "browser" ? "browser preview" : "desktop"}
      hint="Agent, Keys, and MCP need Desktop. Status, Verify, Recipes, Guidance work in browser when ade serve is authorized."
      storageKey="ade_capability_matrix"
      className="border-white/10 bg-black/20"
    >
      <div className="overflow-x-auto">
        <table className="w-full min-w-[28rem] text-left text-[11px]">
          <thead>
            <tr className="text-[10px] uppercase tracking-wider text-slate-500">
              <th className="py-1.5 pr-3 font-semibold">Capability</th>
              <th className="py-1.5 pr-3 font-semibold">Desktop</th>
              <th className="py-1.5 pr-3 font-semibold">Browser</th>
              <th className="py-1.5 font-semibold">Note</th>
            </tr>
          </thead>
          <tbody>
            {ADE_CAPABILITY_MATRIX.map((row) => {
              const here = shell === "desktop" ? row.desktop : row.browser;
              return (
                <tr
                  key={row.id}
                  className={`border-t border-white/6 ${here ? "text-slate-300" : "text-slate-500"}`}
                >
                  <td className="py-1.5 pr-3 font-medium text-slate-200">{row.label}</td>
                  <td className="py-1.5 pr-3">{row.desktop ? "yes" : "—"}</td>
                  <td className="py-1.5 pr-3">{row.browser ? "yes" : "—"}</td>
                  <td className="py-1.5 text-slate-500">{row.note}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </Disclosure>
  );
}
