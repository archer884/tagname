use std::{borrow::Cow, ffi::OsString, fmt, path::Path, process, str::FromStr};

use audiotags::AudioTag;
use clap::Parser;
use regex::Regex;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    AudioTags(#[from] audiotags::Error),

    #[error("bad format key: {0}")]
    Format(String),

    #[error("missing required tag: {0}")]
    MissingTag(Tag),
}

#[derive(Debug, Clone, Copy)]
enum Tag {
    Album,
    Artist,
    Title,
    Track,
    Year,
}

impl Tag {
    fn read_from<'a>(self, meta: &'a Box<dyn AudioTag>) -> Result<Cow<'a, str>> {
        match self {
            Tag::Album => Ok(meta.album().ok_or(Error::MissingTag(self))?.title.into()),
            Tag::Artist => meta.artist().map(Cow::from).ok_or(Error::MissingTag(self)),
            Tag::Title => meta.title().map(Cow::from).ok_or(Error::MissingTag(self)),
            Tag::Track => Ok(meta
                .track_number()
                .ok_or(Error::MissingTag(self))?
                .to_string()
                .into()),
            Tag::Year => Ok(meta
                .year()
                .ok_or(Error::MissingTag(self))?
                .to_string()
                .into()),
        }
    }
}

impl FromStr for Tag {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim_start_matches('%');
        match s {
            "album" => Ok(Tag::Album),
            "artist" => Ok(Tag::Artist),
            "title" => Ok(Tag::Title),
            "track" => Ok(Tag::Track),
            "year" => Ok(Tag::Year),
            _ => Err(Error::Format(s.into())),
        }
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tag::Album => f.write_str("Album"),
            Tag::Artist => f.write_str("Artist"),
            Tag::Title => f.write_str("Title"),
            Tag::Track => f.write_str("Track"),
            Tag::Year => f.write_str("Year"),
        }
    }
}

#[derive(Debug, Parser)]
struct Args {
    template: String,
    paths: Vec<String>,
}

#[derive(Debug, Clone)]
enum Element {
    Tag(Tag),
    Literal(String),
}

#[derive(Debug, Clone)]
struct Format {
    elements: Vec<Element>,
}

impl Format {
    fn from_template(template: &str) -> Result<Self> {
        let rx = Regex::new(r#"(%[a-z]+)|([^%]+)"#).unwrap();
        let elements: Result<Vec<_>> = rx
            .captures_iter(template)
            .map(|cx| {
                if let Some(tag) = cx.get(1) {
                    tag.as_str().parse::<Tag>().map(Element::Tag)
                } else {
                    Ok(Element::Literal(cx.get(2).unwrap().as_str().into()))
                }
            })
            .collect();

        Ok(Self {
            elements: elements?,
        })
    }

    fn build_name(&self, meta: &Box<dyn AudioTag>) -> Result<String> {
        let mut f = String::new();

        for element in &self.elements {
            match element {
                Element::Tag(tag) => f += &*tag.read_from(meta)?,
                Element::Literal(lit) => f += lit,
            }
        }

        Ok(f)
    }
}

fn main() {
    if let Err(e) = run(Args::parse_from(wild::args())) {
        eprintln!("{e}");
        process::exit(1);
    }
}

fn run(args: Args) -> Result<()> {
    let format = Format::from_template(&args.template)?;
    for path in &args.paths {
        let path = Path::new(path);
        let meta = audiotags::Tag::new().read_from_path(path)?;

        let mut name = OsString::from(format.build_name(&meta)?);
        if let Some(extension) = path.extension() {
            name.push(".");
            name.push(extension);
        }

        let new_path = path.with_file_name(name);
        println!("{}", new_path.display());
    }
    Ok(())
}
