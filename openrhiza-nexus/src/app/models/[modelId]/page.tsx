import { notFound } from "next/navigation";

import { getModel } from "@/app/registry-data";

export const dynamic = "force-dynamic";

export default async function ModelDetailPage({
  params,
}: {
  params: Promise<{ modelId: string }>;
}) {
  const { modelId } = await params;
  const model = getModel(modelId);

  if (!model) {
    notFound();
  }

  return (
    <main className="min-h-screen bg-[linear-gradient(180deg,#07111f_0%,#08172a_50%,#040913_100%)] text-slate-100">
      <section className="mx-auto flex w-full max-w-4xl flex-col gap-6 px-6 py-10 md:px-10 md:py-14">
        <header className="rounded-[24px] border border-white/10 bg-slate-950/45 p-7">
          <div className="flex flex-wrap items-center gap-3">
            <h1 className="text-4xl font-semibold text-white">{model.display_name}</h1>
            <span className="rounded-full border border-sky-300/20 bg-sky-500/10 px-3 py-1 text-xs uppercase tracking-[0.24em] text-sky-200">
              {model.status}
            </span>
          </div>
          <div className="mt-3 flex flex-wrap gap-4 text-sm text-slate-400">
            <span>{model.provider}</span>
            <span>{model.mode}</span>
            <span className="font-mono text-sky-200">{model.model_id}</span>
          </div>
          <p className="mt-5 text-sm leading-7 text-slate-300">{model.summary}</p>
        </header>

        <section className="rounded-[24px] border border-white/10 bg-slate-950/45 p-6">
          <h2 className="text-xl font-semibold text-white">Recommended For</h2>
          <div className="mt-4 flex flex-wrap gap-3">
            {model.recommended_for.map((item: string) => (
              <span key={item} className="rounded-full border border-white/8 px-4 py-2 text-sm text-slate-200">
                {item}
              </span>
            ))}
          </div>
        </section>
      </section>
    </main>
  );
}
