import { Button } from "@/shared/ui/button";

/** Forgets the Plugin `name`'s Personal settings. */
export function ForgetButton({
  name,
  disabled,
  onClick,
}: {
  name: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      aria-label={`Forget your ${name} settings`}
      disabled={disabled}
      onClick={onClick}
    >
      Forget
    </Button>
  );
}
