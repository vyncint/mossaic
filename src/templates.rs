//! The catalogue of pixel-art templates.
//!
//! Two sources, looked up in this order:
//!
//! 1. **Built in.** Every `art/templates/*.art` file in the repository, baked
//!    into the binary at compile time by `build.rs`. Contributing one is
//!    dropping a file into that directory and opening a pull request — there
//!    is no list to remember to edit, which is the whole design.
//! 2. **Yours.** `.art` files in `./templates/` and in
//!    `$XDG_CONFIG_HOME/mossaic/templates/` (or `~/.config/mossaic/templates/`),
//!    so a template can be tried, kept and used without it ever being anyone
//!    else's business.
//!
//! A local file shadows a built-in one of the same name. That way a template
//! you are working on can be tested under the name it will eventually ship
//! with, rather than under a temporary one you then have to remember to change.

use std::path::{Path, PathBuf};

use crate::art::Canvas;

include!(concat!(env!("OUT_DIR"), "/templates.rs"));

/// Where a template came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// Shipped with the crate, from `art/templates/`.
    Builtin,
    /// Read from a file on this machine.
    Local(PathBuf),
}

impl std::fmt::Display for Origin {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Builtin => write!(out, "built in"),
            Self::Local(path) => write!(out, "{}", path.display()),
        }
    }
}

/// One template in the catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    /// What you type after `--template`: the file's stem.
    pub name: String,
    /// The picture.
    pub canvas: Canvas,
    /// Where it was found.
    pub origin: Origin,
}

impl Template {
    /// The display name from the file's header, falling back to its stem.
    #[must_use]
    pub fn title(&self) -> &str {
        self.canvas
            .meta()
            .name
            .as_deref()
            .unwrap_or(self.name.as_str())
    }

    /// The one-line description from the file's header, if it has one.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.canvas.meta().description.as_deref()
    }

    /// Who wrote it, if the file says.
    #[must_use]
    pub fn author(&self) -> Option<&str> {
        self.canvas.meta().author.as_deref()
    }
}

/// The directories local templates are read from, in the order they shadow.
///
/// `./templates/` first, because a template being worked on is usually beside
/// the work; then the config directory, which is where one that has earned its
/// keep gets moved to.
#[must_use]
pub fn local_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("templates")];
    if let Some(config) = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    {
        dirs.push(config.join("mossaic").join("templates"));
    }
    dirs
}

/// Every template, local ones first, each name appearing once.
///
/// Sorted by name, so a listing reads the same on every machine. A file that
/// does not parse is skipped rather than fatal: one broken template in a
/// directory must not take out `--list-templates`, which is the command you
/// would reach for to find out which one is broken.
#[must_use]
pub fn catalogue() -> Vec<Template> {
    let mut found: Vec<Template> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    for dir in local_dirs() {
        for (name, canvas) in read_dir(&dir).0 {
            if !seen.contains(&name) {
                seen.push(name.clone());
                found.push(Template {
                    name,
                    canvas,
                    origin: Origin::Local(dir.clone()),
                });
            }
        }
    }
    for (name, source) in BUILTIN {
        if seen.iter().any(|taken| taken == name) {
            continue;
        }
        // A built-in that does not parse is a bug in this repository, and the
        // test in `render_tests` fails the build before it can ship. Skipping
        // it here rather than panicking keeps a corrupted install usable.
        if let Ok(canvas) = Canvas::parse(source) {
            found.push(Template {
                name: (*name).to_string(),
                canvas,
                origin: Origin::Builtin,
            });
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// A `.art` file in a template directory that could not be read.
///
/// The skip is deliberate policy — one broken template must not take out
/// `--list-templates` — but the file was skipped *in silence*, and
/// `--list-templates` is precisely "the command you would reach for to find
/// out which one is broken". It named nothing, counted nothing, and wrote no
/// byte to stderr. So `--template mine` answered "no template named mine"
/// for a file sitting right there under that name, and the natural
/// conclusion was that the lookup was wrong rather than the file.
#[derive(Debug, Clone)]
pub struct Skipped {
    /// The file name, as a listing should print it.
    pub file: String,
    /// The stem, so `--template <stem>` can find the reason.
    pub stem: String,
    /// What the parser said.
    pub why: String,
}

/// Every `.art` file in one directory, as `(stem, canvas)`, plus what was
/// skipped and why.
fn read_dir(dir: &Path) -> (Vec<(String, Canvas)>, Vec<Skipped>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (Vec::new(), Vec::new());
    };
    let mut found = Vec::new();
    let mut skipped = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("art") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let file = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(stem)
            .to_string();
        let body = match std::fs::read_to_string(&path) {
            Ok(body) => body,
            Err(error) => {
                skipped.push(Skipped {
                    file,
                    stem: stem.to_string(),
                    why: error.to_string(),
                });
                continue;
            }
        };
        match Canvas::parse(&body) {
            Ok(canvas) => found.push((stem.to_string(), canvas)),
            Err(why) => skipped.push(Skipped {
                file,
                stem: stem.to_string(),
                why,
            }),
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    skipped.sort_by(|a, b| a.file.cmp(&b.file));
    (found, skipped)
}

/// Every local `.art` file that could not be read, across every directory.
///
/// Separate from [`catalogue`] because most callers do not want it; the
/// listing does, and so does the `--template` miss.
#[must_use]
pub fn skipped() -> Vec<Skipped> {
    local_dirs()
        .iter()
        .flat_map(|dir| read_dir(dir).1)
        .collect()
}

/// Find one template by name, or say what there was to choose from.
///
/// The error lists the catalogue because the alternative — "no such template" —
/// leaves you running a second command to find out what you meant, and the
/// catalogue is short enough to print.
pub fn find(name: &str) -> Result<Template, String> {
    let catalogue = catalogue();
    let found = catalogue.iter().find(|template| template.name == name);
    // A local file with that stem is there and did not parse.
    //
    // Reported even when a built-in of the same name exists, which is the
    // shadowing case: the same command drew two different pictures depending
    // on whether the user's file happened to parse, with nothing printed
    // either way. Somebody who put `templates/dragon.art` there meant it.
    if found.is_none_or(|template| template.origin == Origin::Builtin) {
        if let Some(broken) = skipped().into_iter().find(|s| s.stem == name) {
            return Err(format!("{}: {}", broken.file, broken.why));
        }
    }
    if let Some(found) = found {
        return Ok(found.clone());
    }
    if catalogue.is_empty() {
        return Err(format!(
            "no template named {name:?}, and none are installed"
        ));
    }
    let names: Vec<&str> = catalogue
        .iter()
        .map(|template| template.name.as_str())
        .collect();
    Err(format!(
        "no template named {name:?} — there is {}",
        names.join(", ")
    ))
}

/// The built-in catalogue as `(name, source)`, for tests that check the files
/// themselves rather than what parsing made of them.
#[must_use]
pub fn builtin_sources() -> &'static [(&'static str, &'static str)] {
    BUILTIN
}
