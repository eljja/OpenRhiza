import { listEvaluations } from "@/app/registry-data";

export const dynamic = "force-dynamic";

export default function EvaluationsPage() {
  const evaluations = listEvaluations();
  return (
    <main className="min-h-screen bg-[linear-gradient(180deg,#07111f_0%,#08172a_50%,#040913_100%)] text-slate-100">
      <section className="mx-auto flex w-full max-w-6xl flex-col gap-6 px-6 py-10 md:px-10 md:py-14">
        <header className="rounded-[24px] border border-white/10 bg-slate-950/45 p-7">
          <p className="text-xs uppercase tracking-[0.28em] text-sky-300/75">Capability Board</p>
          <h1 className="mt-2 text-4xl font-semibold text-white">Evaluation Board</h1>
          <p className="mt-3 max-w-3xl text-sm leading-7 text-slate-300">Observed outcomes from nodes after trying drivers and other capability artifacts.</p>
        </header>
        <div className="grid gap-4">
          {evaluations.map((evaluation) => (
            <article key={evaluation.evaluation_id} className="rounded-[24px] border border-white/10 bg-slate-950/45 p-6">
              <h2 className="text-2xl font-semibold text-white">{evaluation.subject}</h2>
              <p className="mt-2 font-mono text-sm text-sky-200">{evaluation.driver_id}</p>
              <p className="mt-3 text-sm leading-7 text-slate-300">{evaluation.note}</p>
              <div className="mt-4 text-sm text-slate-400">Stability {evaluation.stability_score} · Performance {evaluation.performance_score} · Node {evaluation.node_id}</div>
            </article>
          ))}
        </div>
      </section>
    </main>
  );
}
