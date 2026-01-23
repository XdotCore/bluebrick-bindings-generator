use std::{any::type_name, collections::VecDeque, fmt::{self, Debug}, path::Path, str::FromStr};

use itertools::Itertools;
use num_traits::Num;

use crate::generator::{BBFile, Error, ParseError, ParseResult, Result};
use crate::logger::Logger;
use crate::tokens::{FromWord, Token};

pub struct Parser<'a> {
    logger: &'a Logger,
    pub file: BBFile,
    basic_tokens: VecDeque<BasicToken>,
    used_basic_tokens: Vec<Vec<BasicToken>>,
    token_depth: usize,
}

impl<'a> Parser<'a> {
    pub fn new(file: BBFile, logger: &'a Logger) -> Self {
        logger.log(&file.path, &format!("file: {}.bb", file.name));

        let basic_tokens = Self::split_to_basics(&file).into();
        logger.log(&file.path, &format!("\nBasics: {basic_tokens:#?}\n"));
        
        Self {
            logger,
            file,
            basic_tokens,
            used_basic_tokens: Vec::new(),
            token_depth: 0,
        }
    }

    const RESERVED_CHARS: [char; 13] = [ '/', '*', ':', '=', ',', '[', ']', '{', '}', '(', ')', '<', '>' ];
    // must be sorted by length so that longer words are checked first
    const RESERVED_WORDS: [&'static str; 15] = [ "//", "/*", "*/", "::", ":", "=", ",", "[", "]", "{", "}", "(", ")", "<", ">" ];

    // TODO: this deserves a rewrite eventually
    // TODO: add markdown comments for docs generation
    fn split_to_basics(file: &BBFile) -> Vec<BasicToken> {
        let chars = file.contents.chars().collect_vec();
        let mut column = 1;
        let mut line = 1;
        let mut basics = Vec::new();
        
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '\n' => { // newline
                    column = 1;
                    line += 1;
                }
                c if c.is_whitespace() => { // whitespace
                    column += 1;
                }
                c if Self::RESERVED_CHARS.contains(&c) => { // reserved word
                    let begin = (line, column);
                    
                    let mut found_word = None;
                    for word in Self::RESERVED_WORDS.iter().map(|w| w.chars().collect_vec()) {
                        let word_end = i + word.len();
                        if word_end < chars.len() {
                            let check = &chars[i..word_end];
                            if word == check {
                                found_word = Some(word);
                                break;
                            }
                        }
                    }

                    if let Some(word) = found_word {
                        match word.into_iter().collect::<String>().as_str() {
                            "//" => {
                                i += 2;
                                column += 2;
                                let mut comment = "//".to_owned();

                                while i < chars.len() {
                                    match chars[i] {
                                        '\n' => {
                                            i -= 1;
                                            break;
                                        }
                                        '\r' => {
                                            column += 1;
                                        }
                                        c => {
                                            comment.push(c);
                                            column += 1;
                                        }
                                    }
                                    i += 1;
                                }

                                let end = (line, column - 1);

                                basics.push(BasicToken {
                                    variant: BasicTokenVariant::Comment(comment),
                                    char_rect: CharRect::from((begin, end)),
                                });
                            }
                            "/*" => {
                                i += 2;
                                column += 2;
                                let mut comment = "/*".to_owned();

                                while i < chars.len() {
                                    match chars[i] {
                                        '*' if i + 1 < chars.len() && chars[i + 1] == '/' => {
                                            i += 1;
                                            column += 2;
                                            break;
                                        }
                                        '\n' => {
                                            column = 0;
                                            line += 1;
                                            comment.push('\n');
                                        }
                                        c => {
                                            column += 1;
                                            comment.push(c);
                                        }
                                    }
                                    i += 1;
                                }

                                let end = (line, column - 1);

                                basics.push(BasicToken {
                                    variant: BasicTokenVariant::Comment(comment),
                                    char_rect: CharRect::from((begin, end)),
                                });
                            }
                            word => {
                                let word = word.to_owned();
                                column += word.len();
                                i += word.len() - 1;

                                let end = (line, column - 1);

                                basics.push(BasicToken {
                                    variant: BasicTokenVariant::Reserved(word),
                                    char_rect: CharRect::from((begin, end)),
                                });
                            }
                        }
                    } else {
                        let reserved = chars[i].to_string();
                        column += 1;

                        basics.push(BasicToken {
                            variant: BasicTokenVariant::Reserved(reserved),
                            char_rect: CharRect::from((begin, begin)),
                        })
                    }
                }
                c => { // word
                    let begin = (line, column);
                    let mut word = c.to_string();
                    column += 1;
                    i += 1;

                    while i < chars.len() {
                        match chars[i] {
                            c if c.is_whitespace() || Self::RESERVED_CHARS.contains(&c) => {
                                i -= 1; // ignore and end word
                                break;
                            }
                            c => {
                                word.push(c);
                                column += 1;
                            }
                        }
                        i += 1;
                    }

                    let end = (line, column - 1);

                    if word.len() > 0 {
                        basics.push(BasicToken {
                            variant: BasicTokenVariant::Word(word.clone()),
                            char_rect: CharRect::from((begin, end)),
                        });
                    }
                }
            }
            i += 1;
        }

