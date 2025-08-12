use std::{fs::{self, File}, io::Write, path::PathBuf, process::{Command, Stdio}};

use clap::Parser;

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
    filename: String,
}

fn get_new_file_contents() -> String {
    TYPST_HEADER.into()
}

fn main() {
    let organizer = Organizer::parse();

    let typst_file_path = PathBuf::from(organizer.filename);

    let output_folder = {
        let mut output_folder = typst_file_path.parent().unwrap().to_owned();
        output_folder.push("output");
        output_folder
    };

    let pdf_file_path = {
        let mut pdf_file_path = output_folder.clone();
        let mut pdf_file_name = typst_file_path.file_stem().unwrap().to_string_lossy().to_string();
        pdf_file_name.push_str(".pdf");
        pdf_file_path.push(pdf_file_name);
        pdf_file_path
    };

    if !typst_file_path.exists() {
        let mut f = File::create(&typst_file_path).unwrap();
        f.write_all(get_new_file_contents().as_bytes()).unwrap();
    }

    if !pdf_file_path.exists() {
        if !output_folder.exists() {
            fs::create_dir(output_folder).unwrap();
        }
        Command::new("typst")
            .arg("compile")
            .arg(&typst_file_path)
            .arg(&pdf_file_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("Failed to compile typst file");
    }

    let mut pdf_reader = Command::new(PDF_READER)
        .arg(&pdf_file_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn pdf reader");

    Command::new(EDITOR)
        .arg(&typst_file_path)
        .status()
        .expect("Failed to spawn editor");

    pdf_reader.kill().unwrap();
    pdf_reader.wait().unwrap();
}
