# images/taskfmt/Dockerfile — harness-taskfmt: the Rust `taskfmt` binary, no shell tooling.
#
#   docker build -f images/taskfmt/Dockerfile -t harness-taskfmt:latest .   (context = harness/)
#
# Stage 1 compiles the crate; stage 2 ships only the static-ish binary on bare bookworm-slim.
# The runtime image needs no git/docker/herdr: those live in harness-base. What it does need is
# the same glibc the base image uses, hence the bookworm-pinned rust image (rust:*-slim tracks
# current Debian stable = trixie, whose glibc is newer than bookworm's) and bookworm-slim export.
FROM rust:1.98-bookworm AS build

WORKDIR /build
# manifests first so the dependency layer caches independently of the sources
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "pub fn placeholder() {}" > src/lib.rs \
 && echo "fn main() {}" > src/main.rs \
 && cargo build --release \
 && rm -rf src

COPY src ./src
ARG BUILDKIT_INLINE_CACHE=1
RUN touch src/lib.rs src/main.rs && cargo build --release --bin taskfmt \
 && /build/target/release/taskfmt --version

FROM debian:bookworm-slim

COPY --from=build /build/target/release/taskfmt /usr/local/bin/taskfmt
# smoke test at build time: a broken image must fail the build, not the run
RUN /usr/local/bin/taskfmt --version

ENTRYPOINT ["/usr/local/bin/taskfmt"]
