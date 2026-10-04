# syntax=docker/dockerfile:1
#
# ma serve in a container. Build from the repository root:
#
#   docker build -f apps/cli/Dockerfile -t modular-agent .
#   docker run -e MA_TOKEN=... -p 8080:8080 -v ./my-workspace:/workspace modular-agent
#
# The workspace (settings.json, patches/, data/) is a volume; secrets come in as
# environment variables referenced from settings.json as ${VAR}. Listening on
# 0.0.0.0 requires MA_TOKEN, the bearer token for /mcp.

FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .

# Out-of-tree module crates the selection below links, cloned the same way as
# for a local build (see custom_modules/README.md).
ARG MODULE_REPOS="modular-agent-slack modular-agent-web"
ARG MODULE_REF=main
RUN for repo in $MODULE_REPOS; do \
        git clone --depth 1 --branch "$MODULE_REF" \
            "https://github.com/modular-agent/$repo.git" "custom_modules/$repo"; \
    done

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/tools/ma-config/target \
    cargo run --manifest-path tools/ma-config/Cargo.toml -- \
        cli --apply --config apps/cli/ma-config.cloud.toml

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release -p modular-agent-cli \
    && cp target/release/ma /usr/local/bin/ma

FROM debian:bookworm-slim
# TLS is rustls throughout, reading the system trust store.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN useradd --system --uid 10001 --create-home ma \
    && mkdir /workspace \
    && chown ma:ma /workspace
COPY --from=build /usr/local/bin/ma /usr/local/bin/ma

USER ma
VOLUME /workspace
EXPOSE 8080
ENTRYPOINT ["ma"]
CMD ["--log-format", "json", "serve", "--workspace", "/workspace", "--listen", "0.0.0.0:8080"]
