use std::fs;
use std::path::Path;

#[derive(Deserialize, Serialize, Debug)]
pub struct Store {}

impl Store {
  pub fn load<P: AsRef<Path>>(path: P) -> Self {
    let _ = fs::read(path);
    Store {}
  }
}
