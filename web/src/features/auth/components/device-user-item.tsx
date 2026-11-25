import type { User } from "@/gen/nama/v1/sign_in_pb";

/** One User in the User picker, by their initial and name, marked when the Device acts as them. */
export function DeviceUserItem({
  user,
  current,
  disabled,
  onPick,
}: {
  user: User;
  current: boolean;
  disabled: boolean;
  onPick: () => void;
}) {
  return (
    <li>
      <button
        type="button"
        aria-current={current}
        disabled={disabled}
        onClick={onPick}
        className="flex h-14 w-full items-center gap-3.5 rounded-lg border border-input bg-input/30 px-3.5 text-start text-base font-medium outline-none hover:bg-input/50 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:bg-muted disabled:text-muted-foreground aria-current:border-primary"
      >
        <span
          aria-hidden
          className="grid size-9 place-items-center rounded-full bg-muted text-sm font-semibold"
        >
          {user.name.slice(0, 1).toUpperCase()}
        </span>
        {user.name}
      </button>
    </li>
  );
}
