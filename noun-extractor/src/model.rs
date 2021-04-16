use crate::store::{Store, StoreImpl};
use anyhow::Result;
use lazy_static::lazy_static;
use log::debug;
//use serde::{Deserialize, Serialize};
use hashbrown::{HashMap, HashSet};
//use std::collections::HashMap;
use borsh::{BorshDeserialize as Deserialize, BorshSerialize as Serialize};
use std::path::Path;

#[derive(Deserialize, Serialize, Hash, PartialOrd, Eq, Debug, PartialEq, Default)]
pub struct Postfix {
    //target_has_support: bool,
    last_char: u32,
    postfix: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Default, Clone, Copy)]
pub struct Count {
    postnoun: u32,
    postother: u32,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Copy)]
pub struct Score {
    pub noun_probability: f32,
    pub count: u32,
    pub unique_postfixes_count: u32,
}
impl Score {
    pub fn new(noun_probability: f32, count: u32, unique_postfixes_count: u32) -> Self {
        Self {
            noun_probability,
            count,
            unique_postfixes_count,
        }
    }
}
impl Default for Score {
    fn default() -> Self {
        Self::new(1.0, 0, 0)
    }
}

lazy_static! {
    static ref OTHER_COUNT_KEY: String = "__other_count__".to_string();
    static ref NOUN_COUNT_KEY: String = "__noun_ount__".to_string();
}

const DEFAULT_SMOOTH_FACTOR: f64 = 0.5;

const MAX_POSTFIX_SIZE: usize = 3;

pub struct State {
    postfix_count_store: StoreImpl<Postfix, Count>,
    noun_count_store: StoreImpl<String, u32>,
    smooth_factor: f64,
}

