FROM rust:1.88.0-slim-bookworm AS builder

WORKDIR /app
ENV SQLX_OFFLINE=true

COPY Cargo.toml rust-toolchain ./
COPY .sqlx ./.sqlx
COPY db ./db
COPY src ./src

RUN cargo build --release

FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=builder /app/target/release/zunda-bot-rs /usr/local/bin/zunda-bot-rs

ENV PORT=8080
EXPOSE 8080

USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/zunda-bot-rs"]
