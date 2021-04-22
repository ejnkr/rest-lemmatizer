use clap::Clap;
use derive_more::From;
use std::{
    fs::File,
    io::{stdin, stdout, BufRead, BufReader, Read, Stdout, Write},
};

mod lib;

use lib::*;

#[derive(From)]
enum Writer {
    File(File),
    Stdout(Stdout),
}
impl Writer {
    fn write_all(&mut self, text: &[u8]) -> anyhow::Result<()> {
        match self {
            Writer::File(writer) => writer.write_all(text),
            Writer::Stdout(writer) => writer.write_all(text),
        }?;
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let opts: Opts = Opts::parse();
    let mut writer: Writer = if let Some(output_file_path) = &opts.output_file_path {
        let file = File::create(&output_file_path)?;
        file.into()
    } else {
        stdout().into()
    };
    if let Some(input_file_path) = &opts.input_file_path {
        let file = File::open(&input_file_path)?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line?;
            writer.write_all(normalize(line, &opts).as_bytes())?;
            writer.write_all(b"\n")?;
        }
    } else {
        let mut buffer = String::new();
        stdin().read_to_string(&mut buffer)?;
        writer.write_all(normalize(buffer, &opts).as_bytes())?;
    };
    Ok(())
}
