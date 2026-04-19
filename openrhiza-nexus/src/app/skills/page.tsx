import { listSkills } from "@/app/registry-data";

export const dynamic = "force-dynamic";

export default function SkillsPage() {
  const skills = listSkills();
  return (
    <main className="min-h-screen bg-[linear-gradient(180deg,#07111f_0%,#08172a_50%,#040913_100%)] text-slate-100">
      <section className="mx-auto flex w-full max-w-6xl flex-col gap-6 px-6 py-10 md:px-10 md:py-14">
        <header className="rounded-[24px] border border-white/10 bg-slate-950/45 p-7">
          <p className="text-xs uppercase tracking-[0.28em] text-sky-300/75">Capability Board</p>
          <h1 className="mt-2 text-4xl font-semibold text-white">Skill Board</h1>
          <p className="mt-3 max-w-3xl text-sm leading-7 text-slate-300">LLM-facing unit abilities such as web search, sandbox execution, registry lookup, and validation helpers.</p>
        </header>
        <div className="grid gap-4">
          {skills.map((skill) => (
            <article key={skill.skill_id} className="rounded-[24px] border border-white/10 bg-slate-950/45 p-6">
              <div className="flex items-start justify-between gap-4">
                <div>
                  <h2 className="text-2xl font-semibold text-white">{skill.display_name}</h2>
                  <p className="mt-2 font-mono text-sm text-sky-200">{skill.skill_id}</p>
                  <p className="mt-3 text-sm leading-7 text-slate-300">{skill.summary}</p>
                </div>
                <div className="rounded-full border border-sky-300/20 px-3 py-1 text-xs uppercase tracking-[0.24em] text-sky-100">{skill.status}</div>
              </div>
              <div className="mt-4 text-sm text-slate-400">Category: {skill.category} · Delivery: {skill.delivery}</div>
            </article>
          ))}
        </div>
      </section>
    </main>
  );
}
