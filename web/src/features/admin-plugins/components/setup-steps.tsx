import { SetupRuns } from "@/features/admin-plugins/components/setup-runs";
import { parseSetupMarkdown } from "@/features/admin-plugins/lib/setup-markdown";

/** A Plugin's setup steps, from the Markdown its author wrote, as data and never as HTML. */
export function SetupSteps({ markdown }: { markdown: string }) {
  return parseSetupMarkdown(markdown).map(({ kind, items }, index) => {
    if (kind === "paragraph") {
      return (
        // oxlint-disable-next-line react/no-array-index-key -- blocks may repeat, and the list is replaced whole, never reordered
        <p key={index} className="mt-3">
          <SetupRuns runs={items[0] ?? []} />
        </p>
      );
    }
    const List = kind === "steps" ? "ol" : "ul";
    const className = kind === "steps" ? "mt-3 list-decimal ps-6" : "mt-3 list-disc ps-6";
    return (
      // oxlint-disable-next-line react/no-array-index-key -- as above
      <List key={index} className={className}>
        {items.map((runs, item) => (
          // oxlint-disable-next-line react/no-array-index-key -- as above
          <li key={item}>
            <SetupRuns runs={runs} />
          </li>
        ))}
      </List>
    );
  });
}
