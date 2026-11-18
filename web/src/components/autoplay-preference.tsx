import { createConnectQueryKey, useMutation, useQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { PlaybackService } from "@/gen/nama/v1/playback_pb";
import { refusal } from "@/shared/lib/forms";

const autoplayKey = createConnectQueryKey({
  schema: PlaybackService.method.getAutoplay,
  cardinality: "finite",
});

/** Whether the next Episode plays by itself for the signed-in User: a switch that saves as it's toggled. */
export function AutoplayPreference() {
  const { data } = useQuery(PlaybackService.method.getAutoplay, {});
  const queryClient = useQueryClient();
  const set = useMutation(PlaybackService.method.setAutoplay, {
    onSettled: () => queryClient.invalidateQueries({ queryKey: autoplayKey }),
  });
  if (!data) {
    return null;
  }
  // Shows the choice being saved until the Server's comes back with it.
  const on = set.isPending ? set.variables.on : data.on;
  return (
    <div className="mt-8">
      <label className="flex items-center gap-3 text-sm">
        <input
          type="checkbox"
          role="switch"
          checked={on === true}
          aria-checked={on === true}
          onChange={(event) => {
            set.mutate({ on: event.target.checked });
          }}
          className="size-4 shrink-0 accent-foreground"
        />
        Play the next Episode automatically
      </label>
      {set.error && (
        <p role="alert" className="mt-1 text-sm text-destructive">
          {refusal(set.error, "save that", {}).form}
        </p>
      )}
    </div>
  );
}
