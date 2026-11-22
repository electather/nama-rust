import { cn } from "cn";
import { Tooltip, TooltipContent, TooltipTrigger } from "nama-ui/components/tooltip";

import { outcomes } from "@/features/admin-jobs/lib/outcomes";
import { JobRunOutcome } from "@/gen/nama/v1/job_pb";
import { useModalContainer } from "@/shared/hooks/use-modal-container";

// Red only for Failed, as DESIGN.md keeps it the only status colour; a Run going turns.
const LOOKS: Partial<Record<JobRunOutcome, string>> = {
  [JobRunOutcome.FAILED]: "text-destructive",
  [JobRunOutcome.RUNNING]: "animate-spin text-muted-foreground motion-reduce:animate-none",
};

/** How a Run ended, or that it's going, as its icon alone, named on hover and on focus. */
export function OutcomeIcon({ outcome }: { outcome: JobRunOutcome }) {
  const [trigger, container] = useModalContainer();
  const shown = outcomes[outcome];
  if (!shown) {
    return null;
  }
  const { label, Icon } = shown;
  return (
    <Tooltip>
      {/* Not a button, as pressing it does nothing; focusable so a keyboard opens its tooltip. */}
      {/* oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- see above */}
      <TooltipTrigger render={<span ref={trigger} tabIndex={0} />}>
        <Icon
          aria-hidden
          strokeWidth={1.5}
          className={cn("block size-4", LOOKS[outcome] ?? "text-muted-foreground")}
        />
        <span className="sr-only">{label}</span>
      </TooltipTrigger>
      <TooltipContent container={container}>{label}</TooltipContent>
    </Tooltip>
  );
}
