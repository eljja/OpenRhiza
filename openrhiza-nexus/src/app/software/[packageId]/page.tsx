import { notFound } from "next/navigation";

import { getSoftwarePackage } from "@/app/registry-data";

export const dynamic = "force-dynamic";

export default async function SoftwareDetailPage({
  params,
}: {
  params: Promise<{ packageId: string }>;
}) {
  const { packageId } = await params;
  const software = getSoftwarePackage(packageId);

  if (!software) {
    notFound();
  }

  return (
    <main className="min-h-screen bg-[linear-gradient(180deg,#07111f_0%,#08172a_50%,#040913_100%)] text-slate-100">
      <section className="mx-auto flex w-full max-w-4xl flex-col gap-6 px-6 py-10 md:px-10 md:py-14">
        <header className="rounded-[24px] border border-white/10 bg-slate-950/45 p-7">
          <div className="flex flex-wrap items-center gap-3">
            <h1 className="text-4xl font-semibold text-white">{software.display_name}</h1>
            <span className="rounded-full border border-sky-300/20 bg-sky-500/10 px-3 py-1 text-xs uppercase tracking-[0.24em] text-sky-200">
              {software.status}
            </span>
          </div>
          <p className="mt-3 font-mono text-sm text-sky-200">{software.package_id}</p>
          <div className="mt-3 flex gap-4 text-sm text-slate-400">
            <span>{software.category}</span>
            <span>{software.delivery}</span>
            <span>Updated {software.updated_at}</span>
          </div>
          <p className="mt-5 text-sm leading-7 text-slate-300">{software.summary}</p>
        </header>
      </section>
    </main>
  );
}
