FROM rust:slim AS build

RUN apt-get update \
    && apt-get install -y \
        --no-install-recommends \
        git

RUN git clone https://gitea.sarahgreywolf.tech/SarahGreyWolf/HookMe.git /tmp/hookme
RUN cd /tmp/hookme \
    && cargo build --release

RUN mkdir /hookme \
    && cp /tmp/hookme/target/release/hook_me /hookme/hookme

FROM rust:slim AS run

LABEL maintainer="SarahGreyWolf <m.sarahgreywolf@outlook.com>"
LABEL description="Docker image for building and running HookMe"

COPY --from=build /hookme /hookme

WORKDIR /hookme
ENTRYPOINT ["hook_me"]