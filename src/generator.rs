use itertools::Itertools;
use proc_macro2::TokenStream;
use std::{collections::HashMap, ffi::OsStr, fmt::{self, Debug}, fs::{self}, iter, path::{Path, PathBuf}};

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
        let tokens = self.validate_parse(tokens)?;

        let rust_files = self.compute_rust(tokens);
        let rust_files = self.validate_compute(rust_files)?;
        self.write_files(&rust_files);

        Ok(())
    }

    fn parse_bb_bindings(&self) -> Vec<Result<FileToken>> {
        self.walk_all_bb_files(&self.bb_dir.clone(), &|file| {
            let mut parser = Parser::new(file, &self.logger);
            parser.token()
        })
    }

    fn walk_all_bb_files(&self, dir: &Path, action: &impl Fn(File) -> Result<FileToken>) -> Vec<Result<FileToken>> {
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
                file_tokens.push(action(File { name, path, contents }));
            }
        }
        file_tokens
    }

    fn validate_parse(&self, files: Vec<Result<FileToken>>) -> Result<Vec<FileToken>> {
        let mut result_out = String::new();
        let mut last_err = None;
        let mut result = Vec::new();

        result_out += &format!("Parsed {} files\n", files.len());

        for file in files {
            match file {
                Ok(file) => {
                    result_out += &format!("Parsed {}\n", file.name);
                    result.push(file);
                },
                Err(e) => {
                    result_out += &format!("{}\n", e);
                    last_err = Some(e);
                },
            }
        }
        
        self.logger.log("validate_parse", &format!("{result_out}\n"));

        if let Some(last_err) = last_err {
            Err(last_err)
        } else {
            Ok(result)
        }
    }

    fn compute_rust(&self, tokens: Vec<FileToken>) -> VecResult<File> {
        let mut compute_errs = Vec::new();

        let mut modules = HashMap::new();
        for mut token in tokens {
            match token.take_module() {
                Ok(module) => modules.entry(module).or_insert(Vec::new()).push(token),
                Err(e) => compute_errs.push(e),
            }
        }

        let files = modules.iter().filter_map(|(module, bbfiles)| {
            let contents = bbfiles.into_iter().filter_map(|bbfile| match bbfile.compute_rust() {
                Ok(rust) => Some(rust),
                Err(mut es) => {
                    compute_errs.append(&mut es);
                    None
                }
            }).flatten().collect::<TokenStream>().to_string();

            Some(File {
                name: module.name.clone(),
                path: module.path(),
                contents,
            })
        }).collect_vec();

        if compute_errs.len() > 0 {
            Err(compute_errs)
        } else {
            Ok(files)
        }
    }

    fn validate_compute(&self, results: VecResult<File>) -> Result<Vec<File>> {
        match results {
            Ok(results) => {
                let mut result_out = format!("Computed {} files\n", results.len());

                result_out += &results.iter().map(|file| &file.name).join("\n");

                self.logger.log("validate_compute", &result_out);

                Ok(results)
            }
            Err(mut errors) => {
                let mut result_out = "Got the following errors when computing rust bindings:\n".to_owned();

                result_out += &errors.iter().join("\n");

                self.logger.elog("validate_compute", &result_out);
                
                // TODO: get rid of this unwrap
                Err(errors.pop().unwrap())
            }
        }
    }

    fn write_files(&self, files: &Vec<File>) {
        for file in files {
            file.write(&self.rust_dir, &self.logger);
        }
    }
}

pub struct File {
    pub name: String,
    pub path: String,
    pub contents: String,
}

impl File {
    fn write(&self, root: &Path, logger: &Logger) {
        let path = root.join(&self.path).join(&self.name).with_added_extension("rs");
        if let Err(e) = fs::write(&path, &self.contents) {
            logger.elog("write_file", &format!("Failed to write output to {}: {}", path.display(), e));
        }
    }
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
pub enum ComputeError {
    MissingModule { file_name: String }
}

impl fmt::Display for ComputeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let string = match self {
            Self::MissingModule { file_name } => format!("File {file_name} requires a module (e.g. \"mod path::name\") at the top"),
        };
        write!(f, "{string}")
    }
}

impl Into<BindingError> for ComputeError {
    fn into(self) -> BindingError {
        BindingError::Compute(self)
    }
}

#[derive(Debug)]
pub enum BindingError {
    Parse(ParseError),
    Compute(ComputeError),
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
        write!(f, "Failed in {} at {}: {}", self.file_name, self.char_rect, self.err)
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        self.file_name == other.file_name && self.char_rect == other.char_rect
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
pub type VecResult<T> = std::result::Result<Vec<T>, Vec<Error>>;
pub type ParseResult<T> = std::result::Result<T, ParseError>;
pub type ComputeResult<T> = std::result::Result<T, ComputeError>;
