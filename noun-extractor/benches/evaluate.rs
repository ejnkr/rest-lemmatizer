extern crate noun_extractor;
use noun_extractor::model::State;
use std::collections::HashSet;
use std::time::Instant;
use tempfile::tempdir;
fn main() {
    let dir = tempdir().unwrap();
    let mut state = State::open(dir.path()).unwrap();
    let (test_data, expected_nouns) = eval_dataset("dataset/nng_nnp.txt");
    let start = Instant::now();
    state.train("dataset/nng_nnp.txt").unwrap();
    let train_duration = start.elapsed();
    state.save().unwrap();
    let start = Instant::now();
    let state = State::open(dir.path()).unwrap();
    let load_duration = start.elapsed();
    let start = Instant::now();
    let result = state.extract_nouns(&test_data).unwrap();
    let extract_duration = start.elapsed();
    let mut false_negative_error = 0.0;
    let mut false_positive_error = 0.0;
    for (noun, score) in result {
        if expected_nouns.contains(&noun) {
            false_negative_error += 1.0 - score.noun_probability;
        } else {
            false_positive_error += score.noun_probability;
        }
    }
    println!();
    println!("Train duration: {:?}", train_duration);
    println!("Load duration: {:?}", load_duration);
    println!("\nv1");
    println!("Extract duration: {:?}", extract_duration);
    println!("False Positive Error: {}", false_positive_error);
    println!("False Negative Error: {}", false_negative_error);
    println!("Total: {}", false_negative_error + false_positive_error);

    let start = Instant::now();
    let result = state.extract_nouns2(&test_data).unwrap();
    let extract_duration = start.elapsed();
    let mut false_negative_error = 0.0;
    let mut false_positive_error = 0.0;
    for (noun, score) in result {
        if expected_nouns.contains(&noun) {
            false_negative_error += 1.0 - score.noun_probability;
        } else {
            false_positive_error += score.noun_probability;
        }
    }
    println!("\nv1");
    println!("Extract duration: {:?}", extract_duration);
    println!("False Positive Error: {}", false_positive_error);
    println!("False Negative Error: {}", false_negative_error);
    println!("Total: {}", false_negative_error + false_positive_error);
}

fn eval_dataset(path: &str) -> (String, HashSet<String>) {
    let mut lines = Vec::new();
    let mut nouns = HashSet::new();
    for line in String::from_utf8(std::fs::read(path).unwrap())
        .unwrap()
        .lines()
    {
        if line.is_empty() {
            continue;
        }
        let data: (String, Vec<(usize, usize)>) = serde_json::from_str(&line).unwrap();
        if data.0.is_empty() {
            continue;
        }
        let chars = data.0.chars().collect::<Vec<_>>();
        for (i, (l, len)) in data.1.iter().enumerate() {
            if l + len > chars.len() {
                break;
            }
            let mut k = i;
            while k >= 1 && (data.1[k - 1].0 + data.1[k - 1].1 == data.1[k].0) {
                k -= 1;
            }
            nouns.insert(chars[data.1[k].0..l + len].iter().collect::<String>());
            //nouns.insert(chars[*l..l + len].iter().collect::<String>());
        }
        lines.push(data.0);
    }
    (lines.join("\n"), nouns)
}
