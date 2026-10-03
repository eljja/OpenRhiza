import Link from "next/link";

import { listModels } from "@/app/registry-data";

export const dynamic = "force-dynamic";

export default function ModelsPage() {
  const models = listModels();

  return (
    <main className="min-h-screen bg-[linear-gradient(180deg,#07111f_0%,#08172a_50%,#040913_100%)] text-slate-100">
      <section className="mx-auto flex w-full max-w-6xl flex-col gap-6 px-6 py-10 md:px-10 md:py-14">
        <header className="rounded-[24px] border border-white/10 bg-slate-950/45 p-7">
          <p className="text-xs uppercase tracking-[0.28em] text-sky-300/75">Board</p>
          <h1 className="mt-2 text-4xl font-semibold text-white">LLM Board</h1>
          <p className="mt-3 max-w-3xl text-sm leading-7 text-slate-300">
            Human-readable view of remote model endpoints and planned LLM integrations for OpenRhiza nodes.
          </p>
        </header>

        <div className="grid gap-4">
          {models.map((model: (typeof models)[number]) => (
            <article key={model.model_id} className="rounded-[24px] border border-white/10 bg-slate-950/45 p-6">
              <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
                <div className="max-w-3xl">
                  <div className="flex flex-wrap items-center gap-3">
                    <h2 className="text-2xl font-semibold text-white">{model.display_name}</h2>
                    <span className="rounded-full border border-sky-300/20 bg-sky-500/10 px-3 py-1 text-xs uppercase tracking-[0.24em] text-sky-200">
                      {model.status}
                    </span>
                  </div>
                  <div className="mt-2 flex flex-wrap gap-3 text-sm text-slate-400">
                    <span>{model.provider}</span>
                    <span>{model.mode}</span>
                    <span className="font-mono text-sky-200">{model.model_id}</span>
                  </div>
                  <p className="mt-4 text-sm leading-7 text-slate-300">{model.summary}</p>
                  <div className="mt-4">
                    <Link href={`/models/${model.model_id}`} className="text-sm text-sky-200 underline underline-offset-4">
                      Open detail
                    </Link>
                  </div>
                </div>
                <div className="min-w-[240px] rounded-2xl border border-white/8 bg-white/5 px-4 py-4">
                  <div className="text-xs uppercase tracking-[0.22em] text-slate-400">Recommended For</div>
                  <div className="mt-3 flex flex-wrap gap-2">
                    {model.recommended_for.map((item: string) => (
                      <span key={item} className="rounded-full border border-white/8 px-3 py-1 text-xs text-slate-200">
                        {item}
                      </span>
                    ))}
                  </div>
                </div>
              </div>
            </article>
          ))}
        </div>
      </section>
    </main>
  );
}
