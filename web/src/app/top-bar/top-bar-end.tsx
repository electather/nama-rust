import { SearchTrigger } from "@/app/top-bar/search-trigger";
import { UserMenu } from "@/app/top-bar/user-menu";

/** Search and the signed-in User's menu. */
export function TopBarEnd({ userName, onSearch }: { userName: string; onSearch: () => void }) {
  return (
    <div className="flex min-w-0 items-center gap-2.5">
      <SearchTrigger onSearch={onSearch} />
      <UserMenu userName={userName} />
    </div>
  );
}
