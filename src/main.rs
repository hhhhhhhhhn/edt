#[macro_use] extern crate rocket;
#[macro_use] extern crate lazy_static;
use std::path::{Path, PathBuf};
use rocket::fs::NamedFile;
use rocket::http::{Status, ContentType};
use rocket::serde::Serialize;
use rocket::serde::json::Json;
use include_dir::{include_dir, Dir};
use std::borrow::Cow;
use std::fs::File;
use std::fs;
use rocket::tokio;
use std::process::Command;
use std::io::prelude::*;
use std::time;
use anyhow::Result;
use rfd::FileDialog;
use std::sync::{Arc, Mutex};

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

static FRONT_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/front/_site");
#[get("/app/<file..>")]
async fn app(file: PathBuf) -> Option<(ContentType, Cow<'static, [u8]>)> {
    let extension = file.extension()?;
    let contents = FRONT_DIR.get_file(file.clone())?;
    let content_type = ContentType::from_extension(extension.to_str()?).unwrap_or(ContentType::Bytes);

    return Some((content_type, Cow::Borrowed(contents.contents())))
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

#[get("/keepalive")]
async fn keepalive() {
    *Arc::clone(&LAST_PING).lock().unwrap() =
        time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_secs();
}

lazy_static! {
    static ref LAST_PING: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
}

#[rocket::main]
async fn main() -> Result<()> {
    Command::new("sh")
        .arg("-c")
        .arg("$BROWSER --app='http://localhost:8000/app/index.html'")
        .spawn()
        .expect("Failed to spawn browser");

    let rocket = rocket::build()
        .mount("/", routes![app, readfile, writefile, selectdir, readdir, deletefile, keepalive])
        .ignite()
        .await?;
    let shutdown = rocket.shutdown();

    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            let time = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_secs();
            let since_last = time - *Arc::clone(&LAST_PING).lock().unwrap();
            if since_last > 30 {
                break
            }
        }
        shutdown.notify();
    });
    rocket.launch().await?;

    return Ok(())
}
