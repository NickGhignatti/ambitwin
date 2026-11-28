FROM rust-base AS chef
WORKDIR /app/backend/chat

# ── capture the dependency graph ────────────────────────────────────────
FROM chef AS planner
COPY schemas/claims-schema /app/schemas/claims-schema
COPY backend/libs/rate-limit /app/backend/libs/rate-limit
COPY backend/chat .
RUN cargo chef prepare --recipe-path recipe.json

# Debug-profile dependencies, shared by the `test` and `dev` stages below.
FROM chef AS debug-deps
COPY --from=planner /app/backend/chat/recipe.json recipe.json
COPY schemas/claims-schema /app/schemas/claims-schema
COPY backend/libs/rate-limit /app/backend/libs/rate-limit
RUN --mount=type=cache,target=/sccache,sharing=shared \
    RUSTC_WRAPPER=sccache SCCACHE_DIR=/sccache CARGO_INCREMENTAL=0 \
    cargo chef cook --recipe-path recipe.json

FROM debug-deps AS test
COPY backend/chat .

FROM debug-deps AS dev
COPY backend/chat .
# Built here, with the flags `cargo run` uses, so a container starts ready instead of compiling.
RUN cargo build
CMD ["cargo", "watch", "-x", "run"]

FROM chef AS builder
COPY --from=planner /app/backend/chat/recipe.json recipe.json
COPY schemas/claims-schema /app/schemas/claims-schema
COPY backend/libs/rate-limit /app/backend/libs/rate-limit
RUN cargo chef cook --release --recipe-path recipe.json
COPY backend/chat .
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12:nonroot AS production
# distroless: no shell or package manager, patched upstream, so no apt-get upgrade step
COPY --from=builder /app/backend/chat/target/release/chat /usr/local/bin/chat

EXPOSE 3000
CMD ["/usr/local/bin/chat"]
