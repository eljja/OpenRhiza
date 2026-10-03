import Link from "next/link";

import { listSoftwarePackages } from "@/app/registry-data";

export const dynamic = "force-dynamic";

export default function SoftwarePage() {
  const programs = listSoftwarePackages();

  return (
    <main className="min-h-screen bg-[linear-gradient(180deg,#07111f_0%,#08172a_50%,#040913_100%)] text-slate-100">
      <section className="mx-auto flex w-full max-w-6xl flex-col gap-6 px-6 py-10 md:px-10 md:py-14">
        <header className="rounded-[24px] border border-white/10 bg-slate-950/45 p-7">
          <p className="text-xs uppercase tracking-[0.28em] text-sky-300/75">Board</p>
          <h1 className="mt-2 text-4xl font-semibold text-white">Program Board</h1>
          <p className="mt-3 max-w-3xl text-sm leading-7 text-slate-300">
            Public catalog of currently known software packages and text-first utilities for OpenRhiza.
          </p>
        </header>

        <div className="overflow-hidden rounded-[24px] border border-white/10 bg-slate-950/45">
          <div className="grid grid-cols-[1.6fr_0.7fr_0.8fr_0.7fr] gap-4 border-b border-white/10 px-6 py-4 text-xs uppercase tracking-[0.22em] text-slate-400">
            <div>Package</div>
            <div>Category</div>
            <div>Delivery</div>
            <div>Status</div>
          </div>
          {programs.map((program: (typeof programs)[number]) => (
            <div key={program.package_id} className="grid grid-cols-[1.6fr_0.7fr_0.8fr_0.7fr] gap-4 border-b border-white/8 px-6 py-5 last:border-b-0">
              <div>
                <div className="text-lg font-semibold text-white">{program.display_name}</div>
                <div className="mt-1 font-mono text-xs text-sky-200">{program.package_id}</div>
                <p className="mt-3 max-w-2xl text-sm leading-7 text-slate-300">{program.summary}</p>
                <div className="mt-3">
                  <Link href={`/software/${program.package_id}`} className="text-sm text-sky-200 underline underline-offset-4">
                    Open detail
                  </Link>
                </div>
              </div>
              <div className="text-sm text-slate-300">{program.category}</div>
              <div className="text-sm text-slate-300">{program.delivery}</div>
              <div className="text-sm text-slate-300">{program.status}</div>
            </div>
          ))}
        </div>
      </section>
    </main>
  );
}
