use crate::store::{hashmap_store::StoreImpl as HashMapStoreImpl, Store};
use anyhow::Result;
use log::debug;
//use serde::{Deserialize, Serialize};
//use hashbrown::HashMap;
use std::collections::HashMap;
//use std::collections::HashMap;
use crate::util::has_support;
use hyperloglog::HyperLogLog;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Deserialize, Serialize, Hash, PartialOrd, Eq, Debug, PartialEq, Default)]
pub struct Suffix {
    target_has_support: bool,
    last_char: u32,
    suffix: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Default, Clone, Copy)]
pub struct Count {
    postnoun: u32,
    postother: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Score {
    pub noun_probability: f32,
    pub count: u32,
    pub unique_suffixes_hll: HyperLogLog,
    //pub unique_suffixes_count: u32,
}
impl Score {
    pub fn new(noun_probability: f32, count: u32, unique_suffixes_hll: HyperLogLog) -> Self {
        Self {
            noun_probability,
            count,
            unique_suffixes_hll,
        }
    }
    pub fn observe_suffix(&mut self, suffix: &str) {
        self.unique_suffixes_hll.insert(&suffix);
    }
    pub fn merge(&mut self, o: &Self) {
        self.count += o.count;
        self.unique_suffixes_hll.merge(&o.unique_suffixes_hll);
        self.noun_probability =
            1. / (1. + ((1. / self.noun_probability - 1.) * (1. / o.noun_probability - 1.)));
    }
}
impl Default for Score {
    fn default() -> Self {
        Self::new(1.0, 0, HyperLogLog::new(0.02))
    }
}

const DEFAULT_SMOOTH_FACTOR: f64 = 0.5;

const MAX_POSTFIX_SIZE: usize = 3;

const LARGE_NUMBER: u32 = 250;
// P(|X - M| > e) <= p(1-p)/(ne^2),
// e == 0.1 => n = 250

pub struct State {
    suffix_count_store: HashMapStoreImpl<Suffix, Count>,
    noun_count: u32,
    other_count: u32,
    path: std::path::PathBuf,
    smooth_factor: f64,
}

impl State {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Ok(State {
            suffix_count_store: HashMapStoreImpl::open(path.as_ref().join("suffix"))?,
            noun_count: std::fs::read_to_string(path.as_ref().join("noun_count"))
                .unwrap_or_else(|_| "0".to_string())
                .parse()?,
            other_count: std::fs::read_to_string(path.as_ref().join("other_count"))
                .unwrap_or_else(|_| "0".to_string())
                .parse()?,
            smooth_factor: DEFAULT_SMOOTH_FACTOR,
            path: path.as_ref().to_path_buf(),
        })
    }
    pub fn set_smooth_factor(&mut self, f: f64) -> &mut Self {
        self.smooth_factor = f;
        self
    }
    pub fn save(&self) -> Result<()> {
        self.suffix_count_store.save()?;
        std::fs::write(
            self.path.clone().join("noun_count"),
            self.noun_count.to_string(),
        )?;
        std::fs::write(
            self.path.clone().join("other_count"),
            self.other_count.to_string(),
        )?;
        Ok(())
    }

