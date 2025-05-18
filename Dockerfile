FROM rust:1.82 AS builder
WORKDIR /usr/src/app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim AS runner
WORKDIR /usr/src/app
RUN apt-get update -y \
  && apt-get install -y --no-install-recommends ca-certificates openssl tini \
  && apt-get autoremove -y \
  && apt-get clean -y \
  && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/target/release/integrations_api ./integrations_api
COPY --from=builder /usr/src/app/configs ./configs
ENV APP_ENVIRONMENT=production
ENTRYPOINT ["tini", "--"]
CMD ["./integrations_api"]
