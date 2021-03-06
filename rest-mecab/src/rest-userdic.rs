use actix_web::{get, post, web, App, FromRequest, HttpResponse, HttpServer, Responder};

use noun_extractor::model::{Score, State as NounExtractorState};
use serde::Deserialize;

use async_rwlock::RwLock;
use rocksdb::{BlockBasedOptions, IteratorMode, Options, DB};

use std::path::Path;

use hangul_normalize::{control_chars, derepeat, whitespace_less};

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

fn rocksdb_default_opts() -> Options {
    let mut opts = Options::default();
    // https://github.com/facebook/rocksdb/wiki/Setup-Options-and-Basic-Tuning
    #[allow(deprecated)]
    opts.set_max_background_compactions(4);
    #[allow(deprecated)]
    opts.set_max_background_flushes(2);
    opts.set_level_compaction_dynamic_level_bytes(true);
    opts.set_bytes_per_sync(1048576);
    opts.create_if_missing(true);

    let mut table_opts = BlockBasedOptions::default();
    table_opts.set_pin_l0_filter_and_index_blocks_in_cache(true);
    table_opts.set_cache_index_and_filter_blocks(true);
    table_opts.set_cache_index_and_filter_blocks(true);
    table_opts.set_block_size(16 * 1024);
    table_opts.set_format_version(5);

    // options.compaction_pri = kMinOverlappingRatio;
    opts.set_block_based_table_factory(&table_opts);
    opts
}

struct State {
    noun_scores: DB,
    noun_extractor: NounExtractorState,
    unique_suffixes_count_threshold: f64,
    count_threshold: u32,
    noun_probability_threshold: f32,
    nouns: DB,
}
impl State {
    fn open<P: AsRef<Path>>(noun_extractor_model_path: P, store_path: P) -> anyhow::Result<Self> {
        Ok(Self {
            noun_extractor: NounExtractorState::open(noun_extractor_model_path)?,
            noun_scores: DB::open(
                &rocksdb_default_opts(),
                store_path.as_ref().join("noun_scores"),
            )?,
            nouns: DB::open(&rocksdb_default_opts(), store_path.as_ref().join("nouns"))?,
            unique_suffixes_count_threshold: 5.0,
            count_threshold: 30,
            noun_probability_threshold: 0.9,
        })
    }
    pub fn set_threshold(
        &mut self,
        unique_suffixes_count: f64,
        count: u32,
        noun_probability: f32,
    ) -> &mut Self {
        self.unique_suffixes_count_threshold = unique_suffixes_count;
        self.count_threshold = count;
        self.noun_probability_threshold = noun_probability;
        self
    }
    fn train(&mut self, s: String) -> anyhow::Result<i32> {
        let s = control_chars(&s, "_");
        let s = whitespace_less(&s);
        let s = derepeat(&s, 3);
        let mut scores = self.noun_extractor.extract_nouns(&s)?;
        for (candidate, score) in scores.iter_mut() {
            let key = bincode::serialize(&candidate)?;
            if let Some(prev_score) = self.noun_scores.get(&key)? {
                let prev_score: Score = bincode::deserialize(&prev_score)?;
                score.merge(&prev_score);
            }
        }
        self.nouns
            .set_options(&[("disable_auto_compactions", "true")])?;
        self.noun_scores
            .set_options(&[("disable_auto_compactions", "true")])?;
        for (candidate, score) in scores.iter() {
            let key = bincode::serialize(&candidate)?;
            self.noun_scores.put(key, bincode::serialize(&score)?)?;
        }
        let mut count = 0;
        for (candidate, score) in scores {
            let key = bincode::serialize(&candidate)?;
            self.noun_scores.put(key, bincode::serialize(&score)?)?;
            if score.noun_probability >= self.noun_probability_threshold
                && score.unique_suffixes_hll.len() >= self.unique_suffixes_count_threshold
                && score.count >= self.count_threshold
            {
                self.nouns.put(candidate, &[0])?;
                count += 1;
            } else {
                self.nouns.delete(&candidate)?;
            }
        }
        self.nouns
            .set_options(&[("disable_auto_compactions", "false")])?;
        self.noun_scores
            .set_options(&[("disable_auto_compactions", "false")])?;
        Ok(count)
    }
    fn noun_score(&self, noun: &str) -> anyhow::Result<Option<Score>> {
        let key = bincode::serialize(&noun)?;
        if let Some(bytes) = self.noun_scores.get(&key)? {
            Ok(Some(bincode::deserialize(&bytes)?))
        } else {
            Ok(None)
        }
    }
    fn nouns(&self) -> Vec<String> {
        self.nouns
            .iterator(IteratorMode::Start)
            .map(|(k, _)| String::from_utf8_lossy(&k).to_string())
            .collect()
    }
}

#[post("/train")]
async fn train(bytes: web::Bytes, state: web::Data<RwLock<State>>) -> Result<String, Error> {
    let lines = String::from_utf8(bytes.to_vec()).map_err(anyhow::Error::from)?;
    Ok(format!("{:?}", state.write().await.train(lines)?))
}

#[get("/nouns")]
async fn nouns(state: web::Data<RwLock<State>>) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().json(state.read().await.nouns()))
}

