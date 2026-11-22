import { ServerLinkState } from "@/features/settings/components/server-link-state";
import type { ShownLink } from "@/features/settings/lib/server-link";

/**
 * Where the Server's or a User's Link stands, the button that changes it, and why a change failed.
 * Starting clears what Unlinking said, and the other way round.
 */
export function LinkControls({
  link,
  enterUnder,
  start,
  unlink,
  failure,
}: {
  link: ShownLink;
  /** The Hub's form the code goes in, such as “Link a Server”. */
  enterUnder: string;
  /** Why the last change failed, if it did. */
  failure: string | undefined;
} & Record<
  // Starting a Link and Unlinking, each a mutation of an empty request.
  "start" | "unlink",
  { isPending: boolean; reset: () => void; mutate: (request: Record<string, never>) => void }
>) {
  return (
    <>
      <ServerLinkState
        link={link}
        enterUnder={enterUnder}
        pending={start.isPending || unlink.isPending}
        onStart={() => {
          unlink.reset();
          start.mutate({});
        }}
        onUnlink={() => {
          start.reset();
          unlink.mutate({});
        }}
      />
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {failure}
      </p>
    </>
  );
}
