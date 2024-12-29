FROM rust:1.83-bullseye

COPY . .

RUN cargo build --release
