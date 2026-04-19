import { listWorkflows } from "@/app/registry-data";

export const dynamic = "force-dynamic";

export default function WorkflowsPage() {
  const workflows = listWorkflows();
  return (
    <main className="min-h-screen bg-[linear-gradient(180deg,#07111f_0%,#08172a_50%,#040913_100%)] text-slate-100">
      <section className="mx-auto flex w-full max-w-6xl flex-col gap-6 px-6 py-10 md:px-10 md:py-14">
        <header className="rounded-[24px] border border-white/10 bg-slate-950/45 p-7">
          <p className="text-xs uppercase tracking-[0.28em] text-sky-300/75">Capability Board</p>
          <h1 className="mt-2 text-4xl font-semibold text-white">Workflow Board</h1>
          <p className="mt-3 max-w-3xl text-sm leading-7 text-slate-300">Reusable multi-step procedures for driver acquisition, program setup, and skill orchestration.</p>
        </header>
        <div className="grid gap-4">
          {workflows.map((workflow) => (
            <article key={workflow.workflow_id} className="rounded-[24px] border border-white/10 bg-slate-950/45 p-6">
              <h2 className="text-2xl font-semibold text-white">{workflow.display_name}</h2>
              <p className="mt-2 font-mono text-sm text-sky-200">{workflow.workflow_id}</p>
              <p className="mt-3 text-sm leading-7 text-slate-300">{workflow.summary}</p>
              <ol className="mt-4 list-decimal space-y-1 pl-5 text-sm text-slate-300">
                {workflow.steps.map((step) => <li key={step}>{step}</li>)}
              </ol>
            </article>
          ))}
        </div>
      </section>
    </main>
  );
}
