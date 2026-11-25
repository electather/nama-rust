import { createFileRoute } from "@tanstack/react-router";

import { UserPicker } from "@/features/auth/components/user-picker";

export const Route = createFileRoute("/_signed-out/switch-user/")({
  component: UserPickerRoute,
});

function UserPickerRoute() {
  const { redirect } = Route.useSearch();
  const { signedIn } = Route.useRouteContext();
  return (
    <UserPicker users={signedIn.deviceUsers} current={signedIn.user?.id} redirect={redirect} />
  );
}
