import { Button } from "nama-ui/components/button";

import { QualityMenu } from "@/features/requests/components/quality-menu";
import type { QualityTarget } from "@/features/requests/lib/quality-target";

/** A button that changes a Request at once, and says why if the Server refuses. */
export function ChangeButton({
  action,
  name,
  change,
  id,
  refusal: whyRefused,
  quality,
}: {
  /** What the change is called, such as "Approve", which the button says. */
  action: string;
  /** The button's accessible name, which says which Request it changes. */
  name: string;
  /** The mutation the button calls with the Request's ID, and the menu with a Quality's too. */
  change: {
    mutate: (input: { id: bigint; qualityId?: bigint }) => void;
    isPending: boolean;
    isError: boolean;
    error: unknown;
  };
  /** The ID of the Request it changes. */
  id: bigint;
  /** Why the Server refused the change, in words, from its error. */
  refusal: (error: unknown) => string | undefined;
  /** The Request's Quality, if making the change can also change it. */
  quality?: QualityTarget | undefined;
}) {
  return (
    <>
      <Button
        variant="ghost"
        aria-label={name}
        disabled={change.isPending}
        onClick={() => {
          change.mutate({ id });
        }}
      >
        {action}
      </Button>
      {quality && (
        <QualityMenu action={action} name={name} quality={quality} change={change} id={id} />
      )}
      {change.isError && (
        <p role="alert" className="w-full ps-4 text-sm text-destructive">
          {whyRefused(change.error)}
        </p>
      )}
    </>
  );
}
