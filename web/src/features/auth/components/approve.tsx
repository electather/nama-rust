import { Approved } from "@/features/auth/components/approved";
import { useApproveSignInCode } from "@/features/auth/data/use-approve-sign-in-code";
import { codeErrors } from "@/features/auth/lib/sign-in-errors";
import { Button } from "@/shared/ui/button";

/** Names the Device asking to sign in, so it's signed in only once the User has seen which. */
export function Approve({
  code,
  deviceName,
  onDone,
}: {
  code: string;
  deviceName: string;
  /** Goes back to entering a code. */
  onDone: () => void;
}) {
  const approve = useApproveSignInCode();
  const errors = approve.error ? codeErrors(approve.error) : {};
  if (approve.isSuccess) {
    return <Approved deviceName={deviceName} onDone={onDone} />;
  }

  return (
    <>
      <p className="text-muted-foreground">
        <strong className="font-medium text-foreground">{deviceName}</strong> is asking to sign in
        as you. Sign it in only if it’s yours.
      </p>
      {/* Only one of the two is ever set. */}
      <p role="alert" className="mt-4 text-sm text-destructive empty:hidden">
        {errors.signInCode}
        {errors.form}
      </p>
      <div className="mt-6 flex gap-2">
        <Button
          disabled={approve.isPending}
          onClick={() => {
            approve.mutate({ code });
          }}
        >
          {approve.isPending ? "Signing it in…" : "Sign it in"}
        </Button>
        <Button variant="ghost" onClick={onDone}>
          Cancel
        </Button>
      </div>
    </>
  );
}
