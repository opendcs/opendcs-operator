FROM rust:1-alpine3.21 AS builder

RUN apk add --no-cache musl-dev
WORKDIR /usr/src/myapp
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo new dummy
COPY Cargo.toml Cargo.lock ./
RUN cat > dummy.rs <<EOF
    fn main() {
        print!("dummy");
    }
EOF
RUN cat >> Cargo.toml <<EOF
[[bin]]
doc = false
name = "dummy"
path = "dummy.rs"
EOF

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --bin=dummy --release

COPY src ./src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    #--mount=type=cache,target=/usr/src/app/target \
    cargo install --target x86_64-unknown-linux-musl --path .


FROM scratch

USER 1000:1000
COPY --from=builder /usr/local/cargo/bin/lrgs ./
CMD [ "/lrgs" ]
