import { Button } from "@/shared/ui/button";

/** Cancels the link the User is `waiting` on, or else disconnects them from the Plugin `name`. */
export function LeaveButton({
  waiting,
  name,
  disabled,
  onClick,
}: {
  waiting: boolean;
  name: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <Button
      variant="ghost"
      aria-label={`${waiting ? "Cancel connecting" : "Disconnect"} ${name}`}
      disabled={disabled}
      onClick={onClick}
    >
      {waiting ? "Cancel" : "Disconnect"}
    </Button>
  );
}
