use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use serde::Deserialize;

pub mod tokenizer;

use async_rwlock::RwLock;
use tokenizer::Tokenizer;

use postage::prelude::{Sink, Stream};

#[derive(Debug, derive_more::Display, derive_more::Error)]
struct Error {
    err: anyhow::Error,
}
impl actix_web::error::ResponseError for Error {}
impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Error {
        Error { err }
    }
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
}

#[get("/tokenize")]
async fn tokenize(
    q: web::Query<SearchQuery>,
    tokenizer: web::Data<RwLock<Tokenizer>>,
) -> Result<HttpResponse, Error> {
    let q = q.into_inner().q;
    let result = tokenizer.read().await.tokenize(&q)?;
    Ok(HttpResponse::Ok().json(result))
}

#[post("/tokenize")]
async fn tokenize_post(
    bytes: web::Bytes,
    tokenizer: web::Data<RwLock<Tokenizer>>,
) -> Result<HttpResponse, Error> {
    let q = String::from_utf8(bytes.to_vec()).map_err(anyhow::Error::from)?;
    let result = tokenizer.read().await.tokenize(&q)?;
    Ok(HttpResponse::Ok().json(result))
}

#[get("/health")]
async fn health() -> impl Responder {
    "ok"
}

#[post("/sync-userdic")]
async fn sync_userdic(
    tokenizer: web::Data<RwLock<Tokenizer>>,
    reload_tx: web::Data<RwLock<postage::broadcast::Sender<()>>>,
) -> Result<String, Error> {
    let userdic_server_url = std::env::var("USERDIC_SERVER_URL")
        .map_err(|_| anyhow::Error::msg("USERDIC_SERVER_URL"))?;
    let client = awc::Client::default();
    let res = client
        .get(&userdic_server_url)
        .send()
        .await
        .unwrap()
        .body()
        .limit(1024 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    let nouns: Vec<String> = serde_json::from_slice(&res).unwrap();
    tokenizer
        .read()
        .await
        .gen_userdic_async(nouns)
        .await
        .map_err(anyhow::Error::from)?;
    reload_tx
        .write()
        .await
        .send(())
        .await
        .map_err(anyhow::Error::from)?;
    //tokenizer.write().await.reload();
    Ok("".to_string())
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let userdic_server_url = std::env::var("USERDIC_SERVER_URL");
    let userdic_sync_interval_seconds: u64 = std::env::var("USERDIC_SYNC_INTERVAL_SECONDS")
        .unwrap_or_else(|_| "86400".to_string())
        .parse()?;
    let mecab_dic_path =
        std::env::var("MECAB_DIC_PATH").unwrap_or_else(|_| "/mecab-dic".to_string());
    let mut tokenizer = Tokenizer::new(mecab_dic_path.clone());
    //let data = web::Data::new(RwLock::new(tokenizer));
    let (reload_tx, reload_rx) = postage::broadcast::channel(8);
    tokenizer
        .gen_userdic_async(vec![])
        .await
        .map_err(anyhow::Error::from)?;
    tokenizer.reload();
    if let Ok(userdic_server_url) = userdic_server_url {
        let mut reload_tx = reload_tx.clone();
        actix_web::rt::spawn(async move {
            loop {
                let res: Result<(), anyhow::Error> = (async {
                    let client = awc::Client::default();
                    loop {
                        let res = client
                            .get(&userdic_server_url)
                            .send()
                            .await
                            .map_err(|_| anyhow::Error::msg("userdic server request fail"))?
                            .body()
                            .limit(1024 * 1024 * 1024)
                            .await?
                            .to_vec();
                        let nouns: Vec<String> = serde_json::from_slice(&res)?;
                        if !nouns.is_empty() {
                            tokenizer.gen_userdic_async(nouns).await?;
                            tokenizer.reload();
                            reload_tx.send(()).await?;
                        }
                        actix_web::rt::time::sleep(std::time::Duration::from_secs(
                            userdic_sync_interval_seconds,
                        ))
                        .await;
                    }
                })
                .await;
                if let Err(err) = res {
                    println!("ERROR: {}", err);
                    actix_web::rt::time::sleep(std::time::Duration::from_secs(
                        userdic_sync_interval_seconds,
                    ))
                    .await;
                }
            }
        });
    }
    Ok(HttpServer::new(move || {
        let mecab_dic_path = mecab_dic_path.clone();
        let tokenizer = Tokenizer::new(mecab_dic_path);
        let tokenizer = web::Data::new(RwLock::new(tokenizer));
        let mut reload_rx = reload_rx.clone();
        let tokenizer_ = tokenizer.clone();
        actix_web::rt::spawn(async move {
            while reload_rx.recv().await.is_some() {
                println!("reload tokenizer");
                tokenizer_.write().await.reload();
            }
        });
        let reload_tx = web::Data::new(RwLock::new(reload_tx.clone()));
        /*data.read()
            .await
            .gen_userdic_async(vec![])
            .await
            .map_err(anyhow::Error::from)?;
        data.write().await.reload();*/

        App::new()
            .app_data(tokenizer)
            .app_data(reload_tx)
            .service(tokenize)
            .service(sync_userdic)
            .service(tokenize_post)
    })
    .bind(&format!("0.0.0.0:{}", port))?
    .run()
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    use serial_test::serial;

    fn test_server() -> actix_test::TestServer {
        actix_test::start_with(actix_test::config().h1(), || {
            let mecab_dic_path = "./mecab-ko-dic".to_string();
            let tokenizer = Tokenizer::new(mecab_dic_path);
            let data = web::Data::new(RwLock::new(tokenizer));
            App::new()
                .app_data(data)
                .service(tokenize)
                .service(sync_userdic)
        })
    }
    #[actix_rt::test]
    #[serial]
    async fn test_example() {
        let srv = test_server();

        let req = srv.get("/tokenize?q=%EC%95%88%EB%85%95");
        let mut res = req.send().await.unwrap();

        assert!(res.status().is_success());
        assert_eq!(
            String::from_utf8(res.body().await.unwrap().to_vec()).unwrap(),
            "안녕\tIC,*,T,안녕,*,*,*,*\nEOS\n".to_string()
        );
    }
    #[actix_rt::test]
    #[serial]
    async fn test_regist_nouns() {
        let mecab_dic_path = "./mecab-ko-dic".to_string();
        let mut tokenizer = Tokenizer::new(mecab_dic_path);
        tokenizer
            .gen_userdic_async(vec!["뤣쉙퀡".to_string()])
            .await
            .unwrap();
        tokenizer.reload();
        let res = tokenizer.tokenize("뤣쉙퀡").unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].tags.len(), 1);
        assert_eq!(res[0].tags[0], "NNG");
    }
    #[actix_rt::test]
    #[serial]
    async fn test_concurrent_jobs() {
        let srv = test_server();
        let search_reqs = (0..10u32).map(|_| {
            srv.get("/tokenize?q=%ED%86%A9%ED%86%A9%ED%86%A9%0A")
                .timeout(std::time::Duration::from_secs(5))
                .send()
        });
        //let sync_reqs = srv.get("/search?q=%ED%86%A9%ED%86%A9%ED%86%A9%0A").timeout(std::time::Duration::from_secs(5)).send();
        let sync_reqs = (0..2u32).map(|_| {
            srv.post("/sync-userdic")
                .timeout(std::time::Duration::from_secs(60))
                .send_body(bincode::serialize(&vec!["톩톩톩"]).unwrap())
        });
        let (a, b) = futures::join!(
            futures::future::join_all(search_reqs),
            futures::future::join_all(sync_reqs)
        );
        for i in a {
            assert!(i.unwrap().status().is_success());
        }
        for i in b {
            assert!(i.unwrap().status().is_success());
        }
        let mut res = srv
            .get("/tokenize?q=%ED%86%A9%ED%86%A9%ED%86%A9%0A")
            .send()
            .await
            .unwrap();
        assert!(res.status().is_success());
        assert_eq!(
            String::from_utf8(res.body().await.unwrap().to_vec()).unwrap(),
            "톩톩톩\tNNP,*,T,톩톩톩,*,*,*,*\nEOS\n".to_string()
        );
    }

    #[actix_rt::test]
    #[serial]
    async fn concurrent_tokenize() {
        let srv = test_server();
        let search_reqs = (0..100u32).map(|_| {
            let rand_string: String = thread_rng()
                .sample_iter(&Alphanumeric)
                .take(30)
                .map(char::from)
                .collect();
            srv.get(&format!("/tokenize?q={}", rand_string))
                .timeout(std::time::Duration::from_secs(5))
                .send()
        });
        let res = futures::future::join_all(search_reqs)
            .await
            .into_iter()
            .map(|i| i.unwrap());
        for mut i in res {
            assert!(i.status().is_success());
        }
    }
}
