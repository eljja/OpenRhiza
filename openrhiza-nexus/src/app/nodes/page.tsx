import Link from "next/link";

import { listEvaluations, listNodes } from "@/app/registry-data";

export const dynamic = "force-dynamic";

export default function NodesPage() {
  const nodes = listNodes();
  const evaluations = listEvaluations();

  return (
    <main className="min-h-screen bg-[linear-gradient(180deg,#07111f_0%,#08172a_50%,#040913_100%)] text-slate-100">
      <section className="mx-auto flex w-full max-w-6xl flex-col gap-6 px-6 py-10 md:px-10 md:py-14">
        <header className="rounded-[24px] border border-white/10 bg-slate-950/45 p-7">
          <p className="text-xs uppercase tracking-[0.28em] text-sky-300/75">Board</p>
          <h1 className="mt-2 text-4xl font-semibold text-white">Node Board</h1>
          <p className="mt-3 max-w-3xl text-sm leading-7 text-slate-300">
            Public node status and the latest evaluation reports currently exposed by the registry.
          </p>
        </header>

        <div className="grid gap-6 lg:grid-cols-[1fr_1fr]">
          <section className="rounded-[24px] border border-white/10 bg-slate-950/45 p-6">
            <h2 className="text-2xl font-semibold text-white">Nodes</h2>
            <div className="mt-5 space-y-4">
              {nodes.map((node: (typeof nodes)[number]) => (
                <article key={node.node_id} className="rounded-2xl border border-white/8 bg-white/5 p-4">
                  <div className="flex flex-wrap items-center gap-3">
                    <h3 className="text-lg font-semibold text-white">{node.node_id}</h3>
                    <span className="rounded-full border border-sky-300/20 bg-sky-500/10 px-3 py-1 text-xs uppercase tracking-[0.24em] text-sky-200">
                      {node.status}
                    </span>
                  </div>
                  <div className="mt-3 grid gap-2 text-sm text-slate-300">
                    <div>Trust tier: {node.trust_tier}</div>
                    <div>Last seen: {node.last_seen}</div>
                    <div className="font-mono text-sky-200">{node.hardware_fingerprint}</div>
                  </div>
                  <p className="mt-3 text-sm leading-7 text-slate-300">{node.note}</p>
                  <div className="mt-3">
                    <Link href={`/nodes/${node.node_id}`} className="text-sm text-sky-200 underline underline-offset-4">
                      Open detail
                    </Link>
                  </div>
                </article>
              ))}
            </div>
          </section>

          <section className="rounded-[24px] border border-white/10 bg-slate-950/45 p-6">
            <h2 className="text-2xl font-semibold text-white">Evaluations</h2>
            <div className="mt-5 space-y-4">
              {evaluations.map((evaluation: (typeof evaluations)[number]) => (
                <article key={evaluation.evaluation_id} className="rounded-2xl border border-white/8 bg-white/5 p-4">
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div>
                      <h3 className="text-lg font-semibold text-white">{evaluation.subject}</h3>
                      <div className="mt-1 text-sm text-slate-400">{evaluation.node_id}</div>
                    </div>
                    <div className="text-right text-sm text-slate-300">
                      <div>Stability {evaluation.stability_score}</div>
                      <div>Performance {evaluation.performance_score}</div>
                    </div>
                  </div>
                  <p className="mt-3 text-sm leading-7 text-slate-300">{evaluation.note}</p>
                  <div className="mt-3 text-xs uppercase tracking-[0.22em] text-slate-500">
                    {evaluation.created_at}
                  </div>
                </article>
              ))}
            </div>
          </section>
        </div>
      </section>
    </main>
  );
}
