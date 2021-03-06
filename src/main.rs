extern crate chrono;
extern crate reqwest;
extern crate syndication;
extern crate yaml_rust;
use chrono::prelude::*;
use git2::{Error, Repository, ResetType};
use gray_matter::engine::YAML;
use gray_matter::{Matter, ParsedEntity};
use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::io::Read;
use std::path::Path;
use syndication::Feed;
use yaml_rust::YamlLoader;

fn main() -> Result<(), io::Error> {
    let args: Vec<String> = env::args().collect();
    world(args)
}

fn world(mut args: Vec<String>) -> Result<(), io::Error> {
    // First command-line argument is the executable
    args.remove(0);
    // Second command-line argument is the project root folder
    let root = match args.pop() {
        Some(val) => val,
        None => "./disc".to_string(),
    };
    let project_path = Path::new(&root);

    let mut file = File::open(&project_path.join("disc.yaml"))?;
    let mut yaml = String::new();
    file.read_to_string(&mut yaml)?;
    let config = YamlLoader::load_from_str(&yaml).unwrap();
    let doc = &config[0];

    let output = project_path.join("content/post");
    fs::create_dir_all(&output)?;

    for blog in doc["blogs"].as_vec().unwrap().iter() {
        if let Err(e) = add(&output, blog.as_str().unwrap()) {
            eprintln!("failed to add {}: {}", &blog.as_str().unwrap(), e);
        };
    }
    // Take render mode from command-line
    if args.contains(&"--html".to_string()) {
        html(Path::new(&project_path))?;
    }
    Ok(())
}

fn html(project_path: &std::path::Path) -> Result<std::path::PathBuf, io::Error> {
    let output = project_path.join("public");
    fs::create_dir_all(&output)?;
    let mut index = File::create(&output.join("index.html")).expect("failed to prepare index.html");
    write!(index, "<body><ul>")?;

    for leaf in project_path
        .join("content/post")
        .read_dir()
        .expect("failed to read posts")
        .flatten()
    {
        if leaf.path().is_file() {
            let path = output.join(leaf.file_name()).with_extension("html");
            let mut file = File::create(&path)
                .expect(format!("failed to prepare output file: {:?}", path).as_str());
            let matter = extract_matter(leaf.path().as_path())
                .expect(format!("failed to read leaf: {:?}", leaf).as_str());
            let data = matter
                .data
                .expect(format!("failed to extract front matter: {:?}", leaf.path()).as_str());
            let title = data["title"].as_string().expect("no title");
            write!(
                file,
                "
    <title>{}</title>
    <body>{}</body>
    ",
                title, matter.content
            )?;
            write!(
                index,
                "<li><a href=\"./{:?}\">{}</a></li>",
                path.file_name().unwrap(),
                title
            )?;
        }
    }
    write!(index, "</ul></body>")?;
    Ok(output)
}

fn extract_matter(path: &Path) -> Result<ParsedEntity, io::Error> {
    let mut matter = Matter::<YAML>::new();
    matter.excerpt_delimiter = Some("<!--more-->".to_string());
    let mut file = File::open(path)?;
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    Ok(matter.parse(&text))
}

fn add(output: &std::path::Path, blog: &str) -> Result<std::path::PathBuf, io::Error> {
    let cache = output.join(".blogs");
    fs::create_dir_all(&cache)?;
    let path = Path::new(&cache).join(blog.replace('/', "-"));

    if blog.ends_with(".git") {
        if let Err(e) = clone_or_pull(blog, &path) {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to clone or pull {}: {}", blog, e),
            ));
        }
        let posts = path.join("content/post");
        for leaf in posts
            .read_dir()
            .expect(format!("failed to read posts: {}", blog).as_str())
            .flatten()
        {
            if leaf.path().is_dir() {
                continue;
            }
            let matter = extract_matter(leaf.path().as_path()).expect("failed to read leaf");
            let data = matter.data.as_ref().expect("invalid front matter");
            let tags = matter.data.as_ref().expect("no tags")["tags"]
                .as_vec()
                .expect("tags")
                .iter()
                .fold(",".to_string(), |cur, next| {
                    format!("{},{}", cur, next.as_string().unwrap())
                });
            post(
                output,
                &data["title"].as_string().expect("no title"),
                matter.content.as_str(),
                &tags.replace(",,", ""),
                &data["author"].as_string().unwrap_or(blog.to_string()),
                &data["date"].as_string().expect("no date"),
                blog,
            )?;
        }
    } else if blog.ends_with(".xml") {
        let mut body = String::new();
        if blog.starts_with("http") {
            let mut res = match reqwest::get(blog) {
                Ok(res) => res,
                Err(e) => {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("failed to download: {}", e),
                    ))
                }
            };
            body = res.text().unwrap();
        } else {
            let mut file = File::open(blog)?;
            file.read_to_string(&mut body)?;
        }
        match body.parse::<Feed>().unwrap() {
            Feed::Atom(atom) => {
                for item in atom.entries().iter() {
                    let summary = match item.summary() {
                        Some(summary) => summary,
                        None => match item.content() {
                            Some(content) => content
                                .value()
                                .expect(format!("no content: {}", item.id()).as_str()),
                            None => {
                                return Err(io::Error::new(
                                    io::ErrorKind::Other,
                                    "no summary or content",
                                ))
                            }
                        },
                    };
                    let author = match item.authors().to_vec().pop() {
                        Some(author) => author.name().to_string(),
                        None => blog.to_string(),
                    };
                    post(
                        output,
                        item.title(),
                        summary,
                        item.categories().to_vec().pop().expect("categories").term(),
                        author.as_str(),
                        rfc3339(item.published().unwrap_or(item.updated()))?.as_str(),
                        item.links().to_vec().pop().unwrap().href(),
                    )?;
                }
            }
            Feed::RSS(rss) => {
                for item in rss.items().iter() {
                    let link = item.link().expect(format!("no link: {}", blog).as_str());
                    post(
                        output,
                        item.title().unwrap(),
                        item.description().unwrap(),
                        item.categories().to_vec().pop().expect("categories").name(),
                        item.author().unwrap_or(blog),
                        rfc3339(
                            item.pub_date()
                                .expect(format!("no date: {}", link).as_str()),
                        )?
                        .as_str(),
                        link,
                    )?;
                }
            }
        };
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

