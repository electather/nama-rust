FROM node:26-bookworm

RUN apt-get update && apt-get install -y \
  git curl jq build-essential pkg-config libssl-dev protobuf-compiler \
  && rm -rf /var/lib/apt/lists/*

# GitHub CLI
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
  | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
  && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
  > /etc/apt/sources.list.d/github-cli.list \
  && apt-get update && apt-get install -y gh \
  && rm -rf /var/lib/apt/lists/*

# buf (proto lint/breaking/codegen, ADR 0007)
RUN curl -fsSL "https://github.com/bufbuild/buf/releases/latest/download/buf-Linux-$(uname -m)" -o /usr/local/bin/buf \
  && chmod +x /usr/local/bin/buf

# Headless Chromium, so a UI agent can screenshot its own screens (`playwright screenshot`).
# As root: --with-deps apt-installs the browser's libraries.
ENV PLAYWRIGHT_BROWSERS_PATH=/ms-playwright
RUN npm install -g playwright && playwright install --with-deps chromium

# sandcastle docker build-image passes the host UID/GID so bind-mounted files share an owner
ARG AGENT_UID=1000
ARG AGENT_GID=1000
RUN groupmod -o -g $AGENT_GID node && usermod -o -u $AGENT_UID -g $AGENT_GID -d /home/agent -m -l agent node
USER ${AGENT_UID}:${AGENT_GID}

# Rust toolchain (wasm32 target for Extism runtime plugins, ADR 0006)
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal \
  --component clippy,rustfmt --target wasm32-unknown-unknown
# Cache dirs exist up front so the host cache mounts land on agent-owned dirs
RUN mkdir -p /home/agent/.cargo/registry /home/agent/.local/share/pnpm/store /home/agent/.claude

RUN curl -fsSL https://claude.ai/install.sh | bash

# Vite+ (`vp`, ADR 0017) with the Node 26 and pnpm 12 that web/ pins; vp fetches an exact pin it lacks on first use.
# ponytail: the build context is .sandcastle/, which can't see web/, so the majors are copied here and must follow
# a bump in web/. Build with the repo root as context and run `vp env install` in web/ if that drifts.
RUN curl -fsSL https://vite.plus | bash
ENV PATH="/home/agent/.local/share/vite-plus/bin:/home/agent/.local/share/vite-plus/fallback-bin:$PATH"
RUN vp env default node@26 && vp env default pnpm@12 && vp env install node@26 pnpm@12

ENV PATH="/home/agent/.local/bin:/home/agent/.cargo/bin:$PATH"

# Sandcastle bind-mounts the worktree at /home/agent/workspace
WORKDIR /home/agent
ENTRYPOINT ["sleep", "infinity"]
