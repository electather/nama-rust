import type { RequestToCancel } from "@/gen/nama/v1/library_pb";
import { seasonName } from "@/shared/lib/title-facts";

/** The open Requests still waiting on what goes, which deleting it cancels. */
export function Cancelled({ requests }: { requests: RequestToCancel[] }) {
  if (requests.length === 0) {
    return null;
  }
  return (
    <div>
      <p>
        It cancels {requests.length === 1 ? "this open Request" : "these open Requests"}, telling
        whoever asked that you deleted it:
      </p>
      <ul className="mt-2 flex flex-col gap-1 text-muted-foreground">
        {requests.map(({ title, seasonNumber }) => {
          const name = seasonNumber === undefined ? title : `${title}, ${seasonName(seasonNumber)}`;
          return <li key={name}>{name}</li>;
        })}
      </ul>
    </div>
  );
}
