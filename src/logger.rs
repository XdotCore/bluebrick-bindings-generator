use std::{error::Error, fmt::Display, fs::{self, OpenOptions}, io::Write, path::{Path, PathBuf}};

#[derive(Debug)]
pub enum LoggerError {
    CheckRootExist(PathBuf, Box<dyn Error>),
    ResetRoot(PathBuf, Box<dyn Error>),
    CreateRoot(PathBuf, Box<dyn Error>),
    GetLogFileParent(PathBuf),
    CreateLogFolder(PathBuf, Box<dyn Error>),
    OpenLogFile(PathBuf, Box<dyn Error>),
    WriteLogFile(PathBuf, Box<dyn Error>),
}

impl Display for LoggerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::CheckRootExist(folder, e) => format!("Could not check if log folder exists: {}\n{e}", folder.display()),
            Self::ResetRoot(folder, e) => format!("Could not reset log foler: {}\n{e}", folder.display()),
            Self::CreateRoot(folder, e) => format!("Could not create log folder: {}\n{e}", folder.display()),
            Self::GetLogFileParent(file) => format!("Unexpected! Log file path has no parent: {}", file.display()),
            Self::CreateLogFolder(file, e) => format!("Could not create folder for log file: {}\n{e}", file.display()),
            Self::OpenLogFile(file, e) => format!("Could not create or open log file in append mode: {}\n{e}", file.display()),
            Self::WriteLogFile(file, e) => format!("Could not write to log file: {}\n{e}", file.display()),
        })
    }
}

impl Error for LoggerError {}

pub struct Logger {
    folder: PathBuf,
    e: Option<LoggerError>,
}

impl Logger {
    fn init(folder: &Path) -> Result<(), LoggerError> {
        if folder.try_exists().map_err(|e| LoggerError::CheckRootExist(folder.to_owned(), Box::new(e)))? {
            fs::remove_dir_all(folder).map_err(|e| LoggerError::ResetRoot(folder.to_owned(), Box::new(e)))?;
        }
        fs::create_dir_all(folder).map_err(|e| LoggerError::CreateRoot(folder.to_owned(), Box::new(e)))?;
        Ok(())
    }

    pub fn new(output_dir: &Path) -> Self {
        let folder = output_dir.join("logs");
        let e = Self::init(&folder).err();

        if let Some(e) = &e {
            eprintln!("{e}");
        }

        Self { folder, e, }
    }

    fn log_to_file(&self, file: &str, msg: &str) -> Result<(), LoggerError> {
        let file_path = self.folder.join(file).with_extension("log");

        let parent = file_path.parent().ok_or_else(|| LoggerError::GetLogFileParent(file_path.clone()))?;
        fs::create_dir_all(parent).map_err(|e| LoggerError::CreateLogFolder(file_path.clone(), Box::new(e)))?;

        let mut file = OpenOptions::new().create(true).append(true)
            .open(&file_path).map_err(|e| LoggerError::OpenLogFile(file_path.clone(), Box::new(e)))?;
        file.write(msg.as_bytes()).map_err(|e| LoggerError::WriteLogFile(file_path.clone(), Box::new(e)))?;

        Ok(())
    }

    fn log_impl(&self, file: &str, msg: &str, print: &impl Fn (&str)) {
        // out to stdout
        print(msg);

        // don't try to write to file if setup failed
        if self.e.is_some() {
            return;
        }

        // try to write to log file
        let msg = format!("{}\n", msg);
        if let Err(e) = self.log_to_file(file, &msg) {
            eprintln!("{e}");
        }
    }

    pub fn log(&self, file: &str, msg: &str) {
        self.log_impl(file, msg, &|msg | println!("{msg}"))
    }

    pub fn elog(&self, file: &str, msg: &str) {
        self.log_impl(file, msg, &|msg | eprintln!("{msg}"))
    }
}
