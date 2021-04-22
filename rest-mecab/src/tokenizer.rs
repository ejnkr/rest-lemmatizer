use anyhow::{Error, Result};
use hangul_normalize::{control_chars, derepeat, whitespace_less};
use mecab::Tagger;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub fn has_support(c: char) -> bool {
    0xAC00 <= c as u32 && c as u32 <= 0xD7A3 && ((c as u32 - 0xAC00) % 28 != 0)
}
fn mecab_csv_nnp_format(nnp: &str) -> Result<String> {
    if nnp.is_empty() {
        return Err(Error::msg("NNP length 0"));
    }
    Ok(format!(
        "{},,,,NNP,*,{},{},*,*,*,*",
        nnp,
        if has_support(nnp.chars().rev().next().unwrap()) {
            "T"
        } else {
            "F"
        },
        nnp
    ))
}

pub struct Tokenizer {
    tagger: Tagger,
    mecab_dic_path: PathBuf,
}

unsafe impl Send for Tokenizer {}
unsafe impl Sync for Tokenizer {}

fn asterisk_as_none(s: String) -> Option<String> {
    if s == "*" {
        None
    } else {
        Some(s)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Morpheme {
    token: String,
    tag: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Analytics {
    pub token: String,
    pub tags: Vec<String>,
    pub symantic_group: Option<String>,
    pub has_support: Option<bool>,
    pub pronounce: Option<String>,
    pub kind: Option<String>,
    pub left_tag: Option<String>,
    pub right_tag: Option<String>,
    pub morphemes: Option<Vec<Morpheme>>,
}
impl Analytics {
    pub fn parse(s: &str) -> Result<Self> {
        let mut sp = s.split('\t');
        let token = sp
            .next()
            .ok_or_else(|| anyhow::Error::msg(s.to_string()))?
            .to_string();
        let mut sp = sp
            .next()
            .ok_or_else(|| anyhow::Error::msg(s.to_string()))?
            .split(',');
        let tags = sp
            .next()
            .ok_or_else(|| anyhow::Error::msg(s.to_string()))?
            .split('+')
            .map(|s| s.to_string())
            .collect();
        let symantic_group = asterisk_as_none(
            sp.next()
                .ok_or_else(|| anyhow::Error::msg(s.to_string()))?
                .to_string(),
        );
        let has_support = match sp.next().ok_or_else(|| anyhow::Error::msg(s.to_string()))? {
            "T" => Some(true),
            "F" => Some(false),
            _ => None,
        };
        let pronounce = asterisk_as_none(
            sp.next()
                .ok_or_else(|| anyhow::Error::msg(s.to_string()))?
                .to_string(),
        );
        let kind = asterisk_as_none(
            sp.next()
                .ok_or_else(|| anyhow::Error::msg(s.to_string()))?
                .to_string(),
        );
        let left_tag = asterisk_as_none(
            sp.next()
                .ok_or_else(|| anyhow::Error::msg(s.to_string()))?
                .to_string(),
        );
        let right_tag = asterisk_as_none(
            sp.next()
                .ok_or_else(|| anyhow::Error::msg(s.to_string()))?
                .to_string(),
        );
        let morphemes = sp.next().ok_or_else(|| anyhow::Error::msg(s.to_string()))?;
        let morphemes = if morphemes == "*" {
            None
        } else {
            Some(
                morphemes
                    .split('+')
                    .map(|s| {
                        let mut splited = s.split('/');
                        match (splited.next(), splited.next()) {
                            (Some(token), Some(tag)) => Ok(Morpheme {
                                token: token.to_string(),
                                tag: tag.to_string(),
                            }),
                            _ => Err(anyhow::Error::msg(s.to_string())),
                        }
                    })
                    .collect::<Result<Vec<Morpheme>>>()?,
            )
        };
        Ok(Analytics {
            token,
            tags,
            symantic_group,
            has_support,
            pronounce,
            kind,
            left_tag,
            right_tag,
            morphemes,
        })
    }
}

impl Tokenizer {
    pub fn new<P: AsRef<Path>>(mecab_dic_path: P) -> Self {
        let tagger = Tagger::new("");
        Self {
            tagger,
            mecab_dic_path: mecab_dic_path.as_ref().to_path_buf(),
        }
    }
    pub fn tokenize(&self, q: &str) -> Result<Vec<Analytics>> {
        let s = control_chars(&q, "_");
        let s = whitespace_less(&s);
        let s = derepeat(&s, 3);
        self.tagger
            .parse_str(s)
            .lines()
            .filter_map(|l| {
                if l != "EOS" {
                    Some(Analytics::parse(l))
                } else {
                    None
                }
            })
            .collect()
    }
    pub async fn gen_userdic(&self, nouns: Vec<String>) -> Result<()> {
        let path = self.mecab_dic_path.clone();
        blocking::unblock(move || -> Result<()> {
            let userdic_path = Path::new(&path).join("user-dic/rest-mecab.csv");
            std::fs::write(
                userdic_path,
                nouns
                    .into_iter()
                    .filter_map(|n| mecab_csv_nnp_format(&n).ok())
                    .collect::<Vec<_>>()
                    .join("\n"),
            )?;
            let output = std::process::Command::new("bash")
                .current_dir(Path::new(&path))
                .args(&["-c", r#"./tools/add-userdic.sh && make && make install"#])
                .output()?;
            if !output.status.success() {
                return Err(anyhow::Error::msg(format!(
                    "Status: {:?}, Stdout: {:?}, Stderr: {:?}",
                    output.status,
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )));
            }
            Ok(())
        })
        .await?;
        Ok(())
    }
    pub fn reload(&mut self) {
        self.tagger = Tagger::new("");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokenize() {
        let tok = Tokenizer::new("");
        let res = tok.tokenize("안녕 반가워").unwrap();
        assert_eq!(format!("{:?}", res), "hi");
    }
}
