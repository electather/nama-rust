import { useQuery } from "@connectrpc/connect-query";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { TitleKind } from "@/gen/nama/v1/match_pb";
import { type Request, RequestService } from "@/gen/nama/v1/request_pb";

/** A Request's Quality, which approving or retrying it can change. */
export interface QualityTarget {
  /** Whose Fulfiller's Qualities it can change to: a Movie's or a Show's. */
  kind: TitleKind;
  /** The id of the Quality it has now, if any. */
  id: bigint | undefined;
}

/**
 * What approving or retrying `request` can change its Quality to, if anything: a Movie Request's
 * Fulfiller's other Qualities. A Season Request has no Quality yet.
 */
export function qualityTargetOf({ movie, quality }: Request): QualityTarget | undefined {
  return movie && { kind: TitleKind.MOVIE, id: quality?.id };
}

/** What a {@link ChangeButton} changes, and how it says the Server refused. */
export interface ChangeButtonProps {
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
  id: bigint;
  refusal: (error: unknown) => string | undefined;
  /** The Request's Quality, if making the change can also change it. */
  quality?: QualityTarget | undefined;
}

/** A button that changes a Request at once, and says why if the Server refuses. */
export function ChangeButton({
  action,
  name,
  change,
  id,
  refusal: whyRefused,
  quality,
}: ChangeButtonProps) {
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

/**
 * Makes a Request manager's `change` to a Request, such as approving it, at another of its
 * Fulfiller's Qualities, changing its Quality in the same step. Shown only when the Fulfiller
 * offers another.
 */
function QualityMenu({
  action,
  name,
  quality: { kind, id: current },
  change,
  id,
}: {
  /** What the change is called, such as "Approve". */
  action: string;
  /** The accessible name of the change's own button, such as "Approve Heat". */
  name: string;
  quality: QualityTarget;
  change: { mutate: (input: { id: bigint; qualityId: bigint }) => void; isPending: boolean };
  id: bigint;
}) {
  const { data } = useQuery(RequestService.method.listOfferedQualities, { kind });
  const others = data?.qualities.filter((quality) => quality.id !== current) ?? [];
  if (others.length === 0) return null;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            variant="ghost"
            aria-label={`${name} at another Quality`}
            disabled={change.isPending}
          />
        }
      >
        {action} at…
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        {others.map((quality) => (
          <DropdownMenuItem
            key={quality.id}
            onClick={() => {
              change.mutate({ id, qualityId: quality.id });
            }}
          >
            {quality.name}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
