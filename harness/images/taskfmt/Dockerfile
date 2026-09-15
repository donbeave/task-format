# images/taskfmt/Dockerfile — harness-taskfmt: validation + runtime binaries on trixie-slim.
#
#   docker build -f harness/images/taskfmt/Dockerfile -t harness-taskfmt:latest .   (context = repo root)
#
# Stage 1 compiles the workspace crates; stage 2 ships both binaries on bare trixie-slim.
FROM rust:1.98-trixie AS build

RUN rustup toolchain install 1.98.1 --profile minimal && rustup default 1.98.1

ARG TASKFMT_GIT_COMMIT_SHA=unknown
ENV TASKFMT_GIT_COMMIT_SHA=${TASKFMT_GIT_COMMIT_SHA}

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/taskfmt-core/Cargo.toml crates/taskfmt-core/Cargo.toml
COPY crates/taskfmt-core/build.rs crates/taskfmt-core/build.rs
COPY crates/taskfmt/Cargo.toml crates/taskfmt/Cargo.toml
COPY crates/taskfmt-runtime/Cargo.toml crates/taskfmt-runtime/Cargo.toml
RUN mkdir -p crates/taskfmt-core/src crates/taskfmt/src crates/taskfmt-runtime/src \
 && echo "pub fn placeholder() {}" > crates/taskfmt-core/src/lib.rs \
 && echo "fn main() {}" > crates/taskfmt/src/main.rs \
 && echo "pub mod cli; pub mod cmds; pub mod ops; fn main() {}" > crates/taskfmt-runtime/src/lib.rs \
 && echo "fn main() {}" > crates/taskfmt-runtime/src/main.rs \
 && cargo build --release -p taskfmt -p taskfmt-runtime --locked \
 && rm -rf crates/*/src

COPY crates/taskfmt-core/src crates/taskfmt-core/src
COPY crates/taskfmt/src crates/taskfmt/src
COPY crates/taskfmt-runtime/src crates/taskfmt-runtime/src
ARG BUILDKIT_INLINE_CACHE=1
RUN touch crates/taskfmt-core/src/lib.rs \
 && cargo build --release -p taskfmt -p taskfmt-runtime --locked \
 && /build/target/release/taskfmt --version \
 && /build/target/release/taskfmt-runtime --version

FROM debian:trixie-slim

COPY --from=build /build/target/release/taskfmt /usr/local/bin/taskfmt
COPY --from=build /build/target/release/taskfmt-runtime /usr/local/bin/taskfmt-runtime
RUN /usr/local/bin/taskfmt --version && /usr/local/bin/taskfmt-runtime --version
