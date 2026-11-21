import type { DetailGroup } from "@/features/title/lib/detail-group";
import { languageName } from "@/shared/lib/language-name";

/** The Languages fact naming the language a title was made in, such as "Japanese" for `ja`. */
export function originalAudio(language: string | undefined): DetailGroup["facts"][number] {
  return ["Original audio", languageName(language)];
}
