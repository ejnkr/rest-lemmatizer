use actix_web::{get, post, web, App, HttpServer, Responder};
use serde::Deserialize;

use noun_extractor::model::State;

use async_rwlock::RwLock;

#[derive(Debug, derive_more::Display, derive_more::Error)]
struct Error {
    err: anyhow::Error,
}
impl actix_web::error::ResponseError for Error { }
impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Error {
        Error { err }
    }
}


#[derive(Debug, Deserialize)]
struct Train {
    lines: String,
}

#[get("/train")]
async fn train(q: web::Query<SearchQuery>, state: web::Data<RwLock<State>>) -> impl Responder {
    let lines = q.into_inner().lines;
    state.train(&opts.input)?;
    let result = tokenizer.read().await.tokenize(&q);
    result
}

#[get("/health")]
async fn health() -> impl Responder {
    "ok"
}

#[post("/nouns")]
async fn nouns(body: web::Bytes, tokenizer: web::Data<RwLock<Tokenizer>>) -> Result<String, Error> {
    println!("1");
    let nouns: Vec<String> = bincode::deserialize(&body).map_err(anyhow::Error::from)?;
    println!("2");
    tokenizer.read().await.gen_userdic(nouns).await.map_err(anyhow::Error::from)?;
    println!("3");
    tokenizer.write().await.reload();
    println!("4");
    Ok("".to_string())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let port = std::env::var("PORT").unwrap_or("8080".to_string());
    HttpServer::new(|| {
        let mecab_dic_path = std::env::var("MECAB_DIC_PATH").unwrap_or("/usr/local/lib/mecab".to_string());
        let tokenizer = Tokenizer::new(mecab_dic_path);
        let data =  web::Data::new(RwLock::new(tokenizer));
        App::new()
            .app_data(data)
            .service(search)
            .service(nouns)
    })
        .bind(&format!("0.0.0.0:{}", port))?
        .run()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn test_server() -> actix_test::TestServer {
        actix_test::start_with(actix_test::config().h1(), || {
            let mecab_dic_path = "./mecab-ko-dic".to_string();
            let mut tokenizer = Tokenizer::new(mecab_dic_path);
            let data =  web::Data::new(RwLock::new(tokenizer));
            App::new()
                .app_data(data)
                .service(search)
                .service(nouns)
        })
    }
    #[actix_rt::test]
    #[serial]
    async fn test_example() {
        let srv = test_server();

        let req = srv.get("/search?q=%EC%95%88%EB%85%95");
        let mut res = req.send().await.unwrap();

        assert!(res.status().is_success());
        assert_eq!(String::from_utf8(res.body().await.unwrap().to_vec()).unwrap(), "안녕\tIC,*,T,안녕,*,*,*,*\nEOS\n".to_string());
    }
    #[actix_rt::test]
    #[serial]
    async fn test_regist_nouns() {
        let mecab_dic_path = "./mecab-ko-dic".to_string();
        let mut tokenizer = Tokenizer::new(mecab_dic_path);
        tokenizer.gen_userdic(vec!["뤣쉙퀡".to_string()]).await.unwrap();
        tokenizer.reload();
        let res = tokenizer.tokenize("뤣쉙퀡");
        assert_eq!(res, "뤣쉙퀡\tNNP,*,T,뤣쉙퀡,*,*,*,*\nEOS\n");
    }
    #[actix_rt::test]
    #[serial]
    async fn test_concurrent_jobs() {
        let srv = test_server();
        let search_reqs = (0..10u32).map(|_| srv.get("/search?q=%ED%86%A9%ED%86%A9%ED%86%A9%0A").timeout(std::time::Duration::from_secs(5)).send());
        //let sync_reqs = srv.get("/search?q=%ED%86%A9%ED%86%A9%ED%86%A9%0A").timeout(std::time::Duration::from_secs(5)).send();
        let sync_reqs = (0..2u32).map(|_| srv.post("/nouns").timeout(std::time::Duration::from_secs(60)).send_body(bincode::serialize(&vec!["톩톩톩"]).unwrap()));
        let (a, b) = futures::join!(futures::future::join_all(search_reqs), futures::future::join_all(sync_reqs));
        for i in a {
            assert!(i.unwrap().status().is_success());
        }
        for i in b {
            assert!(i.unwrap().status().is_success());
        }
        let mut res = srv.get("/search?q=%ED%86%A9%ED%86%A9%ED%86%A9%0A").send().await.unwrap();
        assert!(res.status().is_success());
        assert_eq!(
            String::from_utf8(res.body().await.unwrap().to_vec()).unwrap(), 
            "톩톩톩\tNNP,*,T,톩톩톩,*,*,*,*\nEOS\n".to_string());
    }
}
