MECAB_URL=https://bitbucket.org/eunjeon/mecab-ko/downloads/mecab-0.996-ko-0.9.2.tar.gz
MECAB_DIC_URL=https://bitbucket.org/eunjeon/mecab-ko-dic/downloads/mecab-ko-dic-2.1.1-20180720.tar.gz

#mkdir /tmp/mecab && cd /tmp/mecab
#curl -SL -o mecab.tar.gz ${MECAB_URL} 
#tar zxf mecab.tar.gz 
#cd mecab-* 
#./configure --enable-utf8-only --with-charset=utf8 
#make && make install && ldconfig /usr/local/lib 
#cd && rm -r mecab 
cd / && rm -rf /tmp/mecab
mkdir /tmp/mecab && cd /tmp/mecab
curl -SL -o mecab-dic.tar.gz ${MECAB_DIC_URL} 
tar zxf mecab-dic.tar.gz 
cd mecab-* 
./autogen.sh 
./configure --with-charset=utf8 
make && make install 
