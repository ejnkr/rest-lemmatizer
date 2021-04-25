# 형태소 추출기

mecab-ko 기반의 rest api server 형태의 형태소 추출기

다음과 같은 기능을 가지고있음

- 토크나이징
- 형태소 추출
- 신조어 명사 학습
- 위 기능들을 rest api 형태로 제공

형태소 분석기와 신조어 명사학습기를 분리하여 스케일 아웃이 용이하도록 만듬


# 기능 구현

- [x] 명사 추출 모듈
- [x] mecab rest server 모듈
- [x] 도커 이미지
- [x] 헬름 차트


# 설치

헬름 차트 형태로 설치하기를 권장함

다음과 같은 명령어로, `my-release` 라는 릴리즈 이름으로 설치할 수 있음

```console
$ helm repo add rest-lemmatizer https://ejnkr.github.io/rest-lemmatizer
$ helm install my-release rest-lemmatizer/rest-lemmatizer 
```

이 경우 설치 파라미터는 디폴트 값을 가지게됨.

tokenizer.replicas 값을 설정하여 형태소추출기를 스케일아웃 시킬수 있음(명사추출기는 아직 확장이 불가능함)

다른 설정 파라미터에 대해서는 `charts/rest-lemmatizer/values.yaml`를 참조


# 학습 원리

한국어에서 명사에는 주로 조사가 붙는다는 특성을 이용하면 명사를 추출하기 용이함.

한국어에서 대부분의 신조어는 명사이며, 문장에서 주로 의미를 반영하는 경우가 많음.

따라서 명사를 추출 후 학습하여 미등록 사전 문제를 어느정도 해결할 수 있음.


# 사용법

설치 후 rest api를 이용해 조작할수있음.

`my-release`라는 릴리즈 네임으로 설치하였고, 쿠버네티스 네트워크 안에 있고, curl을 이용해 통신한다면 다음과 같이 조작할수있음

```console
# 명사 추출 
$ curl -XPOST `my-release-userdic:8080/train --data-binary @<line-splited-text-dataset-path>

# 형태소 추출기 - 명사 추출기 동기화 (하루에 한번씩 자동으로 동기화되며, 대략 수십초가 소요됨)
$ curl -XPOST `my-release-tokenizer:8080/sync-userdic

# 형태소 추출
$ curl -XPOST `my-release-tokenizer:8080/tokenize?q=<text>
```

# TODO

- [ ] 형태소 추출기 - 명사 추출기 동기화 주기 파라미터화(지금은 하루로 설정되있음)
- [ ] hash-sharding을 이용해 명사 추출기도 스케일 아웃이 가능하도록 만들기
