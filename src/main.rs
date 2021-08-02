extern crate reqwest;
extern crate yaml_rust;
use git2::{Error, Repository, ResetType};
use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;
use yaml_rust::YamlLoader;

fn main() -> Result<(), io::Error> {
    let mut args: Vec<String> = env::args().collect();
    // First command-line argument is the executable
    args.remove(0);
    // Second command-line argument is the project root folder
    let project_path = match args.pop() {
        Some(val) => val,
        None => "./disc".to_string(),
    };

    let mut file = File::open("disc.yaml")?;
    let mut yaml = String::new();
    file.read_to_string(&mut yaml)?;
    let config = YamlLoader::load_from_str(&yaml).unwrap();
    let doc = &config[0];

    let output = Path::new(&project_path).join("content/post");
    fs::create_dir_all(&output)?;

    for blog in doc["blogs"].as_vec().unwrap().iter() {
        if let Err(e) = add(&output, blog.as_str().unwrap()) {
            eprintln!("failed to add {}: {}", &blog.as_str().unwrap(), e);
        };
    }
    Ok(())
}

fn add(output: &std::path::Path, blog: &str) -> Result<std::path::PathBuf, io::Error> {
    let cache = output.join(".blogs");
    fs::create_dir_all(&cache)?;
    let path = Path::new(&cache).join(blog.replace("/", "-"));

    if blog.ends_with(".git") {
        if let Err(e) = clone_or_pull(&blog, &path) {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to clone or pull: {}", e),
            ));
        }
        let posts = path.join("content/post");
        for post in posts.read_dir().expect("failed to read posts").flatten() {
            if !output.join(post.file_name()).exists() {
                fs::hard_link(post.path(), output.join(post.file_name()))?
            };
        }
    } else {
        return Err(io::Error::new(io::ErrorKind::Other, "unknown blog type"));
    }
    Ok(path)
}

fn clone_or_pull(blog: &str, path: &Path) -> Result<(), Error> {
    if !path.is_dir() {
        Repository::clone(blog, &path)?;
        return Ok(());
    }

    // Reset
    let repo = Repository::open(path)?;
    repo.reset(&repo.revparse_single("HEAD")?, ResetType::Hard, None)?;

    // Pull
    let default = repo.find_reference("refs/remotes/origin/HEAD")?;
    let branch = default.symbolic_target().unwrap_or("master");
    repo.find_remote("origin")?.fetch(&[branch], None, None)?;
    let head = repo.find_reference("HEAD")?;
    let commit = repo.reference_to_annotated_commit(&head)?;
    let analysis = repo.merge_analysis(&[&commit])?;
    if analysis.0.is_up_to_date() {
        return Ok(());
    }
    if analysis.0.is_fast_forward() {
        let refname = format!("refs/heads/{}", branch);
        let mut reference = repo.find_reference(&refname)?;
        reference.set_target(commit.id(), "Fast-Forward")?;
        repo.set_head(&refname)?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_add() -> Result<(), io::Error> {
        let project_path = tempdir()?;
        let output = Path::new(&project_path.path()).join("content/post");

        let blog = "https://gitlab.com/kalikiana/kalikiana.gitlab.io.git";
        let path = project_path
            .path()
            .join("content/post/.blogs")
            .join(blog.replace("/", "-"));
        assert_eq!(add(&output, &blog)?.file_name(), path.file_name());
        // Repeat, this should be fine on an existing folder
        assert_eq!(add(&output, &blog)?.file_name(), path.file_name());
        Ok(())
    }

    #[test]
    #[should_panic(expected = "unknown blog type")]
    fn test_add_invalid() {
        let project_path = tempdir().unwrap();
        let output = project_path.path().join("content/post");
        add(&output, "http://example.com/file.txt").unwrap();
    }
}