use std::{error::Error, fs, path::{Path, PathBuf}};

use pathdiff::diff_paths;
use walkdir::WalkDir;

pub struct Project {
    pub source: Vec<File>,
}

pub struct File {
    pub path: PathBuf,
    pub relative: PathBuf,
    pub contents: String,
}

pub fn parse_project(folder: &Path) -> (Option<Project>, Vec<Box<dyn Error>>) {
    let mut source = Vec::new();
    let mut errors = Vec::<Box<dyn Error>>::new();

    if folder.is_dir() {
        let folder = match folder.canonicalize() {
            Ok(folder) => folder,
            Err(err) => {
                errors.push(Box::new(err));
                return (None, errors);
            }
        };

        for entry in WalkDir::new(&folder) {
            macro_rules! try_or_continue {
                ($expr:expr) => {
                    match $expr {
                        Ok(value) => value,
                        Err(err) => {
                            errors.push(Box::new(err));
                            continue;
                        }
                    }
                };
            }

            let entry = try_or_continue!(entry);

            let path = try_or_continue!(entry.path().canonicalize());
            let extension = path.extension()
                .map(|ext| ext.to_str())
                .flatten();

            if path.is_file() && extension == Some("bb") {
                let contents = try_or_continue!(fs::read(&path));
                let contents = try_or_continue!(String::from_utf8(contents));

                source.push(File {
                    path: path.to_path_buf(),
                    relative: diff_paths(&path, &folder).unwrap(), // todo: unwrap
                    contents,
                });
            }
        }
    }

    (
        Some(Project {
            source,
        }),
        errors,
    )
}