impl State {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Ok(State {
            postfix_count_store: StoreImpl::open(path.as_ref().join("postfix"))?,
            noun_count_store: StoreImpl::open(path.as_ref().join("noun"))?,
            smooth_factor: DEFAULT_SMOOTH_FACTOR,
        })
    }
    pub fn set_smooth_factor(&mut self, f: f64) -> &mut Self {
        self.smooth_factor = f;
        self
    }
    pub fn save(&self) -> Result<()> {
        self.postfix_count_store.save()?;
        self.noun_count_store.save()?;
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
    pub fn observe_postnoun(&mut self, last_target_char: char, postfix: String) -> Result<()> {
        let mut noun_count: u32 = self
            .noun_count_store
            .get(&NOUN_COUNT_KEY)?
            .unwrap_or_default();
        noun_count += 1;
        self.noun_count_store
            .put(NOUN_COUNT_KEY.clone(), noun_count)?;

        let key = Postfix {
            last_char: last_target_char as u32,
            postfix: postfix.clone(),
        };
        let mut count: Count = self.postfix_count_store.get(&key)?.unwrap_or_default();
        count.postnoun += 1;
        self.postfix_count_store.put(key, count)?;

        let key = Postfix {
            last_char: '\0' as u32,
            postfix,
        };
        let mut count: Count = self.postfix_count_store.get(&key)?.unwrap_or_default();
        count.postnoun += 1;
        self.postfix_count_store.put(key, count)?;
        Ok(())
    }
    pub fn observe_postother(&mut self, last_target_char: char, postfix: String) -> Result<()> {
        let mut noun_count: u32 = self
            .noun_count_store
            .get(&OTHER_COUNT_KEY)?
            .unwrap_or_default();
        noun_count += 1;
        self.noun_count_store
            .put(OTHER_COUNT_KEY.clone(), noun_count)?;

        let key = Postfix {
            last_char: last_target_char as u32,
            postfix: postfix.clone(),
        };
        let mut count: Count = self.postfix_count_store.get(&key)?.unwrap_or_default();
        count.postother += 1;
        self.postfix_count_store.put(key, count)?;

        let key = Postfix {
            last_char: '\0' as u32,
            postfix,
        };
        let mut count: Count = self.postfix_count_store.get(&key)?.unwrap_or_default();
        count.postother += 1;
        self.postfix_count_store.put(key, count)?;
        Ok(())
    }
    fn postfix_noun_prob(
        &self,
        last_target_char: char,
        postfix: String,
        _noun_count: f64,
        _other_count: f64,
    ) -> Result<f64> {
        let key = Postfix {
            last_char: last_target_char as u32,
            postfix: postfix.clone(),
        };
        let count: Option<Count> = self.postfix_count_store.get(&key)?;
        let key = Postfix {
            last_char: '\0' as u32,
            postfix,
        };
        let count_without_lastchar: Option<Count> = self.postfix_count_store.get(&key)?;
        let with_lastchar = match count {
            Some(count) if count.postnoun + count.postother >= 100 => {
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
            Some(count) if count.postnoun + count.postother >= 100 => {
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
    pub fn extract_nouns2(&self, text: &str) -> Result<Vec<(String, Score)>> {
        let noun_count = self.noun_count_store.get(&NOUN_COUNT_KEY)?.unwrap_or(0u32) as f64;
        let other_count = self.noun_count_store.get(&OTHER_COUNT_KEY)?.unwrap_or(0u32) as f64;
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
                    //let postfix = chars[i + 1..i + 1 + j].iter().collect::<String>();
                    let word = (
                        chars[word_start_index..(i + 1 + j)].to_vec(),
                        j,
                    );
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
            let prob = self.postfix_noun_prob(
                word[word.len() - suffix_len - 1],
                suffix.clone(),
                noun_count,
                other_count,
            )?;
            debug!("{} ~ {:?}: {:?}", &candidate, &suffix, prob);
            let s = candidates.entry(candidate).or_insert(Score::new(0.0, 0, 0));
            s.noun_probability += count as f32 * prob as f32;
            if suffix_len == 1 {
                s.unique_postfixes_count += 1;
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
                        s.unique_postfixes_count,
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
                ((s2.count as f32).log10() * s2.noun_probability * s2.unique_postfixes_count as f32)
                    .partial_cmp(
                        &((s1.count as f32).log10()
                            * s1.noun_probability
                            * s1.unique_postfixes_count as f32),
                    )
                    .unwrap()
            }
        });
        Ok(res)
    }

    fn postfix_likelihood(
        &self,
        last_target_char: char,
        postfix: String,
        noun_count: f64,
        other_count: f64,
    ) -> Result<(f64, f64)> {
        let key = Postfix {
            last_char: last_target_char as u32,
            postfix: postfix.clone(),
        };
        let count: Option<Count> = self.postfix_count_store.get(&key)?;
        let key = Postfix {
            last_char: '\0' as u32,
            postfix,
        };
        let count_without_lastchar: Option<Count> = self.postfix_count_store.get(&key)?;
        let alpha = noun_count / (other_count + noun_count);
        let beta = other_count / (other_count + noun_count);
        let with_lastchar = match count {
            Some(count) if count.postnoun + count.postother >= 100 => (
                (alpha * self.smooth_factor + count.postnoun as f64)
                    / (self.smooth_factor + noun_count),
                (beta * self.smooth_factor + count.postother as f64)
                    / (self.smooth_factor + other_count),
            ),
            _ => (1.0, 1.0),
        };
        let without_lastchar = match count_without_lastchar {
            Some(count) if count.postnoun + count.postother >= 100 => (
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
    }
    pub fn extract_nouns(&self, text: &str) -> Result<Vec<(String, Score)>> {
        let noun_count: f32 = self.noun_count_store.get(&NOUN_COUNT_KEY)?.unwrap_or(0u32) as f32;
        let other_count: f32 = self.noun_count_store.get(&OTHER_COUNT_KEY)?.unwrap_or(0u32) as f32;
        let mut likelihoods = HashMap::new();
        let mut unique_postfixes = HashSet::new();
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
                    let postfix = chars[i + 1..i + 1 + j].iter().collect::<String>();
                    let (nl, ol) = self.postfix_likelihood(
                        candidate_last_char,
                        postfix,
                        noun_count as f64,
                        other_count as f64,
                    )?;
                    if j == 1 && ((nl - 1.0).abs() > f64::EPSILON && (ol - 1.0).abs() > f64::EPSILON) {
                        unique_postfixes.insert(
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
        unique_postfixes.into_iter().for_each(|up| {
            let chars = up.chars().collect::<Vec<_>>();
            let Score {
                unique_postfixes_count,
                ..
            } = likelihoods
                .entry(chars[..chars.len() - 1].iter().collect::<String>())
                .or_insert(Score::default());
            *unique_postfixes_count += 1;
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
                        score.unique_postfixes_count,
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
                ((s2.count as f32).log10() * s2.noun_probability * s2.unique_postfixes_count as f32)
                    .partial_cmp(
                        &((s1.count as f32).log10()
                            * s1.noun_probability
                            * s1.unique_postfixes_count as f32),
                    )
                    .unwrap()
            }
        });
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{tempdir, TempDir};

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
                .postfix_likelihood('가', "테스트1".to_string(), 2., 1.)
                .unwrap(),
            (0.5, 1.0)
        );
        assert_eq!(
            state
                .postfix_likelihood('가', "테스트2".to_string(), 2., 1.)
                .unwrap(),
            (0.5, 0.0)
        );
    }

    #[test]
    fn it_extract_nouns() {
        let dir = tempdir().unwrap();
        let mut state = State::open(dir.path()).unwrap();
        state.train("dataset/test.txt").unwrap();
        let text = std::fs::read_to_string("dataset/test.txt").unwrap();
        assert_eq!(state.extract_nouns(&text).unwrap(), Vec::new());
    }

    /*#[test]
    fn it_observe_postother() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("db");
        let store = StoreImpl::open(path).unwrap();
        store.put(&1, &2).unwrap();
    }*/
}
