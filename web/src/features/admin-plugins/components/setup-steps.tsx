import { SetupRuns } from "@/features/admin-plugins/components/setup-runs";
import { parseSetupMarkdown } from "@/features/admin-plugins/lib/setup-markdown";

const lists = {
  bullets: { List: "ul", className: "mt-3 list-disc ps-6" },
  steps: { List: "ol", className: "mt-3 list-decimal ps-6" },
} as const;

/**
 * A Plugin's setup steps, from the Markdown its author wrote, as data and never as HTML
 * (`src/features/admin-plugins/AGENTS.md`).
 */
export function SetupSteps({ markdown }: { markdown: string }) {
  return parseSetupMarkdown(markdown).map((block, index) => {
    if (block.kind === "paragraph") {
      return (
        // oxlint-disable-next-line react/no-array-index-key -- blocks may repeat, and the list is replaced whole, never reordered
        <p key={index} className="mt-3">
          <SetupRuns runs={block.runs} />
        </p>
      );
    }
    const { List, className } = lists[block.kind];
    return (
      // oxlint-disable-next-line react/no-array-index-key -- blocks may repeat, and the list is replaced whole, never reordered
      <List key={index} className={className}>
        {block.items.map((runs, item) => (
          // oxlint-disable-next-line react/no-array-index-key -- items may repeat, and the list is replaced whole, never reordered
          <li key={item}>
            <SetupRuns runs={runs} />
          </li>
        ))}
      </List>
    );
  });
}
