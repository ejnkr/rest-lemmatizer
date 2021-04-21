#!/bin/bash

readonly PROG_NAME=$(basename $0)
readonly PROG_DIR=$(readlink -f $(dirname $0))
readonly DIC_PATH=$PROG_DIR/..
readonly USERDIC_PATH=${DIC_PATH}/user-dic
readonly MECAB_EXEC_PATH=/usr/local/libexec/mecab
readonly DICT_INDEX=$MECAB_EXEC_PATH/mecab-dict-index

get_userdics() {
    echo $(ls "$USERDIC_PATH")
}

gen_cost() {
    local input_dic=$1
    echo $input_dic

    $DICT_INDEX \
        -m ${DIC_PATH}/model.def \
        -d ${DIC_PATH} \
        -u ${DIC_PATH}/user-${input_dic} \
        -f utf-8 \
        -t utf-8 \
        -a ${USERDIC_PATH}/$input_dic
}

compile() {
    pushd $DIC_PATH &> /dev/null
    make clean;make
    popd &> /dev/null
}

main() {
    echo "generating userdic..."

    for dic in $(get_userdics); do
        gen_cost $dic
    done

    compile
}

main
