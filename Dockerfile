FROM alpine:latest as builder-system

RUN apk update && apk add binutils build-base ca-certificates curl file g++ gcc libressl-dev make patch rust linux-headers llvm-dev clang
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
RUN . ~/.cargo/env && rustup toolchain install nightly && rustup default nightly
RUN . ~/.cargo/env && rustup target add x86_64-unknown-linux-musl

# install mecab
ARG MECAB_URL=https://bitbucket.org/eunjeon/mecab-ko/downloads/mecab-0.996-ko-0.9.2.tar.gz
RUN mkdir /temp && cd /temp && \
    curl -SL -o mecab.tar.gz ${MECAB_URL} && \
    tar zxf mecab.tar.gz && \
    rm mecab.tar.gz && \
    cd mecab* && \
    ./configure --enable-utf8-only --with-charset=utf8 && \
    make && make install && ldconfig /usr/local/lib && \
    cd && rm -r /temp

COPY . ./

ENV RUSTFLAGS="-C target-feature=-crt-static -Clink-arg=-Wl,--allow-multiple-definition"

RUN . ~/.cargo/env && cargo build --release

FROM alpine

ARG MECAB_URL=https://bitbucket.org/eunjeon/mecab-ko/downloads/mecab-0.996-ko-0.9.2.tar.gz
ARG MECAB_DIC_URL=https://bitbucket.org/eunjeon/mecab-ko-dic/downloads/mecab-ko-dic-2.1.1-20180720.tar.gz

ENV MECAB_DIC_PATH=/mecab-dic

RUN apk add --no-cache --virtual .builddeps \
    curl make g++ automake autoconf ca-certificates && \
    mkdir /temp && cd /temp && \
    curl -SL -o mecab.tar.gz ${MECAB_URL} && \
    tar zxf mecab.tar.gz && \
    rm mecab.tar.gz && \
    cd mecab* && \
    ./configure --enable-utf8-only --with-charset=utf8 && \
    make && make install && ldconfig /usr/local/lib && \
    cd && rm -r /temp && \
    mkdir /temp && cd /temp && \
    curl -SL -o mecab-dic.tar.gz ${MECAB_DIC_URL} && \
    tar zxf mecab-dic.tar.gz && \
    rm mecab-dic.tar.gz && \
    cd mecab* && \
    ./autogen.sh && \
    ./configure --with-charset=utf8 && \
    make && make install && \
    mv /temp/mecab* ${MECAB_DIC_PATH} && \
    cd && rm -rf /temp && \
    rm ${MECAB_DIC_PATH}/tools/add-userdic.sh && \
    cd ${MECAB_DIC_PATH} && make clean && \
    apk del --purge .builddeps

RUN apk add --no-cache libstdc++ libgcc make bash curl

COPY assets/add-userdic.sh ${MECAB_DIC_PATH}/tools/add-userdic.sh
COPY assets/userdic.csv ${MECAB_DIC_PATH}/user-dic/userdic.csv

COPY --from=builder-system \
    /target/release/rest-tokenizer \
    /usr/local/bin/
COPY --from=builder-system \
    /rest-mecab/showcase/build/ \
    ./static/

COPY --from=builder-system \
    /target/release/rest-userdic \
    /usr/local/bin/

COPY assets/noun-extractor-model \
    ./noun-extractor-model

ENTRYPOINT ["bash", "-c"]
