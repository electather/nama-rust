import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { librariesKey } from "@/features/admin-libraries/data/use-library";
import { LibraryService } from "@/gen/nama/v1/library_pb";

/** Creates a Library, then refetches the Libraries. */
export function useCreateLibrary() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(LibraryService.method.createLibrary, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: librariesKey(transport) });
    },
  });
}