#[derive(Deserialize)]
struct ScoreQuery {
    noun: String,
}
#[get("/noun-score")]
async fn noun_score(
    state: web::Data<RwLock<State>>,
    query: web::Query<ScoreQuery>,
) -> Result<HttpResponse, Error> {
    let noun = query.into_inner().noun;
    let score = state.read().await.noun_score(&noun)?;
    Ok(HttpResponse::Ok().json(score))
}

#[get("/health")]
async fn health() -> impl Responder {
    "ok"
}

#[derive(Deserialize)]
struct SetThresholdQuery {
    unique_suffixes_count: f64,
    count: u32,
    noun_probability: f32,
}

#[post("/set-threshold")]
async fn set_threshold(
    query: web::Json<SetThresholdQuery>,
    state: web::Data<RwLock<State>>,
) -> impl Responder {
    let query = query.into_inner();
    state.write().await.set_threshold(
        query.unique_suffixes_count,
        query.count,
        query.noun_probability,
    );
    "done"
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let noun_extractor_model_path = std::env::var("NOUN_EXTRACTOR_MODEL_PATH")
        .unwrap_or_else(|_| "noun-extractor-model".to_string());
    let store_path = std::env::var("STORE_PATH").unwrap_or_else(|_| "store".to_string());
    let state = State::open(noun_extractor_model_path, store_path)?;
    let data = web::Data::new(RwLock::new(state));
    let unique_suffixes_count_threshold: f64 = std::env::var("UNIQUE_SUFFIXES_COUNT_THRESHOLD")
        .unwrap_or_else(|_| "5.0".to_string())
        .parse()?;
    let count_threshold: u32 = std::env::var("COUNT_THRESHOLD")
        .unwrap_or_else(|_| "30".to_string())
        .parse()?;
    let noun_probability_threshold: f32 = std::env::var("NOUN_PROBABILITY_THRESHOLD")
        .unwrap_or_else(|_| "0.9".to_string())
        .parse()?;
    data.write().await.set_threshold(
        unique_suffixes_count_threshold,
        count_threshold,
        noun_probability_threshold,
    );

    Ok(HttpServer::new(move || {
        let data = data.clone();
        App::new()
            .app_data(data)
            .app_data(web::PayloadConfig::new(1024 * 1024 * 1024))
            .app_data(web::Bytes::configure(|cfg| cfg.limit(1024 * 1024 * 1024)))
            .service(train)
            .service(nouns)
            .service(health)
            .service(set_threshold)
            .service(noun_score)
    })
    .bind(&format!("0.0.0.0:{}", port))?
    .run()
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn test_server() -> actix_test::TestServer {
        actix_test::start_with(actix_test::config().h1(), || {
            let noun_extractor_path =
                std::env::var("NOUN_EXTRACTOR_PATH").expect("NOUN_EXTRACTOR_PATH");
            let scores_store_path = std::env::var("SCORES_STORE_PATH").expect("SCORES_STORE_PATH");
            let state = State::open(noun_extractor_path, scores_store_path).unwrap();
            App::new()
                .app_data(web::Data::new(RwLock::new(state)))
                .service(train)
                .service(nouns)
                .service(health)
        })
    }
    #[actix_rt::test]
    #[serial]
    async fn test_example() {
        let srv = test_server();

        let req = srv.get("/search?q=%EC%95%88%EB%85%95");
        let mut res = req.send().await.unwrap();

        assert!(res.status().is_success());
        assert_eq!(
            String::from_utf8(res.body().await.unwrap().to_vec()).unwrap(),
            "??????\tIC,*,T,??????,*,*,*,*\nEOS\n".to_string()
        );
    }
    /*#[actix_rt::test]
    #[serial]
    async fn test_regist_nouns() {
        let mecab_dic_path = "./mecab-ko-dic".to_string();
        let mut tokenizer = Tokenizer::new(mecab_dic_path);
        tokenizer
            .gen_userdic(vec!["?????????".to_string()])
            .await
            .unwrap();
        tokenizer.reload();
        let res = tokenizer.tokenize("?????????").unwrap();
        assert_eq!(res[0].tags[0], "NNP");
    }*/
    #[actix_rt::test]
    #[serial]
    async fn test_concurrent_jobs() {
        let srv = test_server();
        let search_reqs = (0..10u32).map(|_| {
            srv.get("/search?q=%ED%86%A9%ED%86%A9%ED%86%A9%0A")
                .timeout(std::time::Duration::from_secs(5))
                .send()
        });
        //let sync_reqs = srv.get("/search?q=%ED%86%A9%ED%86%A9%ED%86%A9%0A").timeout(std::time::Duration::from_secs(5)).send();
        let sync_reqs = (0..2u32).map(|_| {
            srv.post("/nouns")
                .timeout(std::time::Duration::from_secs(60))
                .send_body(bincode::serialize(&vec!["?????????"]).unwrap())
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
            .get("/search?q=%ED%86%A9%ED%86%A9%ED%86%A9%0A")
            .send()
            .await
            .unwrap();
        assert!(res.status().is_success());
        assert_eq!(
            String::from_utf8(res.body().await.unwrap().to_vec()).unwrap(),
            "?????????\tNNP,*,T,?????????,*,*,*,*\nEOS\n".to_string()
        );
    }
}
