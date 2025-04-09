FROM rust:1.86.0-alpine3.21 AS builder

RUN apk add --no-cache musl-dev

WORKDIR /usr/src/myapp
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/app/target \
    cargo install --target `uname -m`-unknown-linux-musl --path .

FROM scratch

USER 1000:1000
COPY --from=builder /usr/local/cargo/bin/lrgs ./
CMD [ "/lrgs" ]
