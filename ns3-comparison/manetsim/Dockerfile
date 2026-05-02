FROM rust

RUN mkdir -p /home/ij22909
WORKDIR /home/ij22909
ADD ns3-comparison/manetsim/src src
ADD ns3-comparison/manetsim/Cargo.toml Cargo.toml
ADD ns3-comparison/manetsim/rust-toolchain.toml rust-toolchain.toml
ADD lib /lib

RUN cargo fetch

ENV CARGO_TARGET_DIR=/user/home/ij22909/internal
ENV RAYON_NUM_THREADS=1
ENV RUST_LOG=info

CMD ["cargo", "run", "--target", "host-tuple", "--release"]
