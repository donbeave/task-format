# images/taskfmt/Dockerfile — harness-taskfmt: the Rust `taskfmt` binary, no shell tooling.
#
#   docker build -f images/taskfmt/Dockerfile -t harness-taskfmt:latest .   (context = harness/)
#
# Stage 1 compiles the crate; stage 2 ships only the static-ish binary on bare trixie-slim.
# The runtime image needs no git/docker/herdr: those live in harness-base. What it does need is
# the same glibc the base image uses, hence the trixie-pinned rust image and trixie-slim export.
FROM rust:1.98-trixie AS build

# The official patch-specific Docker tag is not published yet; select the exact
# stable toolchain through rustup independently of the distro image tag.
RUN rustup toolchain install 1.98.1 --profile minimal && rustup default 1.98.1

ARG TASKFMT_GIT_COMMIT_SHA=unknown
ENV TASKFMT_GIT_COMMIT_SHA=${TASKFMT_GIT_COMMIT_SHA}

WORKDIR /build
# manifests and the build script first, so the dependency layer caches independently of the
# sources. The script bakes the Git commit into the binary.
COPY Cargo.toml Cargo.lock build.rs ./
RUN mkdir -p src/bin \
 && echo "pub fn placeholder() {}" > src/lib.rs \
 && echo "fn main() {}" > src/bin/taskfmt.rs \
 && cargo build --release --bin taskfmt --locked \
 && rm -rf src

COPY src ./src
ARG BUILDKIT_INLINE_CACHE=1
RUN touch src/lib.rs src/main.rs && cargo build --release --bin taskfmt --locked \
 && /build/target/release/taskfmt --version

FROM debian:trixie-slim

COPY --from=build /build/target/release/taskfmt /usr/local/bin/taskfmt
# smoke test at build time: a broken image must fail the build, not the run
RUN /usr/local/bin/taskfmt --version

ENTRYPOINT ["/usr/local/bin/taskfmt"]
