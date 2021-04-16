use anyhow::{Result, Error};
use mecab::Tagger;
use std::path::Path;

pub fn has_support(c: char) -> bool {
    0xAC00 <= c as u32 && c as u32 <= 0xD7A3 && ((c as u32 - 0xAC00) % 28 != 0)
}
fn mecab_csv_nnp_format(nnp: &str) -> Result<String> {
    if nnp.len() == 0 { 
        return Err(Error::msg("NNP length 0"));
    }
    Ok(format!("{},,,,NNP,*,{},{},*,*,*,*", nnp, if has_support(nnp.chars().rev().nth(0).unwrap()) { "T" } else { "F" }, nnp))
}

pub struct Tokenizer {
    tagger: Tagger,
    mecab_dic_path: String,
}

impl Tokenizer {
    pub fn new(mecab_dic_path: String) -> Self {
        let tagger = Tagger::new("");
        Self {
            tagger,
            mecab_dic_path,
        }
    }
    pub fn tokenize(&self, q: &str) -> String {
        self.tagger.parse_str(q)
    }
    pub async fn gen_userdic(&self, nouns: Vec<String>) -> Result<()> {
        let path = self.mecab_dic_path.clone();
        blocking::unblock(move || -> Result<()>{
            let userdic_path = Path::new(&path).join("user-dic/rest-mecab.csv");
            std::fs::write(userdic_path, nouns.into_iter().filter_map(|n| mecab_csv_nnp_format(&n).ok()).collect::<Vec<_>>().join("\n"))?;
            let output = std::process::Command::new("sh")
                .current_dir(Path::new(&path))
                .args(&["-c", r#"./tools/add-userdic.sh && make && make install"#])
                .output()?;
            if !output.status.success() {
                return Err(anyhow::Error::msg(format!("Status: {:?}, Stdout: {:?}, Stderr: {:?}", output.status, String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))));
            }
            Ok(())
        }).await?;
        Ok(())
    }
    pub fn reload(&mut self) {
        self.tagger = Tagger::new("");
    }
}
