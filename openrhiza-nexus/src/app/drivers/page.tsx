import Link from "next/link";

import { getDriverVoteSummary, listDrivers } from "@/app/registry-data";

export const dynamic = "force-dynamic";

function badgeClass(status: string) {
  if (status === "verified") return "border-emerald-400/25 bg-emerald-500/10 text-emerald-200";
  if (status === "testing") return "border-amber-400/25 bg-amber-500/10 text-amber-200";
  return "border-slate-300/20 bg-slate-500/10 text-slate-200";
}

export default function DriversPage() {
  const drivers = listDrivers();

  return (
    <main className="min-h-screen bg-[linear-gradient(180deg,#07111f_0%,#08172a_50%,#040913_100%)] text-slate-100">
      <section className="mx-auto flex w-full max-w-6xl flex-col gap-6 px-6 py-10 md:px-10 md:py-14">
        <header className="rounded-[24px] border border-white/10 bg-slate-950/45 p-7">
          <p className="text-xs uppercase tracking-[0.28em] text-sky-300/75">Board</p>
          <h1 className="mt-2 text-4xl font-semibold text-white">Driver Board</h1>
          <p className="mt-3 max-w-3xl text-sm leading-7 text-slate-300">
            Public view of current driver entries, hardware match keys, and evaluation scores.
          </p>
        </header>

        <div className="grid gap-4">
          {drivers.map((driver: (typeof drivers)[number]) => {
            const votes = getDriverVoteSummary(driver.driver_id);
            return (
              <article key={driver.driver_id} className="rounded-[24px] border border-white/10 bg-slate-950/45 p-6">
                <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
                  <div className="max-w-3xl">
                    <div className="flex flex-wrap items-center gap-3">
                      <h2 className="text-2xl font-semibold text-white">{driver.display_name}</h2>
                      <span className={`rounded-full border px-3 py-1 text-xs uppercase tracking-[0.24em] ${badgeClass(driver.status)}`}>
                        {driver.status}
                      </span>
                    </div>
                    <p className="mt-2 font-mono text-sm text-sky-200">{driver.match_key}</p>
                    <p className="mt-1 text-sm text-slate-400">{driver.hardware}</p>
                    <p className="mt-4 text-sm leading-7 text-slate-300">{driver.summary}</p>
                    <div className="mt-4">
                      <Link href={`/drivers/${driver.driver_id}`} className="text-sm text-sky-200 underline underline-offset-4">
                        Open detail
                      </Link>
                    </div>
                  </div>
                  <div className="grid min-w-[260px] grid-cols-2 gap-3 text-sm">
                    <div className="rounded-2xl border border-white/8 bg-white/5 px-4 py-3">
                      <div className="text-xs uppercase tracking-[0.22em] text-slate-400">Stability</div>
                      <div className="mt-2 text-2xl font-semibold text-white">{driver.stability_score}</div>
                    </div>
                    <div className="rounded-2xl border border-white/8 bg-white/5 px-4 py-3">
                      <div className="text-xs uppercase tracking-[0.22em] text-slate-400">Performance</div>
                      <div className="mt-2 text-2xl font-semibold text-white">{driver.performance_score}</div>
                    </div>
                    <div className="rounded-2xl border border-emerald-400/20 bg-emerald-500/10 px-4 py-3">
                      <div className="text-xs uppercase tracking-[0.22em] text-emerald-200/80">Upvotes</div>
                      <div className="mt-2 text-2xl font-semibold text-white">{votes.upvotes}</div>
                    </div>
                    <div className="rounded-2xl border border-rose-400/20 bg-rose-500/10 px-4 py-3">
                      <div className="text-xs uppercase tracking-[0.22em] text-rose-200/80">Downvotes</div>
                      <div className="mt-2 text-2xl font-semibold text-white">{votes.downvotes}</div>
                    </div>
                  </div>
                </div>
                <div className="mt-5 border-t border-white/8 pt-4 text-xs uppercase tracking-[0.22em] text-slate-500">
                  Updated {driver.updated_at}
                </div>
              </article>
            );
          })}
        </div>
      </section>
    </main>
  );
}
