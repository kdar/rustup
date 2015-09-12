extern crate hyper;
extern crate env_logger;
extern crate toml;
extern crate time;
extern crate rustbar;
extern crate rustc_serialize;

use std::io;
use std::fs::File;
use std::path::Path;
use std::io::{Read, Write};
use std::io::Result as IOResult;
use rustbar::rustbars::ProgressBar;
use std::process::Command;

use hyper::Client;
use hyper::header;

use rustc_serialize::Encodable;

const URL: &'static str = "https://static.rust-lang.org/dist/rust-nightly-x86_64-pc-windows-gnu.msi";

#[derive(RustcEncodable, RustcDecodable, Debug)]
struct DB {
  datemodified: String,
}

fn read_file(path: &Path) -> IOResult<String> {
  let mut f = try!(File::open(path));
  let mut buf = String::new();
  try!(f.read_to_string(&mut buf));
  Ok(buf)
}

pub fn copy_with_progress<R: io::Read, W: io::Write>(reader: &mut R, writer: &mut W, size: u64) -> io::Result<u64> {
  let mut pbar = rustbar::rustbars::PercentageProgressBar::new();

  let mut buf = [0; 8096];
  let mut written = 0;
  loop {
    let len = match reader.read(&mut buf) {
      Ok(0) => return Ok(written),
      Ok(len) => len,
      Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
      Err(e) => return Err(e),
    };

    try!(writer.write_all(&buf[..len]));
    written += len as u64;

    pbar.set_msg("Downloading...");
    pbar.set_value((written as f64/size as f64 * 100.0) as u8);
    pbar.render().unwrap();
  }
}

fn main() {
  env_logger::init().unwrap();

  let tomlstr = match read_file(&Path::new("db.toml")) {
    Err(_) => "".to_owned(),
    Ok(data) => data,
  };

  let mut db = match toml::decode_str::<DB>(&tomlstr) {
     Some(x)  => x,
     None => DB{datemodified: "".to_owned()},
  };

  let client = Client::new();
  let res = client.head(URL).send().unwrap();

  let header::ContentLength(contentlength) = *res.headers.get::<header::ContentLength>().unwrap();
  let datemodified = match res.headers.get() {
    Some(&header::LastModified(header::HttpDate(ref tm))) => {
      format!("{}", tm.rfc3339())
    },
    None => "".to_owned(),
  };

  if datemodified > db.datemodified {
    println!("Update required.");
    {
      let mut f = File::create("update.msi").unwrap();
      let mut res = client.get(URL).send().unwrap();
      copy_with_progress(&mut res, &mut f, contentlength).unwrap();
    }

    println!("\nInstalling...");
    Command::new("msiexec").arg("/passive").arg("/a").arg("update.msi").status().unwrap();

    db.datemodified = datemodified;

    let mut f = File::create("db.toml").unwrap();
    write!(&mut f, "{}", toml::encode_str(&db)).unwrap();
  } else {
    println!("You have the latest version!");
  }
}
