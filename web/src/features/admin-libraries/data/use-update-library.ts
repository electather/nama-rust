import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { librariesKey } from "@/features/admin-libraries/data/use-library";
import { LibraryService } from "@/gen/nama/v1/library_pb";

/** Renames a Library or changes its folders, title sources or Recognisers, then refetches it. */
export function useUpdateLibrary() {
  const queryClient = useQueryClient();
  return useMutation(LibraryService.method.updateLibrary, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: librariesKey });
    },
  });
}
