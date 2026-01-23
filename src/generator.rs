use itertools::Itertools;
use proc_macro2::TokenStream;
use std::{ffi::OsStr, fmt::{self, Debug}, fs::{self}, path::{Path, PathBuf}};

use crate::{logger::Logger, parser::{CharRect, Parser}, tokens::{FileToken, Token}};
use crate::tokens::RootToken;

pub fn generate_bluebrick_bindings(bb_dir: &Path, rust_dir: &Path) -> Result<()> {
    let generator = Generator::new(bb_dir, rust_dir)?;
    generator.generate()
}

struct Generator {
    logger: Logger,
    bb_dir: PathBuf,
    rust_dir: PathBuf,
}

impl Generator {
    fn new(bb_dir: &Path, rust_dir: &Path) -> Result<Self> {
        let logger= Logger::new(bb_dir);

        Ok(Self {
            logger,
            bb_dir: bb_dir.to_owned(),
            rust_dir: rust_dir.to_owned(),
        })
    }

    fn generate(&self) -> Result<()> {
        let tokens = self.parse_bb_bindings();
        let tokens = self.validate_results(tokens, "Parsed", &|item| &item.name)?;

        let (rust_files, mut compute_errs) = self.compute_rust(tokens);
        self.logger.log("generated", &format!("Generated {} files\n{}", rust_files.len(), compute_errs.iter().join("\n")));
        if let Some(last_err) = compute_errs.pop() {
            Err(last_err)?;
        }

        Ok(())
    }

    fn parse_bb_bindings(&self) -> Vec<Result<FileToken>> {
        self.walk_all_bb_files(&self.bb_dir.clone(), &|file| {
            let mut parser = Parser::new(file, &self.logger);
            parser.token()
        })
    }

    fn walk_all_bb_files(&self, dir: &Path, action: &impl Fn(BBFile) -> Result<FileToken>) -> Vec<Result<FileToken>> {
        let mut file_tokens = Vec::new();
        if !dir.is_dir() {
            self.logger.elog("walk_all_bb_files", &format!("{} is not a directory", dir.display()));
            return file_tokens;
        }

        for entry in match dir.read_dir() {
            Ok(read_dir) => read_dir,
            Err(e) => {
                self.logger.elog("walk_all_bb_files", &format!("Failed to read contents from {}: {}", dir.display(), e));
                return file_tokens;
            }
        } {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    self.logger.elog("walk_all_bb_files", &format!("Failed to read entry from {}: {}", dir.display(), e));
                    continue;
                }
            }.path();

            if entry.is_dir() {
                let mut inner = self.walk_all_bb_files(&entry, action);
                file_tokens.append(&mut inner);
            }
            else if entry.extension() == Some(&OsStr::new("bb")) {
                let name = match entry.file_stem() {
                    Some(name) => name,
                    None => {
                        self.logger.elog("walk_all_bb_files", &format!("Failed to get file name from {}", entry.display()));
                        continue;
                    }
                }.to_string_lossy().to_string();
                let root_dir = Path::new(&self.bb_dir).canonicalize().unwrap();
                let path = entry.strip_prefix(root_dir).unwrap().to_string_lossy().to_string();
                let contents = match fs::read_to_string(&entry) {
                    Ok(contents) => contents,
                    Err(e) => {
                        self.logger.elog("walk_all_bb_files", &format!("Failed to read contents from {}: {}", entry.display(), e));
                        continue;
                    }
                };
                file_tokens.push(action(BBFile { name, path, contents }));
            }
        }
        file_tokens
    }

    fn compute_rust(&self, tokens: Vec<FileToken>) -> (Vec<RustFile>, Vec<Error>) {
        let mut compute_errs = Vec::new();

        let files = tokens.iter().chunk_by(|token| token.module()).into_iter().filter_map(|(module, bbfiles)| {
            let module = match module {
                Ok(module) => module,
                Err(e) => {
                    compute_errs.push(e);
                    return None;
                }
            };

            Some(RustFile {
                name: module.name.clone(),
                path: module.path(),
                contents: bbfiles.flat_map(|bbfile| {
                    bbfile.children.iter().filter_map(|child| {
                        match child.to_rust() {
                            Ok(rust_file) => Some(rust_file),
                            Err(e) => {
                                compute_errs.push(e);
                                None
                            }
                        }
                    }).collect_vec()
                }).collect(),
            })
        }).collect();

        (files, compute_errs)
    }

    fn validate_results<T>(&self, items: Vec<Result<T>>, action_word: &str, get_name: &impl Fn(&T) -> &str) -> Result<Vec<T>> {
        let mut result_out = String::new();
        let mut last_err = None;
        let mut result = Vec::new();

        result_out += &format!("{} {} files\n", action_word, items.len());

        for item in items {
            match item {
                Ok(item) => {
                    result_out += &format!("{} {}\n", action_word, get_name(&item));
                    result.push(item);
                },
                Err(e) => {
                    result_out += &format!("{}\n", e);
                    last_err = Some(e);
                },
            }
        }
        
        self.logger.log("validate_results", &format!("{result_out}\n"));

        if let Some(last_err) = last_err {
            Err(last_err)
        } else {
            Ok(result)
        }
    }
}

