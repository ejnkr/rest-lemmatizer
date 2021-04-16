FROM alpine

ARG MECAB_URL=https://bitbucket.org/eunjeon/mecab-ko/downloads/mecab-0.996-ko-0.9.2.tar.gz
ARG MECAB_DIC_URL=https://bitbucket.org/eunjeon/mecab-ko-dic/downloads/mecab-ko-dic-2.0.1-20150920.tar.gz

# --virtual .builddeps
RUN apk add --no-cache \ 
    curl make g++ file bash openssl git automake autoconf

# install mecab
RUN mkdir /temp && cd /temp && \
    curl -SL -o mecab.tar.gz ${MECAB_URL} && \
    tar zxf mecab.tar.gz && \
    rm mecab.tar.gz && \
    cd mecab* && \
    ./configure --enable-utf8-only --with-charset=utf8 && \
    make && make install && ldconfig /usr/local/lib && \
    cd && rm -r /temp

ENV MECAB_DIC_PATH=/mecab-dic

# install mecab-dic
RUN mkdir /temp && cd /temp && \
    curl -SL -o mecab-dic.tar.gz ${MECAB_DIC_URL} && \
    tar zxf mecab-dic.tar.gz && \
    rm mecab-dic.tar.gz && \
    cd mecab* && \
    ./autogen.sh && \
    ./configure --with-charset=utf8 && \
    make && make install && \
    mv /temp/mecab* ${MECAB_DIC_PATH} && \
    cd && rm -rf /temp 
    # remove dependencies apk del --purge .builddeps

ENTRYPOINT ["mecab"]

