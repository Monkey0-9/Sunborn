import type { Metadata } from "next";
import { Playground } from "./playground";

export const metadata: Metadata = {
  title: "Playground · Sunborn",
  description: "Send prompts through Sunborn's cache and watch it decide: exact hit, semantic hit, or refuse an unsafe reuse.",
};

export default function PlaygroundPage() {
  return <Playground />;
}
