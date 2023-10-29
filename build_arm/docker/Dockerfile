FROM debian:11

# install armv7 cross compile
RUN apt-get update && \
    apt-get install -y curl

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
RUN /root/.cargo/bin/rustup target add armv7-unknown-linux-gnueabihf

RUN dpkg --add-architecture armhf && apt-get update

RUN apt-get install -y \
    build-essential \
    clang \
    crossbuild-essential-armhf \
    gcc-arm-linux-gnueabihf

RUN apt-get install -y \
    libxcb1-dev:armhf \
    libxcb-randr0-dev:armhf \
    libxcb-shm0-dev:armhf
