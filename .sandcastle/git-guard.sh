#!/bin/sh
# Installed as /usr/local/bin/git in the factory image, ahead of /usr/bin/git. Each sandbox is a worktree of the
# host repo, and git keeps refs/stash in the shared common dir, so concurrent stashes pop each other's work (#750).
# Sandcastle itself never stashes in a sandbox.
skip=
for arg do
  if [ -n "$skip" ]; then skip=; continue; fi
  case $arg in
    # Global options whose value is the next argument; their `--opt=value` forms fall through to -*.
    -C | -c | --git-dir | --work-tree | --namespace | --config-env) skip=1 ;;
    -*) ;;
    stash)
      echo "git stash is refused in factory sandboxes: refs/stash is shared by every sandbox, so a pop can take another ticket's work (#750). Commit your WIP instead, or save it with \`git diff HEAD > /tmp/wip.patch\` and restore it later with \`git apply /tmp/wip.patch\`." >&2
      exit 1
      ;;
    *) break ;;
  esac
done
exec /usr/bin/git "$@"
