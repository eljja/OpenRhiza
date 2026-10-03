import { notFound } from "next/navigation";

import { getNode, listEvaluationsForNode } from "@/app/registry-data";

export const dynamic = "force-dynamic";

export default async function NodeDetailPage({
  params,
}: {
  params: Promise<{ nodeId: string }>;
}) {
  const { nodeId } = await params;
  const node = getNode(nodeId);

  if (!node) {
    notFound();
  }

  const evaluations = listEvaluationsForNode(nodeId);

  return (
    <main className="min-h-screen bg-[linear-gradient(180deg,#07111f_0%,#08172a_50%,#040913_100%)] text-slate-100">
      <section className="mx-auto flex w-full max-w-5xl flex-col gap-6 px-6 py-10 md:px-10 md:py-14">
        <header className="rounded-[24px] border border-white/10 bg-slate-950/45 p-7">
          <div className="flex flex-wrap items-center gap-3">
            <h1 className="text-4xl font-semibold text-white">{node.node_id}</h1>
            <span className="rounded-full border border-sky-300/20 bg-sky-500/10 px-3 py-1 text-xs uppercase tracking-[0.24em] text-sky-200">
              {node.status}
            </span>
          </div>
          <div className="mt-3 grid gap-2 text-sm text-slate-300">
            <div>Trust tier: {node.trust_tier}</div>
            <div>Last seen: {node.last_seen}</div>
            <div className="font-mono text-sky-200">{node.hardware_fingerprint}</div>
          </div>
          <p className="mt-4 text-sm leading-7 text-slate-300">{node.note}</p>
        </header>

        <section className="rounded-[24px] border border-white/10 bg-slate-950/45 p-6">
          <h2 className="text-xl font-semibold text-white">Transport Capabilities</h2>
          <div className="mt-4 flex flex-wrap gap-3">
            {node.transport_capabilities.map((capability: string) => (
              <span key={capability} className="rounded-full border border-white/8 px-4 py-2 text-sm text-slate-200">
                {capability}
              </span>
            ))}
          </div>
        </section>

        <section className="rounded-[24px] border border-white/10 bg-slate-950/45 p-6">
          <h2 className="text-xl font-semibold text-white">Evaluations</h2>
          <div className="mt-4 space-y-4">
            {evaluations.length === 0 ? (
              <p className="text-sm text-slate-400">No evaluations recorded yet.</p>
            ) : (
              evaluations.map((evaluation: (typeof evaluations)[number]) => (
                <article key={evaluation.evaluation_id} className="rounded-2xl border border-white/8 bg-white/5 p-4">
                  <div className="flex flex-wrap justify-between gap-3 text-sm text-slate-300">
                    <div>{evaluation.subject}</div>
                    <div>{evaluation.created_at}</div>
                  </div>
                  <p className="mt-3 text-sm leading-7 text-slate-300">{evaluation.note}</p>
                </article>
              ))
            )}
          </div>
        </section>
      </section>
    </main>
  );
}
