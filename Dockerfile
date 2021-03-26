# Compile
FROM    alpine:3.10 AS compiler

RUN     apk update --quiet
RUN     apk add curl
RUN     apk add build-base

RUN     curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

WORKDIR /binance-bot

COPY    . .

ENV     RUSTFLAGS="-C target-feature=-crt-static"

RUN     $HOME/.cargo/bin/cargo build --release

# Run
FROM    alpine:3.10

RUN     apk add -q --no-cache libgcc tini

COPY    --from=compiler /binance-bot/target/release/binance-bot .

# ENV     MEILI_HTTP_ADDR 0.0.0.0:7700
# EXPOSE  7700/tcp

ENTRYPOINT ["tini", "--"]
CMD     ./binance-bot