use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Project {
    path: PathBuf,
    dirty: AtomicBool,
}

impl Project {
    pub fn new(path: PathBuf) -> Self {
        Project {
            path,
            dirty: AtomicBool::new(false),
        }
    }

    pub fn list_contents(&self) -> ProjectDirectoryContents {
        let mut files = vec![];
        list_project_files(&mut files, &self.path, &PathBuf::new());

        ProjectDirectoryContents {
            files: files
                .into_iter()
                .filter_map(|path| path.to_str().map(Into::into))
                .collect(),
        }
    }

    pub fn read_file(&self, path: &str) -> ReadFileResult {
        if path.starts_with('/') || path.starts_with('.') || path.contains("..") {
            return ReadFileResult {
                error: Some("Invalid path.".to_string()),
                contents: None,
            };
        }

        let path = self.path.join(path);
        match std::fs::read_to_string(path) {
            Ok(contents) => ReadFileResult {
                error: None,
                contents: Some(contents),
            },
            Err(_) => ReadFileResult {
                error: Some("Cannot read file.".to_string()),
                contents: None,
            },
        }
    }

    pub fn write_file(&self, path: &str, contents: &str) -> WriteFileResult {
        if path.starts_with('/') || path.starts_with('.') || path.contains("..") {
            WriteFileResult {
                error: Some("Invalid path.".to_string()),
            }
        } else {
            let path = self.path.join(path);

            let Some(parent) = path.parent() else {
                return WriteFileResult {
                    error: Some("Invalid path.".to_string()),
                };
            };
            if !parent.is_dir() && !parent.exists() {
                std::fs::create_dir_all(parent).unwrap();
            }

            self.dirty.store(true, Ordering::Release);
            match std::fs::write(path, contents) {
                Ok(_) => WriteFileResult { error: None },
                Err(_) => WriteFileResult {
                    error: Some("Cannot write file.".to_string()),
                },
            }
        }
    }

    pub fn run_cargo_check(&self) -> Option<String> {
        let output = std::process::Command::new("./run_cargo_check")
            .arg(&self.path)
            .output()
            .unwrap();
        let output = String::from_utf8(output.stdout).unwrap();
        if output.is_empty() {
            None
        } else {
            Some(output)
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Acquire)
    }

    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Release);
    }
}

#[derive(Serialize, Deserialize)]
pub struct ProjectDirectoryContents {
    pub files: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadFileArgs {
    /// a relative path to the file in the project directory
    pub path: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct WriteFileArgs {
    /// a relative path to the file in the project directory
    pub path: String,
    /// new contents of a file
    pub contents: String,
}

#[derive(Serialize, Deserialize)]
pub struct ReadFileResult {
    pub error: Option<String>,
    pub contents: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct WriteFileResult {
    pub error: Option<String>,
}

fn list_project_files(files: &mut Vec<PathBuf>, path: &Path, relpath: &Path) {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let path2 = path.join(&file_name);
            let relpath2 = relpath.join(&file_name);

            if is_not_important_path(&path2, &relpath2) {
                continue;
            }

            if path2.is_dir() {
                list_project_files(files, &path2, &relpath2);
            } else if path2.is_file() {
                files.push(relpath2);
            }
        }
    }
}

fn is_not_important_path(path: &Path, relpath: &Path) -> bool {
    if path.is_dir() {
        relpath == Path::new(".git") || relpath == Path::new("target")
    } else if path.is_file() {
        relpath == Path::new(".gitignore")
            || relpath == Path::new(".env")
            || relpath == Path::new("Cargo.lock")
            || relpath == Path::new("LICENSE")
            || relpath == Path::new("LICENSE.txt")
    } else {
        true
    }
}

#[test]
fn test_list_files() {
    let project = Project::new("project_src".into());
    let contents = project.list_contents();
    assert_eq!(contents.files.len(), 1);
}
