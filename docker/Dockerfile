# CLI-only image — not ADE Desktop / Tauri / WebView.
# Build from repo root: docker build -f docker/Dockerfile -t ade-cli .
FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
# Override workspace sibling target-dir from .cargo/config.toml
ENV CARGO_TARGET_DIR=/src/target
RUN cargo build -p ade-cli --release

FROM debian:bookworm-slim
COPY --from=build /src/target/release/ade /usr/local/bin/ade
ENTRYPOINT ["ade"]
CMD ["--help"]
