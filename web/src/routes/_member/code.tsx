import { Code, type ConnectError } from "@connectrpc/connect";
import { useMutation } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { FormEnd, FormField } from "@/components/form-field";
import { Page } from "@/components/page";
import { Button } from "@/components/ui/button";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { type FormErrors, formText, onSubmitted, refusal } from "@/lib/forms";

export const Route = createFileRoute("/_member/code")({ component: EnterCode });

/** Signing in another Device: look up the Sign-in Code it shows, check its name, then approve. */
function EnterCode() {
  const lookUp = useMutation(SignInService.method.lookUpSignInCode);
  return (
    <Page heading="Sign in a Device">
      <section className="mt-2 max-w-sm">
        {lookUp.data === undefined ? (
          <LookUp
            onSubmit={(code) => {
              lookUp.mutate({ code });
            }}
            pending={lookUp.isPending}
            error={lookUp.error}
          />
        ) : (
          <Approve
            code={lookUp.variables?.code ?? ""}
            deviceName={lookUp.data.deviceName}
            onDone={lookUp.reset}
          />
        )}
      </section>
    </Page>
  );
}

function LookUp({
  onSubmit,
  pending,
  error,
}: {
  onSubmit: (code: string) => void;
  pending: boolean;
  error: ConnectError | null;
}) {
  const errors = error === null ? {} : codeErrors(error);
  return (
    <>
      <p className="text-muted-foreground">
        Enter the Sign-in Code the other Device shows to sign it in as you.
      </p>
      <form
        className="mt-6 flex flex-col gap-5"
        onSubmit={onSubmitted((typed) => {
          onSubmit(formText(typed, "code"));
        })}
      >
        <FormField
          label="Sign-in Code"
          name="code"
          placeholder="XXXX-XXXX"
          autoComplete="off"
          autoCapitalize="characters"
          spellCheck={false}
          error={errors.signInCode}
        />
        <FormEnd
          action={pending ? "Looking it up…" : "Continue"}
          pending={pending}
          done={undefined}
          refusal={errors.form}
        />
      </form>
    </>
  );
}

/** Names the Device asking to sign in, so it's signed in only once the User has seen which. */
function Approve({
  code,
  deviceName,
  onDone,
}: {
  code: string;
  deviceName: string;
  /** Goes back to entering a code. */
  onDone: () => void;
}) {
  const approve = useMutation(SignInService.method.approveSignInCode);
  const errors = approve.error ? codeErrors(approve.error) : {};
  if (approve.isSuccess) return <Approved deviceName={deviceName} onDone={onDone} />;

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

function Approved({ deviceName, onDone }: { deviceName: string; onDone: () => void }) {
  return (
    <>
      <p className="text-muted-foreground">
        <strong className="font-medium text-foreground">{deviceName}</strong> is signed in as you.
        It may take a few seconds to notice.
      </p>
      <div className="mt-6 -ms-4">
        <Button variant="ghost" onClick={onDone}>
          Sign in another Device
        </Button>
      </div>
    </>
  );
}

function codeErrors(error: unknown): FormErrors {
  return refusal(error, "check that code", {
    [Code.NotFound]: {
      signInCode:
        "That code doesn’t match a Device waiting to sign in. Check it, or get a new code on the Device.",
    },
    [Code.FailedPrecondition]: { signInCode: "That code has already been used." },
  });
}
