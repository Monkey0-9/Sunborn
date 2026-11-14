import type { Metadata } from "next";
import { Playground } from "./playground";

export const metadata: Metadata = {
  title: "Playground · Sunborn",
  description: "Send prompts through Sunborn's cache and watch it decide: exact hit, semantic hit, or refuse an unsafe reuse.",
};

export default function PlaygroundPage() {
  return (
    <div className="pt-12">
      <header className="mb-8 max-w-3xl">
        <h1 className="text-3xl font-semibold tracking-tight text-white sm:text-4xl">Playground</h1>
        <p className="mt-3 text-slate-400">
          Send a prompt, then send it again in different words. Watch where the answer comes from, how long it takes and
          what it costs, and see Sunborn refuse to reuse an answer when the meaning changed.
        </p>
      </header>
      <Playground />
    </div>
  );
}
