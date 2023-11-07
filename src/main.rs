use std::{path::PathBuf, fs::Metadata, time::SystemTime, ffi::OsString, os::unix::prelude::OsStrExt};

use chrono::{DateTime, Local};
use clap::{Parser, FromArgMatches, Args};
use colored::Colorize;
use walkdir::WalkDir;

use indexmap::IndexMap;

const ABOUT: &str = "
lsn lists directory contents with large numbered file lists by grouping files with common roots together.";

const EXAMPLES: &str = "
EXAMPLES:

List numbered files:

$ lsn
";

#[derive(Parser, Debug)]
#[clap(author, version, about = ABOUT, name = "lsn")]
#[clap(after_long_help(EXAMPLES))]
struct Opt {
    /// A directory whose contents need to be printed.
    #[clap(default_value = ".")]
    path: String,

    #[clap(short, long)]
    all: bool,

    #[clap(short = 'U', long = "unsorted")]
    unsorted: bool,

    #[clap(long, default_value = "1")]
    depth: usize,

    #[clap(short = 'L', long)]
    follow_links: bool,

    #[clap(short = 'l', long)]
    long: bool,

    #[clap(short = 't', long)]
    sort_by_modified: bool,

    #[clap(short = 'S', long)]
    sort_by_size: bool,

    #[clap(short = 'r', long)]
    reverse: bool,

    #[clap(short = 'n', long)]
    nocolor: bool,
}

#[derive(Clone, Debug)]
pub struct Meta {
    modified: Option<SystemTime>,
    accessed: Option<SystemTime>,
    created: Option<SystemTime>,
    size: u64,
    is_dir: bool,
    is_symlink: bool,
}

