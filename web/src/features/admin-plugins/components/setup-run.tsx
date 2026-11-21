import type { SetupRun as Run } from "@/features/admin-plugins/lib/setup-markdown";
import { ExternalTextLink } from "@/shared/components/external-text-link";

/** One run of a Plugin's setup steps, its text only ever a text node. */
export function SetupRun({ run }: { run: Run }) {
  if (run.kind === "link") {
    return <ExternalTextLink href={run.href}>{run.text}</ExternalTextLink>;
  }
  if (run.kind === "strong") {
    return <strong>{run.text}</strong>;
  }
  if (run.kind === "code") {
    return <code className="rounded-sm bg-muted px-1 font-sans text-sm">{run.text}</code>;
  }
  return run.text;
}
