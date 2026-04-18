import { notFound } from "next/navigation";

import { getDriver, listEvaluationsForDriver } from "@/app/registry-data";

export const dynamic = "force-dynamic";

export default async function DriverDetailPage({
  params,
}: {
  params: Promise<{ driverId: string }>;
}) {
  const { driverId } = await params;
  const driver = getDriver(driverId);

  if (!driver) {
    notFound();
  }

  const evaluations = listEvaluationsForDriver(driverId);

  return (
    <main className="min-h-screen bg-[linear-gradient(180deg,#07111f_0%,#08172a_50%,#040913_100%)] text-slate-100">
      <section className="mx-auto flex w-full max-w-5xl flex-col gap-6 px-6 py-10 md:px-10 md:py-14">
        <header className="rounded-[24px] border border-white/10 bg-slate-950/45 p-7">
          <div className="flex flex-wrap items-center gap-3">
            <h1 className="text-4xl font-semibold text-white">{driver.display_name}</h1>
            <span className="rounded-full border border-sky-300/20 bg-sky-500/10 px-3 py-1 text-xs uppercase tracking-[0.24em] text-sky-200">
              {driver.status}
            </span>
          </div>
          <p className="mt-3 font-mono text-sm text-sky-200">{driver.match_key}</p>
          <p className="mt-2 text-sm text-slate-400">{driver.hardware}</p>
          <p className="mt-5 text-sm leading-7 text-slate-300">{driver.summary}</p>
        </header>

        <section className="grid gap-4 md:grid-cols-2">
          <div className="rounded-[24px] border border-white/10 bg-slate-950/45 p-6">
            <h2 className="text-xl font-semibold text-white">Scores</h2>
            <div className="mt-4 grid grid-cols-2 gap-3">
              <div className="rounded-2xl border border-white/8 bg-white/5 px-4 py-4">
                <div className="text-xs uppercase tracking-[0.22em] text-slate-400">Stability</div>
                <div className="mt-2 text-3xl font-semibold text-white">{driver.stability_score}</div>
              </div>
              <div className="rounded-2xl border border-white/8 bg-white/5 px-4 py-4">
                <div className="text-xs uppercase tracking-[0.22em] text-slate-400">Performance</div>
                <div className="mt-2 text-3xl font-semibold text-white">{driver.performance_score}</div>
              </div>
            </div>
          </div>

          <div className="rounded-[24px] border border-white/10 bg-slate-950/45 p-6">
            <h2 className="text-xl font-semibold text-white">Improvements</h2>
            <ul className="mt-4 space-y-3 text-sm leading-7 text-slate-300">
              {driver.improvements.map((item: string) => (
                <li key={item} className="rounded-2xl border border-white/8 bg-white/5 px-4 py-3">
                  {item}
                </li>
              ))}
            </ul>
          </div>
        </section>

        <section className="rounded-[24px] border border-white/10 bg-slate-950/45 p-6">
          <h2 className="text-xl font-semibold text-white">Recent Evaluations</h2>
          <div className="mt-4 space-y-4">
            {evaluations.length === 0 ? (
              <p className="text-sm text-slate-400">No evaluations recorded yet.</p>
            ) : (
              evaluations.map((evaluation: (typeof evaluations)[number]) => (
                <article key={evaluation.evaluation_id} className="rounded-2xl border border-white/8 bg-white/5 p-4">
                  <div className="flex flex-wrap justify-between gap-3 text-sm text-slate-300">
                    <div>{evaluation.node_id}</div>
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
