echo "1. add nouns"
#curl -XPOST localhost:8000/train -w "\n%{time_connect}:%{time_starttransfer}:%{time_total}\n" --data-binary "@/home/song/Projects/nlp/dataset/shards/date=2021-02-04/part-00000-7033d27b-5ea1-417a-80d9-dcb2930d7675-c000.txt"
curl -XPOST localhost:8000/train -w "\n%{time_connect}:%{time_starttransfer}:%{time_total}\n" --data-binary "감스트가 감스트를 감스트는 감스트의 감스트도 감스트의 감스트에 감스트ㅋ 감스트 감스트가 감스트가 감스트를 감스트는 감스트의 감스트도 감스트의 감스트에 감스트ㅋ 감스트 감스트가 감스트가 감스트를 감스트는 감스트의 감스트도 감스트의 감스트에 감스트ㅋ 감스트 감스트가 감스트가 감스트를 감스트는 감스트의 감스트도 감스트의 감스트에 감스트ㅋ 감스트 감스트가 감스트가 감스트를 감스트는 감스트의 감스트도 감스트의 감스트에 감스트ㅋ 감스트 감스트가 감스트가 감스트를 감스트는 감스트의 감스트도 감스트의 감스트에 감스트ㅋ 감스트 감스트가 감스트가 감스트를 감스트는 감스트의 감스트도 감스트의 감스트에 감스트ㅋ 감스트 감스트가 감스트가 감스트를 감스트는 감스트의 감스트도 감스트의 감스트에 감스트ㅋ 감스트 감스트가"
curl -XGET localhost:8000/nouns

echo "2. sync nouns"
curl -XPOST localhost:8080/sync-userdic

echo "3. tokenize before sync"
curl -XGET localhost:8080/tokenize?q=%EA%B0%90%EC%8A%A4%ED%8A%B8

echo "4. wait 10 secs.."
sleep 10

echo "5. tokenize after sync"
curl -XGET localhost:8080/tokenize?q=%EA%B0%90%EC%8A%A4%ED%8A%B8
