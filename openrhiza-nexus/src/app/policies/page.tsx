import { listPolicies } from "@/app/registry-data";

export const dynamic = "force-dynamic";

export default function PoliciesPage() {
  const policies = listPolicies();
  return (
    <main className="min-h-screen bg-[linear-gradient(180deg,#07111f_0%,#08172a_50%,#040913_100%)] text-slate-100">
      <section className="mx-auto flex w-full max-w-6xl flex-col gap-6 px-6 py-10 md:px-10 md:py-14">
        <header className="rounded-[24px] border border-white/10 bg-slate-950/45 p-7">
          <p className="text-xs uppercase tracking-[0.28em] text-sky-300/75">Capability Board</p>
          <h1 className="mt-2 text-4xl font-semibold text-white">Policy Board</h1>
          <p className="mt-3 max-w-3xl text-sm leading-7 text-slate-300">Operational rules that define registry-first behavior, hot-swap activation, and storage safety.</p>
        </header>
        <div className="grid gap-4">
          {policies.map((policy) => (
            <article key={policy.policy_id} className="rounded-[24px] border border-white/10 bg-slate-950/45 p-6">
              <h2 className="text-2xl font-semibold text-white">{policy.policy_id}</h2>
              <p className="mt-2 text-sm text-slate-400">Scope: {policy.scope} · Status: {policy.status}</p>
              <p className="mt-3 text-sm leading-7 text-slate-300">{policy.summary}</p>
              <ul className="mt-4 list-disc space-y-1 pl-5 text-sm text-slate-300">
                {policy.rules.map((rule) => <li key={rule}>{rule}</li>)}
              </ul>
            </article>
          ))}
        </div>
      </section>
    </main>
  );
}
