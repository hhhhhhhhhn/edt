#[macro_use] extern crate rocket;
use std::path::{Path, PathBuf};
use rocket::fs::NamedFile;
use rocket::http::Status;
use rocket::serde::Serialize;
use rocket::serde::json::Json;
use std::fs::File;
use std::fs;
use std::io::prelude::*;
use anyhow::Result;
use rfd::FileDialog;

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "value")]
enum FileOrDir {
    File,
    Dir(Vec<(String, FileOrDir)>)
}

fn read_dir(path: PathBuf) -> Result<FileOrDir> {
    let mut children = vec![];
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            children.push((String::from(path.to_string_lossy()), read_dir(path)?))
        } else {
            children.push((String::from(path.to_string_lossy()), FileOrDir::File))
        }
    }
    return Ok(FileOrDir::Dir(children));
}

#[get("/selectdir")]
fn selectdir() -> Option<String> {
    return Some(
        String::from(FileDialog::new().pick_folder()?.to_string_lossy())
    )
}

#[get("/readdir/<dir..>")]
fn readdir(dir: PathBuf) -> Option<Json<FileOrDir>> {
    let read = read_dir(Path::new("/").join(dir));
    return Some(Json(read.ok()?))
}

#[delete("/deletefile/<dir..>")]
fn deletefile(dir: PathBuf) -> Option<()> {
    let path = Path::new("/").join(dir);
    fs::remove_file(path).ok()?;
    return Some(())
}

#[get("/app/<file..>")]
async fn app(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("front/_site").join(file)).await.ok()
}

#[get("/readfile/<file..>")]
async fn readfile(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("/").join(file)).await.ok()
}

#[post("/writefile/<file..>", data="<data>")]
async fn writefile(file: PathBuf, data: String) -> Status {
    let path = Path::new("/").join(file);
    path.parent().and_then(|folder| fs::create_dir_all(folder).ok());

    match File::create(path) {
        Ok(mut file) => {
            match file.write_all(data.as_bytes()) {
                Ok(_) => Status::Created,
                Err(_) => Status::InternalServerError,
            }
        },
        Err(_) => {
            return Status::NotFound;
        }
    }
}

#[launch]
fn rocket() -> _ {
    println!("{:?}", read_dir(PathBuf::from("/home/persona/Notes/Old/2022")).unwrap());
    rocket::build().mount("/", routes![app, readfile, writefile, selectdir, readdir, deletefile])
}
