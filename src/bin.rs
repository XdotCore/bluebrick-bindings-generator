use bluebrick_bindings_generator::generate_bluebrick_bindings;
use cargo_metadata::MetadataCommand;
use serde::Deserialize;
use structopt::StructOpt;
use std::{error::Error, fmt::Display, path::PathBuf};

#[derive(Debug)]
enum BinError {
    Project(String),
}

impl Display for BinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::Project(project) => format!("Could not find project \"{project}\" in workspace"),
        })
    }
}

impl Error for BinError {}

#[derive(Debug, Deserialize)]
struct BBBGenStruct {
    bb: PathBuf,
    rust: PathBuf,
}

#[derive(Debug, Deserialize)]
struct MetadataStruct {
    #[serde(rename = "bbb-gen")]
    bbb_gen: BBBGenStruct,
}

fn specified_project_metadata(project: String) -> Result<BBBGenStruct, Box<dyn Error>> {
    let metadata = MetadataCommand::new().no_deps().exec()?;
    let packages = metadata.workspace_packages();
    let project = packages.iter()
        .find(|p| p.name == project)
        .ok_or(BinError::Project(project))?;

    let path = project.manifest_path.parent().unwrap();

    let metadata = project.metadata.clone();
    let metadata = serde_json::from_value::<MetadataStruct>(metadata)?;
    let mut bbb_gen = metadata.bbb_gen;

    bbb_gen.bb = path.join_os(bbb_gen.bb).canonicalize()?;
    bbb_gen.rust = path.join_os(bbb_gen.rust).canonicalize()?;

    Ok(bbb_gen)
}

#[derive(Debug, StructOpt)]
#[structopt(name = "BlueBrick Bindings Generator", about = "Converts a BlueBrick bindings project to other languages.")]
struct Opt {
    #[structopt(short, long)]
    project: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_args();
    
    let metadata = specified_project_metadata(opt.project)?;

    let bb = metadata.bb;
    let rust = metadata.rust;

    generate_bluebrick_bindings(&bb, &rust)?;

    Ok(())
}