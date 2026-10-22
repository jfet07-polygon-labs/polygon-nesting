ARG TARGETPLATFORM

FROM rust:1.95.0-bookworm@sha256:6258907abe69656e41cd992e0b705cdcfabcbbe3db374f92ed2d47121282d4a1 AS base

ARG TARGETPLATFORM
RUN test "$TARGETPLATFORM" = "linux/amd64"

FROM base AS ci

LABEL org.opencontainers.image.title="polygon-nesting-ci" \
      org.opencontainers.image.source="https://github.com/jfet07-polygon-labs/polygon-nesting" \
      org.opencontainers.image.version="ci-v1.0.1"

USER root
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        binutils \
        build-essential \
        ca-certificates \
        coreutils \
        curl \
        git \
        gzip \
        libc6-dev \
        libssl-dev \
        pkg-config \
        python3 \
        tar \
        xz-utils \
    && rm -rf /var/lib/apt/lists/*

RUN rustc --version | grep -Eq '^rustc 1\.95\.0 ' \
    && cargo --version | grep -Eq '^cargo 1\.95\.0 ' \
    && rustup component add --toolchain 1.95.0-x86_64-unknown-linux-gnu rustfmt clippy \
    && rustup target add --toolchain 1.95.0-x86_64-unknown-linux-gnu x86_64-unknown-linux-gnu \
    && rustfmt --version | grep -Eq '^rustfmt 1\.9\.0-stable ' \
    && cargo clippy --version | grep -Eq '^clippy 0\.1\.95 '

RUN set -eux; \
    curl --fail --location --show-error --silent --output /tmp/node-v22.23.4-linux-x64.tar.xz https://nodejs.org/dist/v22.23.4/node-v22.23.4-linux-x64.tar.xz; \
    printf '%s\n' '1805bcd35e6815559bd2eb97db438d3fdb0eb3a5e6a977b7c1badca76337e9ce  /tmp/node-v22.23.4-linux-x64.tar.xz' | sha256sum --check --strict; \
    tar -xJf /tmp/node-v22.23.4-linux-x64.tar.xz -C /opt; \
    test -x /opt/node-v22.23.4-linux-x64/bin/node; \
    mv /opt/node-v22.23.4-linux-x64 /opt/node-v22.23.4; \
    rm /tmp/node-v22.23.4-linux-x64.tar.xz

RUN set -eux; \
    curl --fail --location --show-error --silent --output /tmp/node-v24.22.1-linux-x64.tar.xz https://nodejs.org/dist/v24.22.1/node-v24.22.1-linux-x64.tar.xz; \
    printf '%s\n' '6c172a9ab6718f7b801fb5a7bd486af2ee5e32758086a04b84fcce2aa77095e6  /tmp/node-v24.22.1-linux-x64.tar.xz' | sha256sum --check --strict; \
    tar -xJf /tmp/node-v24.22.1-linux-x64.tar.xz -C /opt; \
    test -x /opt/node-v24.22.1-linux-x64/bin/node; \
    mv /opt/node-v24.22.1-linux-x64 /opt/node-v24.22.1; \
    rm /tmp/node-v24.22.1-linux-x64.tar.xz

RUN set -eux; \
    curl --fail --location --show-error --silent --output /tmp/sccache-v0.10.0-x86_64-unknown-linux-musl.tar.gz https://github.com/mozilla/sccache/releases/download/v0.10.0/sccache-v0.10.0-x86_64-unknown-linux-musl.tar.gz; \
    printf '%s\n' '1fbb35e135660d04a2d5e42b59c7874d39b3deb17de56330b25b713ec59f849b  /tmp/sccache-v0.10.0-x86_64-unknown-linux-musl.tar.gz' | sha256sum --check --strict; \
    tar -xzf /tmp/sccache-v0.10.0-x86_64-unknown-linux-musl.tar.gz -C /tmp; \
    install -m 0755 /tmp/sccache-v0.10.0-x86_64-unknown-linux-musl/sccache /usr/local/bin/sccache; \
    sccache --version | grep -Fx 'sccache 0.10.0'; \
    rm -rf /tmp/sccache-v0.10.0-x86_64-unknown-linux-musl /tmp/sccache-v0.10.0-x86_64-unknown-linux-musl.tar.gz

RUN set -eux; \
    curl --fail --location --show-error --silent --output /tmp/gh_2.97.0_linux_amd64.tar.gz https://github.com/cli/cli/releases/download/v2.97.0/gh_2.97.0_linux_amd64.tar.gz; \
    printf '%s\n' 'a2c9b8497e1f85b1ad0dfcb78b5a622e098801b8e461e459e88e1ee12f018112  /tmp/gh_2.97.0_linux_amd64.tar.gz' | sha256sum --check --strict; \
    tar -xzf /tmp/gh_2.97.0_linux_amd64.tar.gz -C /tmp; \
    install -m 0755 /tmp/gh_2.97.0_linux_amd64/bin/gh /usr/local/bin/gh; \
    gh --version | grep -Eq '^gh version 2\.97\.0 '; \
    gh attestation --help >/dev/null; \
    rm -rf /tmp/gh_2.97.0_linux_amd64 /tmp/gh_2.97.0_linux_amd64.tar.gz

ENV NODE_22_HOME=/opt/node-v22.23.4 \
    NODE_24_HOME=/opt/node-v24.22.1 \
    PATH="/opt/node-v22.23.4/bin:/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

RUN printf '%s\n' 'export PATH="/opt/node-v22.23.4/bin:/usr/local/cargo/bin:$PATH"' > /etc/profile.d/ci-image-path.sh

CMD ["sleep", "infinity"]

FROM base AS builder

WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --locked -p polygon-nesting-cli

FROM debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818 AS runtime

ARG ENGINE_VERSION
ARG SOURCE_COMMIT
RUN test -n "$ENGINE_VERSION" \
    && printf '%s' "$ENGINE_VERSION" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$' \
    && test -n "$SOURCE_COMMIT" \
    && test "$SOURCE_COMMIT" != "unknown"
LABEL org.opencontainers.image.title="polygon-nesting" \
      org.opencontainers.image.source="https://github.com/jfet07-polygon-labs/polygon-nesting" \
      org.opencontainers.image.version="${ENGINE_VERSION}" \
      org.opencontainers.image.revision="${SOURCE_COMMIT}" \
      org.opencontainers.image.licenses="NOASSERTION"

RUN groupadd --gid 10001 polygon && useradd --uid 10001 --gid polygon --create-home polygon
COPY --from=builder /workspace/target/release/polygon-nesting /usr/local/bin/polygon-nesting
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY NOTICE /usr/share/doc/polygon-nesting/NOTICE
COPY LICENSES/clipper2-ts-BSL-1.0.txt /usr/share/doc/polygon-nesting/LICENSES/clipper2-ts-BSL-1.0.txt
COPY packages/polygon-nesting/schemas /usr/share/doc/polygon-nesting/schemas

USER polygon
ENTRYPOINT ["/usr/local/bin/polygon-nesting"]
