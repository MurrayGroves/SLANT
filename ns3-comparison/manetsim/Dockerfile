FROM rust

RUN mkdir -p /home/ij22909
WORKDIR /home/ij22909
ADD src src
ADD Cargo.toml Cargo.toml
ADD rust-toolchain.toml rust-toolchain.toml

RUN cargo fetch

ENV CARGO_TARGET_DIR=/user/home/ij22909/internal
ENV RAYON_NUM_THREADS=1
ENV RUST_LOG=info

CMD ["cargo", "run", "--target", "host-tuple", "--release",  "--features", "disable_internal_stats,disable_internal_events"]