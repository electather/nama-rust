import { Button } from "@/shared/ui/button";

/** Says the Device is signed in, and offers to sign in another. */
export function Approved({ deviceName, onDone }: { deviceName: string; onDone: () => void }) {
  return (
    <>
      <p className="text-muted-foreground">
        <strong className="font-medium text-foreground">{deviceName}</strong> is signed in as you.
        It may take a few seconds to notice.
      </p>
      <div className="-ms-4 mt-6">
        <Button variant="ghost" onClick={onDone}>
          Sign in another Device
        </Button>
      </div>
    </>
  );
}