#[derive(Debug)]
pub struct BBFile {
    pub name: String,
    pub path: String,
    pub contents: String,
}

pub struct RustFile {
    pub name: String,
    pub path: String,
    pub contents: TokenStream,
}

#[derive(Debug)]
pub enum ParseError {
    WantWordGotReserved(String),
    WantWordGotEOF(),
    UnexpectedWord { word: String, options: Vec<String> },
    WantReservedGotWord(String),
    WantReservedGotEOF(),
    NotAReserved(String),
    UnexpectedReserved { reserved: String, options: Vec<String> },
    InvalidNumber { word: String, num_type: String, e: Box<dyn std::error::Error> },
    RanOutOfUsedBasics,
    NoLastUsedBasic,
    InvalidCallConv(String),
    ExpectedDelimiter(String),
    EmptyModule,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match self {
            Self::WantWordGotReserved(reserved) => format!("Unexpected reserved: {reserved}"),
            Self::WantWordGotEOF() => format!("Unexpected EOF"),
            Self::UnexpectedWord { word, options } => format!("Unexpected word \"{word}\", expected one of {options:?}"),
            Self::WantReservedGotWord(word) => format!("Unexpected word: {word}"),
            Self::WantReservedGotEOF() => format!("Unexpected EOF"),
            Self::NotAReserved(reserved) => format!("Word '{reserved}' is not reserved"),
            Self::UnexpectedReserved { reserved, options } => format!("Unexpected reserved '{reserved}', expected one of {options:?}"),
            Self::InvalidNumber{ word, num_type, e } => format!("\"{word}\" is not a valid {num_type}: {e}"),
            Self::RanOutOfUsedBasics => format!("Ran out of tracked basic tokens that were used. Tokens must have been ended more times than they were started"),
            Self::NoLastUsedBasic => format!("Tried to access last used basic token when none have been used yet"),
            Self::InvalidCallConv(word) => format!("\"{word}\" is not a valid calling convention"),
            Self::ExpectedDelimiter(delimiter) => format!("Expected delimiter \"{delimiter}\""),
            Self::EmptyModule => format!("Expected module, found nothing"),
        })
    }
}

impl Into<BindingError> for ParseError {
    fn into(self) -> BindingError {
        BindingError::Parse(self)
    }
}

#[derive(Debug)]
pub enum ValidateError {
    MissingModule { file_name: String }
}

impl fmt::Display for ValidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let string = match self {
            ValidateError::MissingModule { file_name } => format!("File {file_name} requires a module (e.g. \"mod path::name\")"),
        };
        write!(f, "{string}")
    }
}

impl Into<BindingError> for ValidateError {
    fn into(self) -> BindingError {
        BindingError::Compute(self)
    }
}

#[derive(Debug)]
pub enum BindingError {
    Parse(ParseError),
    Compute(ValidateError),
}

impl fmt::Display for BindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(p) => write!(f, "{p}"),
            Self::Compute(c) => write!(f, "{c}"),
        }
    }
}

#[derive(Debug)]
pub struct Error {
    pub file_name: String,
    pub char_rect: CharRect,
    pub err: BindingError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to parse {} {}: {}", self.file_name, self.char_rect, self.err)
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        self.file_name == other.file_name && self.char_rect == other.char_rect
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
pub type ParseResult<T> = std::result::Result<T, ParseError>;
pub type ValidateResult<T> = std::result::Result<T, ValidateError>;
