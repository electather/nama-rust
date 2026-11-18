import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { librariesKey } from "@/features/admin-libraries/data/use-library";
import { LibraryService } from "@/gen/nama/v1/library_pb";

/** Deletes a Library, then goes back to the Libraries and refetches them. */
export function useDeleteLibrary() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  return useMutation(LibraryService.method.deleteLibrary, {
    // Away from the deleted Library's page first, so the refetch doesn't ask for it.
    onSuccess: async () => {
      await navigate({ to: "/admin/libraries" });
      await queryClient.invalidateQueries({ queryKey: librariesKey });
    },
  });
}
