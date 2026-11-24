import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { librariesKey } from "@/features/admin-libraries/data/use-library";
import { LibraryService } from "@/gen/nama/v1/library_pb";

/** Deletes a Library, then goes back to the list of Libraries and refetches them. */
export function useDeleteLibrary() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  const navigate = useNavigate();
  return useMutation(LibraryService.method.deleteLibrary, {
    // Away from the deleted Library first, so the refetch doesn't ask for it.
    onSuccess: async () => {
      await navigate({ to: ".", search: (rest) => ({ ...rest, library: undefined }) });
      await queryClient.invalidateQueries({ queryKey: librariesKey(transport) });
    },
  });
}
