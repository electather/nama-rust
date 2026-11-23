import { ProgressRing } from "nama-ui/components/progress";
import { Tooltip, TooltipContent, TooltipTrigger } from "nama-ui/components/tooltip";

import type { Download } from "@/gen/nama/v1/request_pb";
import { useModalContainer } from "@/shared/hooks/use-modal-container";
import { overallPercent } from "@/shared/lib/download-progress";

/** How far a Request's `downloads` have got, as a ring in its state's place, the percentage on hover and on focus. */
export function DownloadRing({ downloads }: { downloads: Download[] }) {
  const overall = Math.floor(overallPercent(downloads));
  const [trigger, container] = useModalContainer();
  return (
    <Tooltip>
      {/* Not a button, as pressing it does nothing; focusable so a keyboard opens its tooltip. */}
      {/* oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- see above */}
      <TooltipTrigger render={<span ref={trigger} tabIndex={0} />}>
        <ProgressRing value={overall} aria-label="Downloaded" />
      </TooltipTrigger>
      <TooltipContent container={container}>{overall}% downloaded</TooltipContent>
    </Tooltip>
  );
}
