FROM rust:1.95.0-bookworm@sha256:6258907abe69656e41cd992e0b705cdcfabcbbe3db374f92ed2d47121282d4a1 AS builder

ARG TARGETPLATFORM
RUN test "$TARGETPLATFORM" = "linux/amd64"

WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --locked -p polygon-nesting-cli

FROM debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818

ARG ENGINE_VERSION
ARG SOURCE_COMMIT
RUN test -n "$ENGINE_VERSION" \
    && test "$ENGINE_VERSION" = "0.1.0" \
    && test -n "$SOURCE_COMMIT" \
    && test "$SOURCE_COMMIT" != "unknown"
LABEL org.opencontainers.image.title="polygon-nesting" \
      org.opencontainers.image.source="https://github.com/jfet97/polygon-nesting" \
      org.opencontainers.image.version="${ENGINE_VERSION}" \
      org.opencontainers.image.revision="${SOURCE_COMMIT}" \
      org.opencontainers.image.licenses="NOASSERTION"

RUN groupadd --gid 10001 polygon && useradd --uid 10001 --gid polygon --create-home polygon
COPY --from=builder /workspace/target/release/polygon-nesting /usr/local/bin/polygon-nesting
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY NOTICE /usr/share/doc/polygon-nesting/NOTICE
COPY LICENSES/clipper2-ts-BSL-1.0.txt /usr/share/doc/polygon-nesting/LICENSES/clipper2-ts-BSL-1.0.txt

USER polygon
ENTRYPOINT ["/usr/local/bin/polygon-nesting"]
