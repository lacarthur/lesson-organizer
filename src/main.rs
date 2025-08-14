use std::{
    fmt,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

use clap::{Parser, Subcommand};

const PDF_READER: &str = "zathura";
const EDITOR: &str = "nvim";
const TYPST_HEADER: &str = r#"#import "template/lib.typ": *

#show: project.with(
    title: [],
)

#bibliography("bibliography.yml")
"#;

#[derive(Parser)]
#[command(version, about, long_about=None)]
struct Organizer {
    #[command(subcommand)]
    command: Option<LoCommand>,
}

struct Lesson {
    parent_folder: PathBuf,
    filestem: String,
}

impl Lesson {
    /// Parse a typst filename, and returns a lesson that we can query for the pdf filename
    fn new(typst_filename: String) -> Self {
        let path = PathBuf::from(typst_filename);
        let parent_folder = path.parent().unwrap().to_owned();
        let filestem = path.file_stem().unwrap().to_string_lossy();

        Lesson {
            parent_folder,
            filestem: filestem.into(),
        }
    }

    fn output_folder(&self) -> PathBuf {
        self.parent_folder.join("output")
    }

    fn pdf_path(&self) -> PathBuf {
        self.output_folder().join(format!("{}.pdf", &self.filestem))
    }

    fn typst_path(&self) -> PathBuf {
        self.parent_folder.join(format!("{}.typ", &self.filestem))
    }

    fn create(&self) -> Result<(), std::io::Error> {
        let mut file = File::create(self.typst_path()).unwrap();
        file.write_all(get_new_file_contents().as_bytes()).unwrap();

        if !self.output_folder().exists() {
            fs::create_dir(self.output_folder()).unwrap();
        }
        Command::new("typst")
            .arg("compile")
            .arg(self.typst_path())
            .arg(self.pdf_path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        Ok(())
    }
}

#[derive(Subcommand)]
enum LoCommand {
    Open {
        filename: String,
    },
    Rename {
        old_filename: String,
        new_filename: String,
    },
    Remove {
        filename: String,
    },
}

#[derive(Debug)]
enum RenameError {
    FileAlreadyExists(PathBuf),
    MissingFile(PathBuf),
    Other(std::io::Error),
}

impl fmt::Display for RenameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenameError::FileAlreadyExists(path) => {
                write!(f, "The file {} already exists", path.display())
            }
            RenameError::MissingFile(path) => {
                write!(f, "The file {} does not exist", path.display())
            }
            RenameError::Other(error) => write!(f, "IO Error when renaming file: {error}"),
        }
    }
}

impl std::error::Error for RenameError {}

fn rename_lesson(old_filename: String, new_filename: String) -> Result<(), RenameError> {
    let old_lesson = Lesson::new(old_filename);
    let new_lesson = Lesson::new(new_filename);

    if !old_lesson.typst_path().exists() {
        return Err(RenameError::MissingFile(old_lesson.typst_path()));
    }

    if new_lesson.typst_path().exists() {
        return Err(RenameError::FileAlreadyExists(new_lesson.typst_path()));
    }

    // if this fails midway through, how do we handle it ?
    fs::rename(old_lesson.typst_path(), new_lesson.typst_path()).map_err(RenameError::Other)?;
    fs::rename(old_lesson.pdf_path(), new_lesson.pdf_path()).map_err(RenameError::Other)?;

    Ok(())
}

#[derive(Debug)]
enum DeleteError {
    MissingFile(PathBuf),
    Other(std::io::Error),
}

impl fmt::Display for DeleteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeleteError::MissingFile(path) => {
                write!(f, "The file {} does not exist", path.display())
            }
            DeleteError::Other(error) => write!(f, "IO Error when deleting file : {error}"),
        }
    }
}

impl std::error::Error for DeleteError {}

fn remove_lesson(filename: String) -> Result<(), DeleteError> {
    let lesson = Lesson::new(filename);
    if !lesson.typst_path().exists() {
        return Err(DeleteError::MissingFile(lesson.typst_path()));
    }

    fs::remove_file(lesson.typst_path()).map_err(DeleteError::Other)?;
    fs::remove_file(lesson.pdf_path()).map_err(DeleteError::Other)?;

    Ok(())
}

fn open_lesson(filename: String) -> Result<(), std::io::Error> {
    let lesson = Lesson::new(filename);
    if !lesson.typst_path().exists() {
        lesson.create()?;
    }

    let mut pdf_reader = Command::new(PDF_READER)
        .arg(lesson.pdf_path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn pdf reader");

    Command::new(EDITOR)
        .arg(lesson.typst_path())
        .status()
        .expect("Failed to spawn editor");

    pdf_reader.kill().unwrap();
    pdf_reader.wait().unwrap();

    Ok(())
}

fn get_new_file_contents() -> String {
    TYPST_HEADER.into()
}

fn fuzzy_find_file() -> Option<String> {
    let ls = Command::new("ls")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run ls");

    let ls_out = ls.stdout.expect("Failed to get ls stdout");

    let grep = Command::new("grep")
        .arg(".typ")
        .stdin(Stdio::from(ls_out))
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run grep");

    let grep_out = grep.stdout.expect("Failed to get grep stdout");

    let fzf = Command::new("fzf")
        .stdin(Stdio::from(grep_out))
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run fzf");

    let output = fzf.wait_with_output().unwrap();
    if output.status.success() {
        Some(str::from_utf8(&output.stdout).unwrap().to_owned())
    } else {
        None
    }
}

fn main() -> anyhow::Result<()> {
    let organizer = Organizer::parse();

    match organizer.command {
        None => match fuzzy_find_file() {
            None => (),
            Some(filename) => open_lesson(filename)?,
        },
        Some(LoCommand::Open { filename }) => open_lesson(filename)?,
        Some(LoCommand::Rename {
            old_filename,
            new_filename,
        }) => rename_lesson(old_filename, new_filename)?,
        Some(LoCommand::Remove { filename }) => remove_lesson(filename)?,
    }
    Ok(())
}
