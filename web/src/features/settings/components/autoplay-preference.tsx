import { useAutoplay } from "@/features/settings/data/use-autoplay";
import { useSetAutoplay } from "@/features/settings/data/use-set-autoplay";
import { refusal } from "@/shared/lib/forms";

/** Whether the next Episode plays by itself for the signed-in User: a switch that saves as it's toggled. */
export function AutoplayPreference() {
  const { data } = useAutoplay();
  const set = useSetAutoplay();
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
