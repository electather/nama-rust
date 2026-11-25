import { Link, useLocation, useNavigate } from "@tanstack/react-router";
import { Button, buttonVariants } from "nama-ui/components/button";

import { DeviceUserItem } from "@/features/auth/components/device-user-item";
import { useFinishSignIn } from "@/features/auth/data/use-finish-sign-in";
import { useSignOutDevice } from "@/features/auth/data/use-sign-out-device";
import { useSwitchUser } from "@/features/auth/data/use-switch-user";
import { needsSwitchPin, switchErrors } from "@/features/auth/lib/switch-errors";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { FormNotices } from "@/shared/components/sign-in-layout/form-notices";
import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";

/**
 * The User picker: the `users` signed in to this Device, the one it acts as `current`, to switch
 * to, then go on to `redirect`; signing in another; and signing them all out, which a shared
 * Device acting as none of them has no other way to do.
 */
export function UserPicker({
  users,
  current,
  redirect,
}: {
  users: User[];
  current: bigint | undefined;
  redirect: string | undefined;
}) {
  const navigate = useNavigate();
  const here = useLocation({ select: (location) => location.href });
  const finish = useFinishSignIn(redirect);
  const switchUser = useSwitchUser(finish);
  const signOut = useSignOutDevice();
  const pick = (user: User) => {
    if (user.id === current) {
      void navigate({ href: redirect ?? "/" });
      return;
    }
    switchUser.mutate(
      { userId: user.id },
      {
        onError: (error) => {
          if (needsSwitchPin(error)) {
            const userId = String(user.id);
            void navigate({ to: "/switch-user/$userId", params: { userId }, search: { redirect } });
          }
        },
      },
    );
  };
  return (
    <SignInCard heading="Who’s watching?">
      <ul aria-label="Users on this Device" className="flex flex-col gap-2.5">
        {users.map((user) => (
          <DeviceUserItem
            key={String(user.id)}
            user={user}
            current={user.id === current}
            disabled={switchUser.isPending}
            onPick={() => {
              pick(user);
            }}
          />
        ))}
      </ul>
      <FormNotices errors={switchUser.error ? switchErrors(switchUser.error) : {}} />
      {/* Signing in on its way back here adds a User to this Device (`skipsSignIn`). */}
      <Link
        to="/sign-in"
        search={{ redirect: here }}
        className={buttonVariants({ variant: "outline", size: "lg" })}
      >
        Add another User
      </Link>
      <p className="text-center text-sm">
        <Button
          variant="link"
          size="link"
          disabled={signOut.isPending}
          onClick={() => {
            signOut.mutate({});
          }}
        >
          Sign everyone out
        </Button>
      </p>
    </SignInCard>
  );
}
