use std::{
    fmt,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

const PDF_READER: &str = "zathura";
const EDITOR: &str = "nvim";
const TYPST_HEADER: &str = r#"#import "template/lib.typ": *

#show: project.with(
    title: [],
)

#bibliography("bibliography.yml")
"#;

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

enum LoCommand {
    Open {
        lesson: Lesson,
    },
    Create {
        lesson: Lesson,
    },
    Rename {
        old_lesson: Lesson,
        new_lesson: Lesson,
    },
    Remove {
        lesson: Lesson,
    },
    Fuzz,
}

#[derive(Debug)]
enum ParsingError {
    WrongArgNumber { expected: u32, got: u32 },
    UnknownCommand(String),
}

impl std::error::Error for ParsingError {}

impl fmt::Display for ParsingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParsingError::WrongArgNumber { expected, got } => {
                write!(
                    f,
                    "Wrong number of arguments for the command. Expected {expected}, got {got}"
                )
            }
            ParsingError::UnknownCommand(command) => {
                write!(f, "Unknown command : {command}")
            }
        }
    }
}

fn parse_args() -> Result<LoCommand, ParsingError> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() == 1 {
        return Ok(LoCommand::Fuzz);
    }

    match &args[1][..] {
        "rename" | "rn" | "move" | "mv" => {
            if args.len() != 4 {
                return Err(ParsingError::WrongArgNumber {
                    expected: 2,
                    got: args.len() as u32 - 2,
                });
            }

            Ok(LoCommand::Rename {
                old_lesson: Lesson::new(args[2].clone()),
                new_lesson: Lesson::new(args[3].clone()),
            })
        }
        "create" | "new" | "n" => {
            if args.len() != 3 {
                return Err(ParsingError::WrongArgNumber {
                    expected: 1,
                    got: args.len() as u32 - 2,
                });
            }

            Ok(LoCommand::Create {
                lesson: Lesson::new(args[2].clone()),
            })
        }
        "remove" | "rm" | "delete" | "d" => {
            if args.len() != 3 {
                return Err(ParsingError::WrongArgNumber {
                    expected: 1,
                    got: args.len() as u32 - 2,
                });
            }

            Ok(LoCommand::Remove {
                lesson: Lesson::new(args[2].clone()),
            })
        }
        "open" | "o" => {
            if args.len() != 3 {
                return Err(ParsingError::WrongArgNumber {
                    expected: 1,
                    got: args.len() as u32 - 2,
                });
            }

            Ok(LoCommand::Open {
                lesson: Lesson::new(args[2].clone()),
            })
        }
        _ => {
            if args.len() != 2 {
                return Err(ParsingError::WrongArgNumber {
                    expected: 1,
                    got: args.len() as u32 - 1,
                });
            }

            if args[1].len() <= 3 || &args[1][args[1].len() - 4..] != ".typ" {
                return Err(ParsingError::UnknownCommand(args[1].clone()));
            }

            Ok(LoCommand::Open {
                lesson: Lesson::new(args[1].clone()),
            })
        }
    }
}

fn rename_lesson(old_lesson: Lesson, new_lesson: Lesson) -> Result<(), std::io::Error> {
    if new_lesson.typst_path().exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("File {} already exists", new_lesson.typst_path().display()),
        ));
    }

    // if this fails midway through, how do we handle it ?
    fs::rename(old_lesson.typst_path(), new_lesson.typst_path())?;
    fs::rename(old_lesson.pdf_path(), new_lesson.pdf_path())?;

    Ok(())
}

fn remove_lesson(lesson: Lesson) -> Result<(), std::io::Error> {
    fs::remove_file(lesson.typst_path())?;
    fs::remove_file(lesson.pdf_path())?;

    Ok(())
}

fn create_lesson(lesson: Lesson) -> Result<(), std::io::Error> {
    if lesson.typst_path().exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "Lesson file already exists: {}",
                lesson.typst_path().display()
            ),
        ));
    }

    lesson.create()?;
    open_lesson(lesson)
}

fn open_lesson(lesson: Lesson) -> Result<(), std::io::Error> {
    if !lesson.typst_path().exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Missing file: {}", lesson.typst_path().display()),
        ));
    }

    // we recompile just in case
    Command::new("typst")
        .arg("compile")
        .arg(lesson.typst_path())
        .arg(lesson.pdf_path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

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
    let command = parse_args()?;

    match command {
        LoCommand::Fuzz => match fuzzy_find_file() {
            None => (),
            Some(filename) => {
                open_lesson(Lesson::new(filename))?;
                println!("");
            }
        },
        LoCommand::Open { lesson } => open_lesson(lesson)?,
        LoCommand::Rename {
            old_lesson,
            new_lesson,
        } => rename_lesson(old_lesson, new_lesson)?,
        LoCommand::Remove { lesson } => remove_lesson(lesson)?,
        LoCommand::Create { lesson } => create_lesson(lesson)?,
    }
    Ok(())
}
