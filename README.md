# 🌍️🦀️ World Crab

The world is changing. A mouse got lose and fell off the discworld. Consequently the elephants got scared and hopped off Great A'Tuin's back. As luck would have it a gigantic crab with four gophers on its back took its place.

![GitHub Actions](https://github.com/kalikiana/worldcrab/actions/workflows/test.yml/badge.svg)
![Crates.io](https://img.shields.io/crates/d/worldcrab)
[![Open in Gitpod](https://gitpod.io/button/open-in-gitpod.svg)](https://gitpod.io/#https://github.com/kalikiana/worldcrab)

## What's this project about?

A static meta blog generator aka a planet.

Imagine you have a bunch of static blogs made with [Hugo](https://gohugo.io/) and you're looking for a way to aggregate those blogs easily without worrying too much about the details and without duplicating metadata. If this sounds too good to be true, the world crab is for you!

## 🔧️ How do I use this?

### Setup a static website

The world crab's task is to populate the disc (or planet). Which means you initially need to generate the actual page with a static generator. This can be done with [Hugo](https://github.com/gohugoio/hugo), [Jekyll](https://jekyllrb.com) or [Sculpin](https://sculpin.io). An example with *Hugo* could be setup like so:

```bash
hugo new site -f yaml disc
git submodule add https://github.com/halogenica/beautifulhugo.git disc/themes/beautifulhugo
sed -i s@.Permalink@.Params.original_link@ disc/themes/beautifulhugo/layouts/partials/*.html
hugo -s disc -t beautifulhugo
```

This creates a new, empty site using the BeautifulHugo theme. The **sed** command here is crucial because it turns a regular blog into an aggregator where all posts link back to the original blog!

### Generate aggregated posts

Be sure to create a file `disc.yaml` containing the repos you want to include:

```yaml
blogs:
- https://gitlab.com/kalikiana/kalikiana.gitlab.io.git
- https://openqa-bites.github.io/index.xml
- http://dominique.leuenberger.net/blog/feed/
```

### Installation

This project is built using cargo, which means you can install it directly from git:

```bash
cargo install --git https://github.com/kalikiana/worldcrab
worldcrab disc
```

or build it from a local clone, which also allows you to work on it and contribute patches:

```
git clone https://github.com/kalikiana/worldcrab
cargo run disc
```

The world crab will expect to find a file `disc.yaml` at the root of the folder containing your static site and the first argument is simply your root folder.

**Note:** You will need `libopenssl-devel` on openSUSE, `libssl-dev` on Ubuntu or `openssl-devel` on Fedora respectively.

### Render modes

By default the world crab produces markdown with front matter written in YAML, which can be processed with **Hugo** or another static site generator. The `html` renderer can also produce a static website from that:

    worldcrab --html disc

## 👷️ Contributing

### How do you validate your code?

Rather than relying on manual testing or waiting for CI you can locally check that your code is sound and also formatted properly:

```bash
cargo fmt
cargo clippy
cargo test
```
