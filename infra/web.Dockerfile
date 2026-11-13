# Next.js site. Single stage: next.config.ts has no `output: "standalone"`, so
# `next start` needs node_modules, and pnpm's symlinked store does not survive a
# naive copy between stages.
FROM node:22-bookworm-slim
ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0 \
    NEXT_TELEMETRY_DISABLED=1
RUN corepack enable
WORKDIR /src
COPY . .
RUN pnpm install --frozen-lockfile

# Read at BUILD time by next.config.ts (CSP connect-src) and inlined into the
# client bundle for every NEXT_PUBLIC_* name, so they must be present here and
# not only at runtime.
ARG LIGHTWALLETD_GRPCWEB
ARG NEXT_PUBLIC_DEFAULT_NETWORK=testnet
ENV LIGHTWALLETD_GRPCWEB=${LIGHTWALLETD_GRPCWEB} \
    NEXT_PUBLIC_DEFAULT_NETWORK=${NEXT_PUBLIC_DEFAULT_NETWORK} \
    NODE_ENV=production
RUN pnpm --filter @sealforge/web build

RUN chown -R node:node /src
USER node
EXPOSE 3000
CMD ["pnpm", "--filter", "@sealforge/web", "exec", "next", "start", "-p", "3000", "-H", "0.0.0.0"]