fn rfc3339(date: &str) -> Result<String, io::Error> {
    let parsed = match DateTime::parse_from_rfc2822(date) {
        // Not using to_rfc3339() because we don't want the time here
        Ok(parsed) => format!("{}", parsed.format("%Y-%m-%d")),
        Err(_e) => match DateTime::parse_from_rfc3339(date) {
            // Not using to_rfc3339() because we don't want the time here
            Ok(parsed) => format!("{}", parsed.format("%Y-%m-%d")),
            Err(e) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("failed to parse as RFC 2822 or RFC 3339 {}: {}", date, e),
                ))
            }
        },
    };
    Ok(parsed)
}

fn post(
    output: &Path,
    title: &str,
    summary: &str,
    tags: &str,
    author: &str,
    date: &str,
    orig: &str,
) -> Result<String, io::Error> {
    let path = output.join(format!("{}-{}.md", date, title).replace('/', "-"));
    let mut file = File::create(&path).expect(format!("invalid path: {:?}", path).as_str());
    write!(
        file,
        "---
title: '{}'
date: {}
author: {}
tags: [{}]
original_link: {}
---
{}",
        title.replace('\'', "''"),
        date,
        author,
        tags,
        orig,
        summary
    )?;
    Ok(path.file_name().unwrap().to_str().unwrap().to_string())
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use git2::Signature;
    use tempfile::tempdir;

    #[test]
    fn test_post() -> Result<(), io::Error> {
        let output = tempdir()?;
        post(
            output.path(),
            "Lorem ipsum",
            "Dolor sit amet",
            "\"lorem\", \"ipsum\"",
            "Cicero",
            "2021-11-29T14:48:11+02:00",
            "http://example.com/2021-11-29-lorem-ipsum",
        )?;
        post(
            output.path(),
            "Sed ut perspiciatis",
            "unde omnis iste natus error",
            "\"perspiciatis\"",
            "Cicero",
            "Sat, 27 Nov 2021 15:32:10 +0100",
            "http://example.com/2021-11-29-sed-ut-perspiciatis",
        )?;
        post(
            output.path(),
            "Robert'); DROP TABLE Students; --",
            "Exploits of a mom",
            "\"bobby tables\"",
            "Little Bobby Tables",
            "Mon, 11 Jun 1999",
            "https://xkcd.com/327/",
        )?;
        post(
            output.path(),
            "Why ./ is better than \\",
            "The path is the goal",
            "",
            "",
            "2021-11-29",
            "http://example.com/path/to/happiness",
        )?;
        Ok(())
    }

    #[test]
    fn test_add() -> Result<(), io::Error> {
        let parent_folder = tempdir()?;
        let repo_path = parent_folder.path().join("blog.git");
        let posts = repo_path.join("content/post");
        fs::create_dir_all(&posts).unwrap();
        let project_path = parent_folder.path().join("project");
        let output = project_path.join("content/post");

        let repo = Repository::init(&repo_path).unwrap();
        let example = &posts.join("2020-02-02-example.md");
        let mut file = File::create(&example)?;
        write!(
            file,
            "---\ntitle: Example\ndate: 2020-02-02\ntags: [a, b]\nauthor: Cicero\n---\nLorem ipsum"
        )?;
        let mut index = repo.index().unwrap();
        index
            .add_path(Path::new("content/post/2020-02-02-example.md"))
            .unwrap();
        let sig = Signature::now("Example", "email@example.com").unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "example", &tree, &[])
            .unwrap();

        let blog = repo_path.to_str().unwrap().to_string();
        let path = output.join(".blogs").join(blog.replace('/', "-"));
        assert_eq!(add(&output, &blog).unwrap().file_name(), path.file_name());
        // Note that the generated filename is based on the title
        let matter = extract_matter(&output.join("2020-02-02-Example.md")).unwrap();
        let mut tags = matter.data.as_ref().expect("no tags")["tags"]
            .as_vec()
            .expect("tags");
        assert_eq!(tags.pop().unwrap().as_string(), Ok("b".to_string()));
        assert_eq!(tags.pop().unwrap().as_string(), Ok("a".to_string()));
        assert_eq!(
            matter.data.as_ref().unwrap()["author"].as_string(),
            Ok("Cicero".to_string())
        );
        // Repeat, this should be fine on an existing folder
        assert_eq!(add(&output, &blog).unwrap().file_name(), path.file_name());
        Ok(())
    }

    #[test]
    fn test_add_rss() -> Result<(), io::Error> {
        let project_path = tempdir()?;
        let output = project_path.path();
        let blog = "./example.rss.xml".to_string();
        add(&output, &blog)?;
        let matter = extract_matter(output.join("2021-08-10-Cogito ergo sum.md").as_path())?;
        assert_eq!(
            matter.data.as_ref().expect("no title")["title"].as_string(),
            Ok("Cogito ergo sum".to_string())
        );
        assert_eq!(
            matter.data.as_ref().unwrap()["original_link"].as_string(),
            Ok("http://example.com/2021/09/01/lorem-ipsum/".to_string())
        );
        let mut tags = matter.data.as_ref().unwrap()["tags"]
            .as_vec()
            .expect("tags");
        assert_eq!(tags.pop().unwrap().as_string(), Ok("b".to_string()));
        assert_eq!(
            matter.data.as_ref().unwrap()["author"].as_string(),
            Ok("Cicero".to_string())
        );
        Ok(())
    }

    #[test]
    fn test_add_atom() -> Result<(), io::Error> {
        let project_path = tempdir()?;
        let output = project_path.path();
        let blog = "./example.atom.xml".to_string();
        add(&output, &blog)?;
        let matter = extract_matter(output.join("2021-09-01-Cogito ergo sum.md").as_path())?;
        assert_eq!(
            matter.data.as_ref().unwrap()["title"].as_string(),
            Ok("Cogito ergo sum".to_string())
        );
        assert_eq!(
            matter.data.as_ref().unwrap()["original_link"].as_string(),
            Ok("http://example.com/2021/09/01/lorem-ipsum/".to_string())
        );
        let mut tags = matter.data.as_ref().expect("no tags")["tags"]
            .as_vec()
            .expect("tags");
        assert_eq!(tags.pop().unwrap().as_string(), Ok("b".to_string()));
        assert_eq!(
            matter.data.as_ref().unwrap()["author"].as_string(),
            Ok("Cicero".to_string())
        );
        Ok(())
    }

    #[test]
    #[should_panic(expected = "unknown blog type")]
    fn test_add_invalid() {
        let project_path = tempdir().unwrap();
        let output = project_path.path().join("content/post");
        add(&output, "http://example.com/file.txt").unwrap();
    }

    #[test]
    fn test_args() {
        let project_path = tempdir().unwrap();
        fs::create_dir(&project_path.path().join("foo")).unwrap();
        let mut file = File::create(&project_path.path().join("foo/disc.yaml")).unwrap();
        let blog = env::current_dir().unwrap().join("example.rss.xml");
        write!(file, "blogs: [{:?}]", blog).unwrap();
        world(vec![
            "self".to_string(),
            project_path
                .path()
                .join("foo")
                .to_str()
                .unwrap()
                .to_string(),
        ])
        .unwrap();
        assert!(Path::new(
            &project_path
                .path()
                .join("foo/content/post/2021-08-10-Cogito ergo sum.md")
        )
        .exists());

        fs::create_dir(&project_path.path().join("disc")).unwrap();
        file = File::create(&project_path.path().join("disc/disc.yaml")).unwrap();
        write!(file, "blogs: [{:?}]", blog).unwrap();
        env::set_current_dir(&project_path.path()).unwrap();
        world(vec!["self".to_string()]).unwrap();
        world(vec!["self".to_string(), "disc".to_string()]).unwrap();
        assert!(Path::new(
            &project_path
                .path()
                .join("disc/content/post/2021-08-10-Cogito ergo sum.md")
        )
        .exists());

        world(vec![
            "self".to_string(),
            "--html".to_string(),
            "disc".to_string(),
        ])
        .unwrap();
        assert!(Path::new(
            &project_path
                .path()
                .join("disc/public/2021-08-10-Cogito ergo sum.html")
        )
        .exists());
        assert!(Path::new(&project_path.path().join("disc/public/index.html")).exists());
    }
}
