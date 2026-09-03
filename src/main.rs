mod parsers;

use std::path::Path;

use crate::parsers::parse_project;

fn main() {
    let project_folder = std::env::args().nth(1).unwrap();
    let project_folder = Path::new(&project_folder);

    let (project, errors) = parse_project(project_folder);

    for error in errors {
        println!("error: {error}");
    }

    if let Some(project) = project {
        for file in project.source {
            println!("Found {}", file.relative.display());
        }
    }
}
