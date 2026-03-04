"use client";

import { CopilotChat } from "@copilotkit/react-core/v2";

export default function CopilotKitPage() {
  return (
    <main className="h-screen w-screen grid grid-rows-[auto_1fr] bg-slate-950 text-slate-100">
      <header className="border-b border-slate-800 px-6 py-4">
        <h1 className="text-xl font-semibold">Quarry + CopilotKit Demo</h1>
        <p className="text-sm text-slate-400 mt-1">
          Ask analytics and context questions. Examples: &quot;revenue by region for tenant_123&quot;,
          &quot;explain the revenue query&quot;, &quot;search context for playbook&quot;.
        </p>
      </header>
      <section className="flex justify-center items-stretch p-4">
        <CopilotChat className="w-full max-w-5xl h-full" />
      </section>
    </main>
  );
}