    pub fn train_line_bytes_pos(&mut self, text: &str, noun_poses: &[(u32, u32)]) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        let mut noun_idx = -1 + (!noun_poses.is_empty() as i32);
        /*for i in 1..text.len() {
        }*/
        let chars = text
            .chars()
            .chain(std::iter::once('\n'))
            .collect::<Vec<_>>();
        for i in 1..chars.len() {
            if noun_idx >= 0
                && noun_idx < (noun_poses.len() as i32)
                && i as u32 == noun_poses[noun_idx as usize].0 + noun_poses[noun_idx as usize].1
            {
                for j in 1..MAX_POSTFIX_SIZE.min(chars.len() - i) {
                    self.observe_postnoun(
                        chars[i - 1],
                        chars[i..i + j].iter().collect::<String>(),
                    )?;
                }
                noun_idx += 1;
            } else {
                for j in 1..MAX_POSTFIX_SIZE.min(chars.len() - i) {
                    self.observe_postother(
                        chars[i - 1],
                        chars[i..i + j].iter().collect::<String>(),
                    )?;
                }
            }
        }
        Ok(())
    }
    pub fn train_line(&mut self, text: &str, noun_poses: &[(u32, u32)]) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        let mut noun_idx = -1 + (!noun_poses.is_empty() as i32);
        let chars = text
            .chars()
            .chain(std::iter::once('\n'))
            .collect::<Vec<_>>();
        for i in 1..chars.len() {
            if noun_idx >= 0
                && noun_idx < (noun_poses.len() as i32)
                && i as u32 == noun_poses[noun_idx as usize].0 + noun_poses[noun_idx as usize].1
            {
                for j in 1..MAX_POSTFIX_SIZE.min(chars.len() - i) {
                    self.observe_postnoun(
                        chars[i - 1],
                        chars[i..i + j].iter().collect::<String>(),
                    )?;
                }
                noun_idx += 1;
            } else {
                for j in 1..MAX_POSTFIX_SIZE.min(chars.len() - i) {
                    self.observe_postother(
                        chars[i - 1],
                        chars[i..i + j].iter().collect::<String>(),
                    )?;
                }
            }
        }
        Ok(())
    }
    pub fn train<P: AsRef<Path>>(&mut self, dataset_path: P) -> Result<()> {
        let total_size = std::fs::metadata(&dataset_path)?.len();
        let mut read_size = 0usize;
        for (i, line) in String::from_utf8(std::fs::read(dataset_path)?)?
            .lines()
            .enumerate()
        {
            read_size += line.len();
            if line.is_empty() {
                continue;
            }
            let data: (String, Vec<(u32, u32)>) = serde_json::from_str(&line)?;
            let (text, noun_poses) = data;
            self.train_line(&text, &noun_poses)?;
            if i % 100 == 0 {
                print!(
                    "\rprocessed bytes: {} / {} ({}%)",
                    read_size,
                    total_size,
                    read_size * 100 / total_size as usize
                );
            }
        }
        Ok(())
    }
    pub fn observe_postnoun(&mut self, last_target_char: char, suffix: String) -> Result<()> {
        self.noun_count += 1;

        let key = Suffix {
            target_has_support: has_support(last_target_char),
            last_char: last_target_char as u32,
            suffix: suffix.clone(),
        };
        let mut count: Count = self.suffix_count_store.get(&key)?.unwrap_or_default();
        count.postnoun += 1;
        self.suffix_count_store.put(key, count)?;

        let key = Suffix {
            target_has_support: has_support(last_target_char),
            last_char: '\0' as u32,
            suffix,
        };
        let mut count: Count = self.suffix_count_store.get(&key)?.unwrap_or_default();
        count.postnoun += 1;
        self.suffix_count_store.put(key, count)?;
        Ok(())
    }
    pub fn observe_postother(&mut self, last_target_char: char, suffix: String) -> Result<()> {
        self.other_count += 1;

        let key = Suffix {
            target_has_support: has_support(last_target_char),
            last_char: last_target_char as u32,
            suffix: suffix.clone(),
        };
        let mut count: Count = self.suffix_count_store.get(&key)?.unwrap_or_default();
        count.postother += 1;
        self.suffix_count_store.put(key, count)?;

        let key = Suffix {
            target_has_support: has_support(last_target_char),
            last_char: '\0' as u32,
            suffix,
        };
        let mut count: Count = self.suffix_count_store.get(&key)?.unwrap_or_default();
        count.postother += 1;
        self.suffix_count_store.put(key, count)?;
        Ok(())
    }

    pub fn is_valid_suffix(&self, key: &Suffix) -> bool {
        if let Ok(Some(count)) = self.suffix_count_store.get(&key) {
            count.postnoun + count.postother >= LARGE_NUMBER
        } else {
            false
        }
    }

    fn suffix_noun_prob2(&self, last_target_char: char, suffix: String) -> Result<f64> {
        let key = Suffix {
            target_has_support: has_support(last_target_char),
            last_char: last_target_char as u32,
            suffix: suffix.clone(),
        };
        let count: Option<Count> = self.suffix_count_store.get(&key)?;
        let key = Suffix {
            target_has_support: has_support(last_target_char),
            last_char: '\0' as u32,
            suffix,
        };
        let count_without_lastchar: Option<Count> = self.suffix_count_store.get(&key)?;
        let with_lastchar = match count {
            Some(count) if count.postnoun + count.postother >= LARGE_NUMBER => {
                ((count.postother as f64 + self.smooth_factor)
                    / ((count.postnoun + count.postother) as f64 + self.smooth_factor))
                    .ln()
                    - ((count.postnoun as f64 + self.smooth_factor)
                        / ((count.postnoun + count.postother) as f64 + self.smooth_factor))
                        .ln()
            }
            _ => 0.0,
        };
        let without_lastchar = match count_without_lastchar {
            Some(count) if count.postnoun + count.postother >= LARGE_NUMBER => {
                ((count.postother as f64 + self.smooth_factor)
                    / ((count.postnoun + count.postother) as f64 + self.smooth_factor))
                    .ln()
                    - ((count.postnoun as f64 + self.smooth_factor)
                        / ((count.postnoun + count.postother) as f64 + self.smooth_factor))
                        .ln()
            }
            _ => 0.0,
        };
        Ok(with_lastchar + without_lastchar)
    }

    fn suffix_noun_prob1(&self, last_target_char: char, suffix: String) -> Result<f64> {
        let noun_count = self.noun_count as f64;
        let other_count = self.other_count as f64;
        let key = Suffix {
            target_has_support: has_support(last_target_char),
            last_char: last_target_char as u32,
            suffix: suffix.clone(),
        };
        let count: Option<Count> = self.suffix_count_store.get(&key)?;
        let key = Suffix {
            target_has_support: has_support(last_target_char),
            last_char: '\0' as u32,
            suffix,
        };
        let count_without_lastchar: Option<Count> = self.suffix_count_store.get(&key)?;
        let alpha = noun_count / (other_count + noun_count);
        let beta = other_count / (other_count + noun_count);
        let with_lastchar = match count {
            Some(count) if count.postnoun + count.postother >= LARGE_NUMBER => {
                ((beta * self.smooth_factor + count.postother as f64)
                    / (self.smooth_factor + other_count))
                    .ln()
                    - ((alpha * self.smooth_factor + count.postnoun as f64)
                        / (self.smooth_factor + noun_count))
                        .ln()
            }
            _ => 0.0,
        };
        let without_lastchar = match count_without_lastchar {
            Some(count) if count.postnoun + count.postother >= LARGE_NUMBER => {
                ((beta * self.smooth_factor + count.postother as f64)
                    / (self.smooth_factor + other_count))
                    .ln()
                    - ((alpha * self.smooth_factor + count.postnoun as f64)
                        / (self.smooth_factor + noun_count))
                        .ln()
            }
            _ => 0.0,
        };
        Ok(with_lastchar + without_lastchar)
    }

    /*fn suffix_likelihood(
        &self,
        last_target_char: char,
        suffix: String,
    ) -> Result<(f64, f64)> {
        let noun_count = self.noun_count as f64;
        let other_count = self.other_count as f64;
        let key = Suffix {
            target_has_support: has_support(last_target_char),
            last_char: last_target_char as u32,
            suffix: suffix.clone(),
        };
        let count: Option<Count> = self.suffix_count_store.get(&key)?;
        let key = Suffix {
            target_has_support: has_support(last_target_char),
            last_char: '\0' as u32,
            suffix,
        };
        let count_without_lastchar: Option<Count> = self.suffix_count_store.get(&key)?;
        let alpha = noun_count / (other_count + noun_count);
        let beta = other_count / (other_count + noun_count);
        let with_lastchar = match count {
            Some(count) if count.postnoun + count.postother >= LARGE_NUMBER => (
                (alpha * self.smooth_factor + count.postnoun as f64)
                    / (self.smooth_factor + noun_count),
                (beta * self.smooth_factor + count.postother as f64)
                    / (self.smooth_factor + other_count),
            ),
            _ => (1.0, 1.0),
        };
        let without_lastchar = match count_without_lastchar {
            Some(count) if count.postnoun + count.postother >= LARGE_NUMBER => (
                (alpha * self.smooth_factor + count.postnoun as f64)
                    / (self.smooth_factor + noun_count),
                (beta * self.smooth_factor + count.postother as f64)
                    / (self.smooth_factor + other_count),
            ),
            _ => (1.0, 1.0),
        };
        Ok((
            with_lastchar.0 * without_lastchar.0,
            with_lastchar.1 * without_lastchar.1,
        ))
        //Ok((without_lastchar.0, without_lastchar.1))
    }*/
    pub fn extract_nouns2(&self, text: &str) -> Result<Vec<(String, Score)>> {
        let mut words = HashMap::new();
        let total_size = text.len();
        let mut read_size = 0usize;
        for (i, line) in text.lines().enumerate() {
            read_size += line.len();
            if line.is_empty() {
                continue;
            }
            let chars = line
                .chars()
                .chain(std::iter::once('\n'))
                .collect::<Vec<_>>();
            let mut word_start_index = 0usize;
            for i in 0..chars.len() {
                if chars[i].is_whitespace()
                    || (!chars[i].is_alphanumeric() && ('ㄱ' > chars[i] || chars[i] > '힣'))
                {
                    word_start_index = i + 1;
                    continue;
                }
                for j in 1..MAX_POSTFIX_SIZE.min(chars.len() - i - 1) {
                    //let suffix = chars[i + 1..i + 1 + j].iter().collect::<String>();
                    let word = (chars[word_start_index..(i + 1 + j)].to_vec(), j);
                    *words.entry(word).or_insert(0) += 1;
                }
            }
            if i % 100 == 0 {
                print!(
                    "\rprocessed bytes: {} / {} ({}%)",
                    read_size,
                    total_size,
                    read_size * 100 / total_size as usize
                );
            }
        }
        let mut candidates = HashMap::new();
        for ((word, suffix_len), count) in words.into_iter() {
            let candidate = word[..word.len() - suffix_len].iter().collect();
            let suffix = word[word.len() - suffix_len..].iter().collect::<String>();
            let prob = self.suffix_noun_prob2(word[word.len() - suffix_len - 1], suffix.clone())?;
            debug!("{} ~ {:?}: {:?}({:?})", &candidate, &suffix, prob, count);
            let s = candidates.entry(candidate).or_insert_with(Score::default);
            s.noun_probability += count as f32 * prob as f32;
            if suffix_len == 1 && prob != 0.0 {
                s.observe_suffix(&suffix);
            }
            s.count += count;
        }
        let mut res = candidates
            .into_iter()
            .map(|(key, s)| {
                (
                    key,
                    Score::new(
                        1.0 / (1.0 + s.noun_probability.exp()),
                        s.count,
                        s.unique_suffixes_hll,
                    ),
                )
            })
            .collect::<Vec<_>>();
        res.sort_by(|(_, s1), (_, s2)| {
            if s1.noun_probability.is_nan() && s2.noun_probability.is_nan() {
                std::cmp::Ordering::Equal
            } else if s1.noun_probability.is_nan() {
                std::cmp::Ordering::Greater
            } else if s2.noun_probability.is_nan() {
                std::cmp::Ordering::Less
            } else {
                ((s2.count as f32).log10()
                    * s2.noun_probability
                    * s2.unique_suffixes_hll.len() as f32)
                    .partial_cmp(
                        &((s1.count as f32).log10()
                            * s1.noun_probability
                            * s1.unique_suffixes_hll.len() as f32),
                    )
                    .unwrap()
            }
        });
        Ok(res)
    }

    pub fn extract_nouns(&self, text: &str) -> Result<Vec<(String, Score)>> {
        let mut words = HashMap::new();
        let total_size = text.len();
        let mut read_size = 0usize;
        for (i, line) in text.lines().enumerate() {
            read_size += line.len();
            if line.is_empty() {
                continue;
            }
            let chars = line
                .chars()
                .chain(std::iter::once('\n'))
                .collect::<Vec<_>>();
            let mut word_start_index = 0usize;
            for i in 0..chars.len() {
                if chars[i].is_whitespace()
                    || (!chars[i].is_alphanumeric() && ('ㄱ' > chars[i] || chars[i] > '힣'))
                {
                    word_start_index = i + 1;
                    continue;
                }
                for j in 1..MAX_POSTFIX_SIZE.min(chars.len() - i - 1) {
                    //let suffix = chars[i + 1..i + 1 + j].iter().collect::<String>();
                    let word = (chars[word_start_index..(i + 1 + j)].to_vec(), j);
                    *words.entry(word).or_insert(0) += 1;
                }
            }
            if i % 100 == 0 {
                print!(
                    "\rprocessed bytes: {} / {} ({}%)",
                    read_size,
                    total_size,
                    read_size * 100 / total_size as usize
                );
            }
        }
        let mut candidates = HashMap::new();
        for ((word, suffix_len), count) in words.into_iter() {
            let candidate = word[..word.len() - suffix_len].iter().collect();
            let suffix = word[word.len() - suffix_len..].iter().collect::<String>();
            let prob = self.suffix_noun_prob1(word[word.len() - suffix_len - 1], suffix.clone())?
                + self.suffix_noun_prob1(
                    word[word.len() - suffix_len - 1],
                    " ".to_string() + suffix.as_str(),
                )?;
            debug!("{} ~ {:?}: {:?}({:?})", &candidate, &suffix, prob, count);
            let s = candidates.entry(candidate).or_insert_with(Score::default);
            s.noun_probability += count as f32 * prob as f32;
            if suffix_len == 1 && prob != 0.0 {
                s.observe_suffix(&suffix);
            }
            s.count += count;
        }
        let mut res = candidates
            .into_iter()
            .map(|(key, s)| {
                (
                    key,
                    Score::new(
                        1.0 / (1.0
                            + (self.other_count as f32 / self.noun_count as f32)
                                * s.noun_probability.exp()),
                        s.count,
                        s.unique_suffixes_hll,
                    ),
                )
            })
            .collect::<Vec<_>>();
        res.sort_by(|(_, s1), (_, s2)| {
            if s1.noun_probability.is_nan() && s2.noun_probability.is_nan() {
                std::cmp::Ordering::Equal
            } else if s1.noun_probability.is_nan() {
                std::cmp::Ordering::Greater
            } else if s2.noun_probability.is_nan() {
                std::cmp::Ordering::Less
            } else {
                ((s2.count as f32).log10()
                    * s2.noun_probability
                    * s2.unique_suffixes_hll.len() as f32)
                    .partial_cmp(
                        &((s1.count as f32).log10()
                            * s1.noun_probability
                            * s1.unique_suffixes_hll.len() as f32),
                    )
                    .unwrap()
            }
        });
        Ok(res)
    }

    /*{
        let noun_count: f32 = self.noun_count_store.get(&NOUN_COUNT_KEY)?.unwrap_or(0u32) as f32;
        let other_count: f32 = self.noun_count_store.get(&OTHER_COUNT_KEY)?.unwrap_or(0u32) as f32;
        let mut likelihoods = HashMap::new();
        let mut unique_suffixes = HashSet::new();
        let total_size = text.len();
        let mut read_size = 0usize;
        for (i, line) in text.lines().enumerate() {
            read_size += line.len();
            if line.is_empty() {
                continue;
            }
            let chars = line
                .chars()
                .chain(std::iter::once('\n'))
                .collect::<Vec<_>>();
            let mut word_start_index = 0usize;
            for i in 0..chars.len() {
                if chars[i].is_whitespace()
                    || (!chars[i].is_alphanumeric() && ('ㄱ' > chars[i] || chars[i] > '힣'))
                {
                    word_start_index = i + 1;
                    continue;
                }
                let candidate = chars[word_start_index..i + 1].iter().collect::<String>();
                let candidate_last_char = chars[i];
                for j in 1..MAX_POSTFIX_SIZE.min(chars.len() - i - 1) {
                    let suffix = chars[i + 1..i + 1 + j].iter().collect::<String>();
                    let (nl, ol) = self.suffix_likelihood(
                        candidate_last_char,
                        suffix,
                        noun_count as f64,
                        other_count as f64,
                    )?;
                    if j == 1 && ((nl - 1.0).abs() > f64::EPSILON && (ol - 1.0).abs() > f64::EPSILON) {
                        unique_suffixes.insert(
                            chars[word_start_index..(i + 2).min(chars.len())]
                                .iter()
                                .collect::<String>(),
                        );
                    }
                    let Score {
                        noun_probability, ..
                    } = likelihoods
                        .entry(candidate.clone())
                        .or_insert(Score::default());
                    *noun_probability *= (nl / ol) as f32;
                    debug!(
                        "{} ~ {:?}: {:?} -> {:?}",
                        candidate,
                        &chars[i + 1..i + 1 + j],
                        nl / ol,
                        noun_probability
                    );
                }
                let Score { count, .. } = likelihoods
                    .entry(candidate.clone())
                    .or_insert(Score::default());
                *count += 1;
            }
            if i % 100 == 0 {
                print!(
                    "\rprocessed bytes: {} / {} ({}%)",
                    read_size,
                    total_size,
                    read_size * 100 / total_size as usize
                );
            }
        }
        unique_suffixes.into_iter().for_each(|up| {
            let chars = up.chars().collect::<Vec<_>>();
            let Score {
                unique_suffixes_count,
                ..
            } = likelihoods
                .entry(chars[..chars.len() - 1].iter().collect::<String>())
                .or_insert(Score::default());
            *unique_suffixes_count += 1;
        });
        let mut res = likelihoods
            .into_iter()
            .map(|(key, score)| {
                (
                    key,
                    Score::new(
                        (score.noun_probability * noun_count).min(f32::MAX)
                            / (score.noun_probability * noun_count + other_count).min(f32::MAX),
                        score.count,
                        score.unique_suffixes_count,
                    ),
                )
            })
            .collect::<Vec<_>>();
        res.sort_by(|(_, s1), (_, s2)| {
            if s1.noun_probability.is_nan() && s2.noun_probability.is_nan() {
                std::cmp::Ordering::Equal
            } else if s1.noun_probability.is_nan() {
                std::cmp::Ordering::Greater
            } else if s2.noun_probability.is_nan() {
                std::cmp::Ordering::Less
            } else {
                ((s2.count as f32).log10() * s2.noun_probability * s2.unique_suffixes_count as f32)
                    .partial_cmp(
                        &((s1.count as f32).log10()
                            * s1.noun_probability
                            * s1.unique_suffixes_count as f32),
                    )
                    .unwrap()
            }
        });
        Ok(res)
    }*/
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn it_has_support() {
        assert_eq!(has_support('가'), false);
        assert_eq!(has_support('갘'), true);
        assert_eq!(has_support('히'), false);
        assert_eq!(has_support('힣'), true);
        assert_eq!(has_support('1'), false);
        assert_eq!(has_support(' '), false);
        assert_eq!(has_support('Z'), false);
    }

    #[test]
    fn it_observes_and_calcaulate_likelihood() {
        let dir = tempdir().unwrap();
        let mut state = State::open(dir.path()).unwrap();
        state.observe_postnoun('가', "테스트1".to_string()).unwrap();
        state
            .observe_postother('가', "테스트1".to_string())
            .unwrap();
        state.observe_postnoun('가', "테스트2".to_string()).unwrap();
        assert_eq!(
            state
                .suffix_noun_prob2('가', "테스트1".to_string())
                .unwrap(),
            0.5,
        );
        assert_eq!(
            state
                .suffix_noun_prob2('가', "테스트2".to_string())
                .unwrap(),
            0.5,
        );
    }

    #[test]
    fn it_extract_nouns() {
        let dir = tempdir().unwrap();
        let mut state = State::open(dir.path()).unwrap();
        state.train("dataset/test.txt").unwrap();
        let text = std::fs::read_to_string("dataset/test.txt").unwrap();
        //assert_eq!(state.extract_nouns(&text).unwrap(), Vec::new());
    }

    /*#[test]
    fn it_observe_postother() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("db");
        let store = StoreImpl::open(path).unwrap();
        store.put(&1, &2).unwrap();
    }*/
}