        basics
    }

    fn make_error_impl(&self, char_rect: CharRect, err: ParseError) -> Error {
        Error {
            file_name: self.file.path.clone(),
            char_rect,
            err: err.into(),
        }
    }
    
    fn make_error_with_basic(&self, basic: &BasicToken, err: ParseError) -> Error {
        self.make_error_impl(basic.char_rect, err)
    }

    pub fn make_error(&self, err: ParseError) -> Error {
        if let Some(last) = self.last_used_basic_token() {
            self.make_error_with_basic(last, err)
        } else {
            self.make_error_impl(Default::default(), ParseError::NoLastUsedBasic)
        }
    }

    fn make_eof_error(&self, err: ParseError) -> Error {
        self.make_error_impl(Default::default(), err)
    }

    fn token_tabs_by_depth(&self) -> String {
        let token_depth = self.token_depth;
        let token_depth = token_depth.checked_sub(1).unwrap_or(0);
        let token_depth = "\t".repeat(token_depth);
        token_depth
    }

    fn start_token<T: Token>(&mut self) {
        self.token_depth += 1;
        self.used_basic_tokens.push(Vec::new());
        
        let depth = self.token_tabs_by_depth();
        let type_name = T::token_type();
        self.logger.log(&self.file.path, &format!("{depth}Starting {type_name} token"));
    }

    fn end_token<T: Token>(&mut self) {
        let depth = self.token_tabs_by_depth();
        let type_name = type_name::<T>();
        self.logger.log(&self.file.path, &format!("{depth}Ending {type_name} token"));

        if let Some(mut basics_used) = self.used_basic_tokens.pop() &&
           let Some(parent_basics_used) = self.used_basic_tokens.last_mut() {
            parent_basics_used.append(&mut basics_used);
        }
        self.token_depth -= 1;
    }

    fn log_token<T: Token>(&self, token: &T) {
        let depth = self.token_tabs_by_depth();
        let char_rect = token.char_rect();
        self.logger.log(&self.file.path, &format!("{depth}{token:?}, {char_rect}"));
    }

    pub fn token<T: Token>(&mut self) -> Result<T> {
        self.token_from_ctor(&T::parse)
    }

    pub fn token_from_ctor<T: Token>(&mut self, ctor: &impl Fn(&mut Self) -> Result<T>) -> Result<T> {
        self.start_token::<T>();
        let token = ctor(self)?;
        self.log_token(&token);
        self.end_token::<T>();
        Ok(token)
    }

    pub fn token_char_rect(&self) -> Result<CharRect> {
        if let Some(basics_used) = self.used_basic_tokens.last() {
            let start = basics_used.iter().map(|b| b.char_rect.start()).min().unwrap_or_default();
            let end = basics_used.iter().map(|b| b.char_rect.end()).max().unwrap_or_default();
            Ok(CharRect::from((start, end)))
        } else {
            Err(self.make_error(ParseError::RanOutOfUsedBasics))
        }
    }

    pub fn all_tokens<T: Token>(&mut self) -> Result<Vec<T>> {
        let mut tokens = Vec::new();

        while self.basic_tokens.iter().filter(|b| !matches!(b.variant, BasicTokenVariant::Comment(_))).count() > 0 {
            tokens.push(T::parse(self)?);
        }

        Ok(tokens)
    }

    fn use_basic(&mut self, basic: BasicToken) -> Result<()> {
        if let Some(basics_used) = self.used_basic_tokens.last_mut() {
            basics_used.push(basic);
            Ok(())
        } else {
            Err(self.make_error(ParseError::RanOutOfUsedBasics))
        }
    }

    fn last_used_basic_token(&self) -> Option<&BasicToken> {
        self.used_basic_tokens.iter().flatten().rev().find(|basic| {
            match basic.variant {
                BasicTokenVariant::Comment(_) => {
                    false // ignore
                }
                _ => true
            }
        })
    }

    pub fn take_word(&mut self) -> Result<String> {
        while let Some(basic) = self.basic_tokens.pop_front() {
            match basic.variant.clone() {
                BasicTokenVariant::Comment(_) => {
                    self.use_basic(basic);
                }
                BasicTokenVariant::Reserved(reserved) => {
                    return Err(self.make_error_with_basic(&basic, ParseError::WantWordGotReserved(reserved)));
                }
                BasicTokenVariant::Word(word) => {
                    self.use_basic(basic);
                    return Ok(word);
                }
            }
        }

        Err(self.make_eof_error(ParseError::WantWordGotEOF()))
    }

    pub fn peek_word(&self) -> Option<String> {
        match self.basic_tokens.front() {
            Some(front) => {
                match front.variant.clone() {
                    BasicTokenVariant::Word(word) => Some(word),
                    _ => None,
                }
            }
            None => None
        }
    }

    pub fn take_reserved(&mut self) -> Result<String> {
        while let Some(basic) = self.basic_tokens.pop_front() {
            match basic.variant.clone() {
                BasicTokenVariant::Comment(_) => {
                    self.use_basic(basic);
                }
                BasicTokenVariant::Reserved(reserved) => {
                    self.use_basic(basic);
                    return Ok(reserved);
                }
                BasicTokenVariant::Word(word) => {
                    return Err(self.make_error_with_basic(&basic, ParseError::WantReservedGotWord(word)));
                }
            }
        }

        Err(self.make_eof_error(ParseError::WantReservedGotEOF()))
    }

    pub fn peek_reserved(&self) -> Option<String> {
        match self.basic_tokens.front() {
            Some(front) => {
                match front.variant.clone() {
                    BasicTokenVariant::Reserved(reserved) => Some(reserved),
                    _ => None,
                }
            }
            None => None
        }
    }

    pub fn drop_word(&mut self) -> Result<()> {
        self.take_word().map(|_| ())
    }

    pub fn if_word<T>(&mut self, word: &str, action: &impl Fn(&mut Self) -> Result<T>) -> Result<Option<T>> {
        if Some(word) == self.peek_word().as_ref().map(|s| s.as_str()) {
            self.drop_word()?;
            action(self).map(|r| Some(r))
        } else {
            Ok(None)
        }
    }

    pub fn drop_reserved(&mut self) -> Result<()> {
        self.take_reserved().map(|_| ())
    }

    pub fn if_reserved<T>(&mut self, reserved: &str, action: &impl Fn(&mut Self) -> Result<T>) -> Result<Option<T>> {
        if !Self::is_reserved(reserved) {
            Err(self.make_error(ParseError::NotAReserved(reserved.to_owned())))?;
        }

        if Some(reserved) == self.peek_reserved().as_deref() {
            action(self).map(|r| Some(r))
        } else {
            Ok(None)
        }
    }

    pub fn from_word<T: FromWord>(&mut self) -> Result<T> {
        T::from_word(self.take_word()?.as_str()).map_err(|e| self.make_error(e))
    }

    pub fn expect_word(&mut self, options: &[&str]) -> Result<String> {
        let options = options.iter().map(|o| (*o).to_owned()).collect_vec();
        let word = self.take_word()?;

        if options.contains(&word) {
            Ok(word)
        } else {
            Err(self.make_error(ParseError::UnexpectedWord { word, options }))
        }
    }

    fn is_reserved(reserved: &str) -> bool {
        Self::RESERVED_WORDS.contains(&reserved)
    }

    pub fn expect_reserved<'i, I>(&mut self, options: I) -> Result<String> where I: IntoIterator<Item = &'i str> {
        let options = options.into_iter().collect_vec();

        if let Some(not_reserved) = options.iter().find(|c| !Self::is_reserved(c)).copied() {
            return Err(self.make_error(ParseError::NotAReserved(not_reserved.to_owned())));
        }

        let reserved = self.take_reserved()?;

        if options.contains(&reserved.as_str()) {
            Ok(reserved)
        } else {
            Err(self.make_error(ParseError::UnexpectedReserved {
                reserved,
                options: options.iter().map(|o| (*o).to_owned()).collect()
            }))
        }
    }

    pub fn expect_num<T: Num + FromStr>(&mut self) -> Result<T>
    where
        T::FromStrRadixErr: std::error::Error + 'static,
        T::Err: std::error::Error + 'static,
    {
        let num = self.take_word()?;
        let (radix, num) = match num.split_at_checked(2) {
            Some((prefix, value)) => {
                match prefix {
                    "0b" => (2, value),
                    "0o" => (8, value),
                    "0x" => (16, value),
                    _ => (10, num.as_str()),
                }
            }
            None => (10, num.as_str())
        };
        T::from_str_radix(num, radix).map_err(|e| self.make_error(ParseError::InvalidNumber {
            word: num.to_owned(),
            num_type: type_name::<T>().to_owned(),
            e: Box::new(e),
        }))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CharRect {
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
}

impl CharRect {
    fn start(&self) -> (usize, usize) {
        (self.start_line, self.start_column)
    }

    fn end(&self) -> (usize, usize) {
        (self.end_line, self.end_column)
    }
}

impl fmt::Display for CharRect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "from {}, {}, to {}, {}", self.start_line, self.start_column, self.end_line, self.end_column)
    }
}

impl From<((usize, usize), (usize, usize))> for CharRect {
    fn from(value: ((usize, usize), (usize, usize))) -> Self {
        let ((start_line, start_column), (end_line, end_column)) = value;
        CharRect {
            start_line,
            start_column,
            end_line,
            end_column,
        }
    }
}

impl From<(CharRect, CharRect)> for CharRect {
    fn from(value: (CharRect, CharRect)) -> Self {
        let (a, b) = value;
        let start = a.start().min(b.start());
        let end = a.end().max(b.end());
        
        Self::from((start, end))
    }
}

#[derive(Debug, Clone)]
struct BasicToken {
    variant: BasicTokenVariant,
    char_rect: CharRect,
}

#[derive(Debug, Clone)]
enum BasicTokenVariant {
    Word(String),
    Reserved(String),
    Comment(String),
}