impl From<Metadata> for Meta {
    fn from(value: Metadata) -> Self {
        Meta {
            modified: value.modified().ok(),
            accessed: value.accessed().ok(),
            created: value.created().ok(),
            size: value.len(),
            is_dir: value.is_dir(),
            is_symlink: value.is_symlink(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FileGroup {
    /// Range of numbers in files with the same root and extension.
    pub range: Option<std::ops::Range<usize>>,
    pub parent: Option<PathBuf>,
    pub stem: OsString,
    pub ext: OsString,
    pub meta: Option<Meta>,
}

impl FileGroup {
    pub fn modified(&self) -> Option<SystemTime> {
        self.meta.as_ref().and_then(|meta| meta.modified)
    }
    pub fn accessed(&self) -> Option<SystemTime> {
        self.meta.as_ref().and_then(|meta| meta.accessed)
    }
    pub fn created(&self) -> Option<SystemTime> {
        self.meta.as_ref().and_then(|meta| meta.created)
    }
    pub fn size(&self) -> Option<u64> {
        self.meta.as_ref().map(|meta| meta.size)
    }
    pub fn is_dir(&self) -> bool {
        self.meta.as_ref().map(|meta| meta.is_dir).unwrap_or(false)
    }
    pub fn is_symlink(&self) -> bool {
        self.meta.as_ref().map(|meta| meta.is_symlink).unwrap_or(false)
    }
}

fn main() {
    let cli = clap::Command::new("lsn");
    let cli = Opt::augment_args(cli);
    let matches = cli.get_matches();
    let opt = Opt::from_arg_matches(&matches).unwrap();

    let glob_options = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };

    let entries = glob::glob_with(&opt.path, glob_options).unwrap();

    let regex = lsn::build_regex();

    let mut map: IndexMap<String, FileGroup> = IndexMap::new();

    for path in entries.filter_map(|e| e.ok()) {
        for entry in WalkDir::new(path).max_depth(opt.depth).follow_links(opt.follow_links).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            let parent = path.parent().map(ToOwned::to_owned);
            let stem = path.file_stem().map(ToOwned::to_owned).unwrap_or(OsString::from("."));
            if !opt.all && stem.as_os_str().as_bytes()[0] == b'.' {
                continue;
            }
            let extension = path.extension().map(ToOwned::to_owned).unwrap_or(OsString::from(""));
            let mut ext = OsString::new();
            if !extension.is_empty() {
                ext.push(".");
                ext.push(extension);
            }
            let file_name_str = path.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or("..".to_string()); // Used to create a key.
            let Some(caps) = regex.captures(&file_name_str) else {
                // Default range of size one will be treated as a single file and not a group anyways.
                map.insert(file_name_str.to_string(), FileGroup { range: None, parent, stem, ext, meta: entry.metadata().ok().map(Meta::from) });
                continue;
            };

            let key = format!("{}#{}", &caps["stem"], &caps["ext"]);
            let num = caps["num"].parse::<usize>().unwrap();
            // println!("{}, {}, {}, {}", &key, &file_name_str, num, &caps["ext"]);
            let meta = entry.metadata().ok().map(Meta::from);
            map.entry(key.clone()).and_modify(
                |grp| {
                    // Update range
                    let range = grp.range.as_mut().unwrap();
                    // println!("key: {}; range: {}..{}", key, range.start, range.end);
                    range.start = range.start.min(num);
                    range.end = range.end.max(num+1);

                    if let Some((grp_meta, meta)) = grp.meta.as_mut().zip(meta.as_ref()) {
                        // Update last modified and last accessed metadata
                        if let Some((grp_modified, cur_modified)) = grp_meta.modified.as_mut().zip(meta.modified) {
                            *grp_modified = (*grp_modified).max(cur_modified);
                        }
                        if let Some((grp_accessed, cur_accessed)) = grp_meta.accessed.as_mut().zip(meta.accessed) {
                            *grp_accessed = (*grp_accessed).max(cur_accessed);
                        }
                        if let Some((grp_created, cur_created)) = grp_meta.created.as_mut().zip(meta.created) {
                            *grp_created = (*grp_created).min(cur_created);
                        }
                        grp_meta.size += meta.size;
                    }
                }
            ).or_insert(FileGroup { range: Some(num..num+1), parent, stem: OsString::from(&caps["stem"]), ext: OsString::from(&caps["ext"]), meta });
        }
    }

    let mut vec: Vec<_> = map.into_values().collect();
    
    let option_names = vec![
        "sort_by_modified",
        "sort_by_size"
    ];
    let mut sort_options = option_names.iter().filter(|name| {
        matches.get_flag(name)
    }).map(|&name| {
        (name, matches.index_of(name))
    }).collect::<Vec<_>>();
    sort_options.sort_by(|(_, i), (_, j)| {
        i.cmp(j)
    });

    if !opt.unsorted || !sort_options.is_empty() {
        vec.sort_by(|a,b| {
            let mut less = std::cmp::Ordering::Equal;
            if let Some((a_meta, b_meta)) = a.meta.as_ref().zip(b.meta.as_ref()) {
                for (option, _) in sort_options.iter() {
                    match *option {
                        "sort_by_modified" => {
                            less = less.then(
                                a_meta.modified.cmp(&b_meta.modified)
                            );
                        }
                        "sort_by_size" => {
                            less = less.then(
                                a_meta.size.cmp(&b_meta.size)
                            );
                        }
                        _ => {}
                    }
                }
            }
            if !opt.unsorted {
                less = less.then(a.stem.cmp(&b.stem));
                if let Some((a_range, b_range)) = a.range.as_ref().zip(b.range.as_ref()) {
                    less = less.then(a_range.clone().cmp(b_range.clone()));
                }
                // println!("comparing {:?}{:?} to {:?}{:?}: {:?}", &a.stem, &a.ext, &b.stem, &b.ext, a.ext.cmp(&b.ext));
                less = less.then(a.ext.cmp(&b.ext));
            }
            if opt.reverse {
                less.reverse()
            } else {
                less
            }
        });
    }

    let max_size = vec.iter().map(|x| x.size().unwrap_or(0)).max().unwrap_or(0);
    let length_for_size = if max_size > 0 {
        max_size.ilog10() as usize + 1
    } else {
        1
    };

    for value in vec.iter() {
        let mut path = if opt.depth > 1 {
            value.parent.clone().unwrap_or_else(|| PathBuf::from("/"))
        } else {
            PathBuf::new()
        };
        if let Some(ref range) = value.range {
            let mut file_name = value.stem.clone();
            if range.end - range.start == 1 {
                file_name.push(range.start.to_string());
                file_name.push(&value.ext);
            } else {
                file_name.push("#");
                file_name.push(&value.ext);
                file_name.push(format!(" ({}..{})", range.start, range.end-1));
            }
            path.push(file_name);
        } else {
            let mut filename = value.stem.clone();
            filename.push(&value.ext);
            path.push(filename);
        }
        if opt.long {
            let time = value.modified().map(|time| DateTime::<Local>::from(time).format("%b %_d %H:%M").to_string()).unwrap_or(String::new());
            let size = value.size().map(|size| size.to_string()).unwrap_or(String::new());
            print!("{:>length_for_size$} {} ", size, time);
        }
        let path_str = path.to_string_lossy();
        if opt.nocolor {
            if value.is_dir() {
                println!("{}/", path_str);
            } else if value.is_symlink() {
                println!("{}@", path_str);
            } else {
                println!("{}", path_str);
            }
        } else if value.is_dir() {
            println!("{}/", path_str.blue());
        } else if value.is_symlink() {
            println!("{}@", path_str.magenta());
        } else {
            println!("{}", path_str);
        }
    }
}
