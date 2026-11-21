import { listed } from "@/features/title/lib/listed";
import type { Role } from "@/gen/nama/v1/request_pb";

/** The names of those of `crew` who did `job`, such as "Director", joined by `listed`. */
export function crewWithJob(crew: Role[], job: "Director" | "Creator") {
  return listed(crew.filter((role) => role.part === job).map((role) => role.name));
}
