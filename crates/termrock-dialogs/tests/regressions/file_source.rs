use termrock_dialogs::file_browser::{FileEntry, FileSource};

struct Remote;

impl FileSource for Remote {
    fn list(&self, _dir: &str) -> Result<Vec<FileEntry>, String> {
        Ok(Vec::new())
    }

    fn is_dir(&self, path: &str) -> bool {
        path.ends_with('/')
    }

    fn home(&self) -> Option<String> {
        Some("/sandbox".into())
    }

    fn parent(&self, path: &str) -> Option<String> {
        path.strip_suffix("/x").map(str::to_owned)
    }

    fn display(&self, path: &str) -> String {
        format!("remote:{path}")
    }

    fn expand(&self, typed: &str) -> String {
        typed.replace('@', "/sandbox")
    }
}

#[test]
fn a_boxed_source_answers_with_the_overrides_of_the_source_inside() {
    let boxed: Box<dyn FileSource> = Box::new(Remote);

    assert!(boxed.is_dir("/a/"));
    assert_eq!(boxed.home().as_deref(), Some("/sandbox"));
    assert_eq!(boxed.parent("/a/x").as_deref(), Some("/a"));
    assert_eq!(boxed.parent("/a/y"), None);
    assert_eq!(boxed.display("/a"), "remote:/a");
    assert_eq!(boxed.expand("@/p"), "/sandbox/p");
    assert_eq!(boxed.list("/"), Ok(Vec::new()));
}

struct Slashes;

impl FileSource for Slashes {
    fn list(&self, _dir: &str) -> Result<Vec<FileEntry>, String> {
        Ok(Vec::new())
    }

    fn is_dir(&self, _path: &str) -> bool {
        true
    }
}

#[test]
fn a_boxed_default_source_still_climbs_from_a_first_level_directory_to_root() {
    let boxed: Box<dyn FileSource> = Box::new(Slashes);

    assert_eq!(boxed.parent("/a").as_deref(), Some("/"));
    assert_eq!(boxed.parent("/").as_deref(), None);
}
