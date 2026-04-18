import Link from "next/link";

import {
  listDrivers,
  listEvaluations,
  listModels,
  listNodes,
  listSoftwarePackages,
} from "@/app/registry-data";

export const dynamic = "force-dynamic";

export default function Home() {
  const drivers = listDrivers();
  const software = listSoftwarePackages();
  const models = listModels();
  const nodes = listNodes();
  const evaluations = listEvaluations();

  const sections = [
    {
      href: "/drivers",
      title: "Driver Board",
      count: `${drivers.length} tracked`,
      summary: "Browse verified, testing, and proposed drivers by hardware match key and current evaluation.",
    },
    {
      href: "/software",
      title: "Program Board",
      count: `${software.length} tracked`,
      summary: "See text-first packages, diagnostic tools, and sandbox-oriented utilities for OpenRhiza nodes.",
    },
    {
      href: "/models",
      title: "LLM Board",
      count: `${models.length} tracked`,
      summary: "Review currently available and planned remote models, including future Google API integration.",
    },
    {
      href: "/nodes",
      title: "Node Board",
      count: `${nodes.length} tracked / ${evaluations.length} evals`,
      summary: "Inspect public node status, trust tier, hardware fingerprints, and the latest evaluation notes.",
    },
  ];

  const apiEndpoints = [
    "POST /api/v1/node/register",
    "POST /api/v1/node/heartbeat",
    "POST /api/v1/hardware/report",
    "POST /api/v1/driver/query",
    "POST /api/v1/software/query",
    "POST /api/v1/llm/query",
    "GET /api/v1/llm/google/models",
    "POST /api/v1/llm/generate",
    "POST /api/v1/evaluation/upload",
  ];

  return (
    <main className="min-h-screen bg-[radial-gradient(circle_at_top_left,rgba(115,195,255,0.18),transparent_28%),radial-gradient(circle_at_top_right,rgba(16,185,129,0.12),transparent_24%),linear-gradient(180deg,#07111f_0%,#08172a_50%,#040913_100%)] text-slate-100">
      <section className="mx-auto flex w-full max-w-6xl flex-col gap-8 px-6 py-10 md:px-10 md:py-14">
        <header className="rounded-[28px] border border-sky-300/15 bg-slate-950/55 p-8 shadow-[0_0_0_1px_rgba(255,255,255,0.02),0_30px_120px_rgba(0,0,0,0.35)] backdrop-blur">
          <p className="text-xs font-semibold uppercase tracking-[0.35em] text-sky-300/80">
            OpenRhiza Registry
          </p>
          <h1 className="mt-4 text-4xl font-semibold tracking-tight text-white md:text-6xl">
            OpenRhiza.com
          </h1>
          <p className="mt-4 max-w-3xl text-sm leading-7 text-slate-300 md:text-base">
            A public registry for drivers, programs, models, and node evaluations. The OS uses the API.
            People can browse the same catalog in board form.
          </p>
        </header>

        <section className="grid gap-5 md:grid-cols-2">
          {sections.map((section) => (
            <Link
              key={section.href}
              href={section.href}
              className="group rounded-[24px] border border-white/10 bg-slate-950/45 p-7 transition hover:border-sky-300/30 hover:bg-slate-900/70"
            >
              <div className="flex items-start justify-between gap-4">
                <div>
                  <div className="text-xs uppercase tracking-[0.28em] text-sky-300/75">{section.count}</div>
                  <h2 className="mt-3 text-2xl font-semibold text-white group-hover:text-sky-100">
                    {section.title}
                  </h2>
                </div>
                <div className="rounded-full border border-sky-300/20 px-3 py-1 text-xs text-sky-200">
                  Open
                </div>
              </div>
              <p className="mt-4 text-sm leading-7 text-slate-300">{section.summary}</p>
            </Link>
          ))}
        </section>

        <section className="rounded-[24px] border border-white/10 bg-slate-950/45 p-7">
          <div className="flex flex-col gap-2 md:flex-row md:items-end md:justify-between">
            <div>
              <p className="text-xs uppercase tracking-[0.28em] text-emerald-300/75">Machine Surface</p>
              <h2 className="mt-2 text-2xl font-semibold text-white">OS API Endpoints</h2>
            </div>
            <Link href="/api/health" className="text-sm text-sky-200 underline underline-offset-4">
              Health check
            </Link>
          </div>
          <div className="mt-6 grid gap-3 md:grid-cols-2">
            {apiEndpoints.map((endpoint) => (
              <div
                key={endpoint}
                className="rounded-2xl border border-sky-300/12 bg-slate-900/70 px-4 py-3 font-mono text-sm text-sky-100"
              >
                {endpoint}
              </div>
            ))}
          </div>
        </section>
      </section>
    </main>
  );
}
