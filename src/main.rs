use std::{collections::HashSet, fs, path::{Path, PathBuf}};

use anyhow::{anyhow, Context, Result};
use clap::{Parser, ValueEnum};

// Number of errors to allow before exiting
const ERROR_LIMIT: usize = 20;

// Time limit for errors to occur within (in seconds)
const ERROR_TIME_LIMIT: u64 = 5;

#[derive(Debug, Parser)]
#[clap(version)]
struct Args {
    /// The directories to watch
    /// (e.g. "C:\Users\user\Documents" or "test/test")
    #[clap(value_parser, num_args = 1.., required = true, verbatim_doc_comment)]
    directories: Vec<String>,

    /// The file names to automatically hide
    /// (e.g. "file.txt" or "file")
    #[clap(short = 'n', long, value_parser, num_args = 1.., verbatim_doc_comment)]
    file_names: Vec<String>,

    /// The file extensions to automatically hide
    /// (e.g. "txt" or ".txt")
    #[clap(short = 'x', long, value_parser, num_args = 1.., verbatim_doc_comment)]
    file_extensions: Vec<String>,

    /// Switch to enable recursive watching
    /// (i.e. watch all subdirectories)
    /// [default: false]
    #[clap(short, long, default_value = "false", verbatim_doc_comment)]
    recursive: bool,

    /// Switch to enable case sensitivity in file names and extensions
    /// (e.g. "file.txt" and "FILE.TXT" are the same)
    /// [default: false]
    #[clap(short = 'c', long, default_value = "false", verbatim_doc_comment)]
    case_sensitive: bool,

    /// Switch to enable test mode. In test mode, the program will not actually hide files
    /// and will instead print the paths of the files that would be hidden.
    /// [default: false]
    #[clap(long = "test", default_value = "false", verbatim_doc_comment)]
    test_mode: bool,

    /// Switch to enable watch mode, which will watch for changes to the files and directories
    /// and automatically hide them.
    /// [default: false]
    #[clap(short, long, default_value = "false", verbatim_doc_comment)]
    watch: bool,

    /// Switch to enable immediate mode, which will immediately hide all files and directories
    /// that match the given file names and extensions.
    /// [default: true]
    #[clap(short, long, default_value = "false", verbatim_doc_comment)]
    immediate: bool,

    /// Types of files to hide
    #[clap(short = 't', long, value_parser, num_args = 1.., value_delimiter = ' ', default_value = "file directory", verbatim_doc_comment)]
    file_types: Vec<FileType>,
}

// Enum for the file types to hide
#[derive(Clone, Copy, Debug, PartialEq, ValueEnum)]
enum FileType {
    File,
    Directory,
}

fn main() -> Result<()> {
    // Parse the command line arguments
    let args: Args = Args::parse();

    // Create the set of directories to watch, validating that they exist and are directories. Return
    // an error if they don't exist or aren't directories.
    let (directories, file_names, file_extensions) = setup(
        args.directories,
        args.file_names,
        args.file_extensions,
        args.case_sensitive,
    )?;

    // Set up the rest of the configuration
    let recursive = args.recursive;
    let case_sensitive = args.case_sensitive;
    let hide_files = args.file_types.contains(&FileType::File);
    let hide_directories = args.file_types.contains(&FileType::Directory);
    let test_mode = args.test_mode;

    // If test mode is enabled, then print a message saying that test mode is enabled and no files
    // will be hidden.
    if test_mode {
        println!("Test mode enabled. No files will be hidden.");
    }

    // Print an error message if both watch mode and immediate mode are disabled.
    if !args.watch && args.immediate {
        return Err(anyhow!("Both watch mode and immediate mode are disabled. At least one of these modes must be enabled."));
    }

    // If immediate mode is enabled, then immediately hide all files and directories that match the
    // given file names and extensions.
    if !args.immediate {
        if test_mode {
            println!("Running immediate mode...");
        }
        immediate_mode(
            &directories,
            &file_names,
            &file_extensions,
            recursive,
            case_sensitive,
            hide_files,
            hide_directories,
            test_mode,
        );
    }

    // If watch mode is enabled, then watch for changes to the files and directories and automatically
    // hide them.
    if args.watch {
        if test_mode {
            println!("Running watch mode...");
        }
        watch_mode(
            &directories,
            &file_names,
            &file_extensions,
            recursive,
            case_sensitive,
            hide_files,
            hide_directories,
            test_mode,
        )
    } else {
        Ok(())
    }
}

// Immediate mode function
fn immediate_mode(
    directories: &HashSet<PathBuf>,
    file_names: &HashSet<String>,
    file_extensions: &HashSet<String>,
    recursive: bool,
    case_sensitive: bool,
    hide_files: bool,
    hide_directories: bool,
    test_mode: bool,
) {
    use walkdir::WalkDir;

    // Small helper function to get a path from an entry result. Used to have consistent error
    // messages.
    fn get_path(entry: &walkdir::Result<walkdir::DirEntry>) -> Option<PathBuf> {
        match entry {
            Ok(entry) => Some(entry.path().to_path_buf()),
            Err(e) => e.path().map(|p| p.to_path_buf()),
        }
    }

    for directory in directories {
        for entry in if recursive {
            WalkDir::new(directory)
        } else {
            WalkDir::new(directory).min_depth(1).max_depth(1)
        } {
            let path = get_path(&entry);

            if entry.is_err() {
                let entry = entry.with_context(|| {
                    if let Some(path) = path {
                        format!("Failed to get path from entry: {}", path.display())
                    } else {
                        "Failed to get path from entry".to_string()
                    }
                });

                eprintln!("{}", entry.unwrap_err());
                continue;
            } else {
                if let Err(e) = handle_path(
                    &path.unwrap(),
                    &file_names,
                    &file_extensions,
                    case_sensitive,
                    hide_files,
                    hide_directories,
                    test_mode,
                ) {
                    eprintln!("{}", e);
                }
            }
        }
    }
}

// Watch mode function
fn watch_mode(
    directories: &HashSet<PathBuf>,
    file_names: &HashSet<String>,
    file_extensions: &HashSet<String>,
    recursive: bool,
    case_sensitive: bool,
    hide_files: bool,
    hide_directories: bool,
    test_mode: bool,
) -> Result<()> {
    use notify::{event, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;

    // Open a channel to receive the events
    let (tx, rx) = channel();

    // Create a watcher object, delivering raw events
    let mut watcher: RecommendedWatcher =
        Watcher::new(tx, notify::Config::default()).with_context(|| "Failed to create watcher!")?;

    // Add the directories to watch
    for directory in directories {
        watcher
            .watch(
                directory.as_path(),
                if recursive {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                },
            )
            .with_context(|| "Failed to add directory to watch!")?;
    }

    // Add a global error counter. If this counter reaches 20 errors within 5 seconds, then the
    // program will exit.
    let mut error_counter = 0;
    let mut timer = std::time::Instant::now();

    loop {
        let event = rx.recv().with_context(|| "Critical error in watcher!")?;

        // Only handle creation events and renames.
        match event {
            Ok(event) if matches!(event.kind, event::EventKind::Create(_)) => {
                // Path should exist, but to be safe, check if it does
                if let Some(path) = event.paths.get(0) {
                    if let Err(e) = handle_path(
                        path,
                        &file_names,
                        &file_extensions,
                        case_sensitive,
                        hide_files,
                        hide_directories,
                        test_mode,
                    ) {
                        eprintln!("{}", e);
                        error_counter += 1;
                    }
                } else {
                    eprintln!("No path in event!");
                    error_counter += 1;
                }
            }
            Ok(event)
            if matches!(
                    event.kind,
                    event::EventKind::Modify(event::ModifyKind::Name(_))
                ) && !matches!(
                    event.kind,
                    event::EventKind::Modify(event::ModifyKind::Name(event::RenameMode::From))
                ) =>
                {
                    // If the length of paths is 2 or more, then the first path is the old name and the
                    // second path is the new name. If the length is 1, then the path is the new name.
                    if let Some(path) = event.paths.get(1) {
                        if let Err(e) = handle_path(
                            path,
                            &file_names,
                            &file_extensions,
                            case_sensitive,
                            hide_files,
                            hide_directories,
                            test_mode,
                        ) {
                            eprintln!("{}", e);
                            error_counter += 1;
                        }
                    } else if let Some(path) = event.paths.get(0) {
                        if let Err(e) = handle_path(
                            path,
                            &file_names,
                            &file_extensions,
                            case_sensitive,
                            hide_files,
                            hide_directories,
                            test_mode,
                        ) {
                            eprintln!("{}", e);
                            error_counter += 1;
                        }
                    } else {
                        eprintln!("No path in event!");
                        error_counter += 1;
                    }
                }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Critical error in watcher: {}", e);
                error_counter += 1;
            }
        }

        // If the error counter is too high, exit the program
        if error_counter >= ERROR_LIMIT && timer.elapsed().as_secs() <= ERROR_TIME_LIMIT {
            return Err(anyhow!(
                "Too many errors in a short period of time. Exiting program."
            ));
        } else if timer.elapsed().as_secs() > 5 {
            error_counter = 0;
            timer = std::time::Instant::now();
        }
    }
}

// Process a path
fn handle_path(
    path: &Path,
    file_names: &HashSet<String>,
    file_extensions: &HashSet<String>,
    case_sensitive: bool,
    hide_files: bool,
    hide_directories: bool,
    test_mode: bool,
) -> Result<()> {
    if should_hide_file(
        path,
        &file_names,
        &file_extensions,
        case_sensitive,
        hide_files,
        hide_directories,
    )? {
        if test_mode {
            println!("Would hide file: {}", path.display());
            Ok(())
        } else {
            hide_file(path)
        }
    } else {
        Ok(())
    }
}

// Windows only function to hide a file
#[cfg(windows)]
fn hide_file(path: &Path) -> Result<()> {
    use std::{
        ffi::OsStr,
        fs::metadata,
        io::Error,
        os::windows::{ffi::OsStrExt, fs::MetadataExt},
    };

    use winapi::{
        shared::minwindef::FALSE,
        um::{fileapi::SetFileAttributesW, winnt::FILE_ATTRIBUTE_HIDDEN},
    };

    // Get the current file attributes
    let attributes = metadata(path)
        .with_context(|| format!("Failed to get file attributes for path {}", path.display()))?
        .file_attributes();

    // Convert the path to a wide string for the Windows API
    let os_path = OsStr::new(path.to_str().with_context(|| {
        format!(
            "Failed to convert path to string for path {}",
            path.display()
        )
    })?)
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();

    // Check if the file is already hidden
    if attributes & FILE_ATTRIBUTE_HIDDEN == FILE_ATTRIBUTE_HIDDEN {
        Ok(())
    } else {
        // Hide the file
        let result =
            unsafe { SetFileAttributesW(os_path.as_ptr(), attributes | FILE_ATTRIBUTE_HIDDEN) };

        // Check if the file was hidden successfully
        if result == FALSE {
            Err::<(), anyhow::Error>(Error::last_os_error().into())
                .with_context(|| format!("Failed to hide path {}", path.display()))
        } else {
            Ok(())
        }
    }
}

// Much simpler function for non-Windows platforms... just adds a dot to the beginning of the file
// name if it doesn't already have one
#[cfg(not(windows))]
fn hide_file(path: &Path) -> Result<()> {
    // Get the file name
    let file_name = path
        .file_name()
        .with_context(|| format!("Failed to get file name from path {}", path.display()))?
        .to_str()
        .with_context(|| {
            format!(
                "Failed to convert file name to string in path {}",
                path.display()
            )
        })?;

    // Check if the file is already hidden
    if file_name.starts_with('.') {
        Ok(())
    } else {
        // Get the parent directory
        let parent = path.parent().with_context(|| {
            format!("Failed to get parent directory of path {}", path.display())
        })?;

        // Get the new file name
        let new_file_name = format!(".{}", file_name);

        // Rename the file
        fs::rename(path, parent.join(new_file_name))
            .with_context(|| format!("Failed to rename path {}", path.display()))?;

        Ok(())
    }
}

// Helper function to build the directory list, file name list, and file extension list
fn setup(
    directories: Vec<String>,
    file_names: Vec<String>,
    file_extensions: Vec<String>,
    case_sensitive: bool,
) -> Result<(HashSet<PathBuf>, HashSet<String>, HashSet<String>)> {
    // Create the set of directories to watch, validating that they exist and are directories. Return
    // an error if they don't exist or aren't directories.
    let directories: HashSet<PathBuf> = directories
        .into_iter()
        .map(|directory| {
            let path = Path::new(&directory).to_path_buf();

            // Check if the path exists and is a directory. Use try_exists instead of exists to
            // catch file system errors.
            if path
                .try_exists()
                .with_context(|| format!("Failed to check if path {} exists!", path.display()))?
            {
                if path.is_dir() {
                    Ok(path)
                } else {
                    Err(anyhow::anyhow!(
                        "Path {} is not a directory!",
                        path.display()
                    ))
                }
            } else {
                Err(anyhow::anyhow!("Path {} does not exist!", path.display()))
            }
        })
        .collect::<Result<HashSet<PathBuf>>>()?;

    // Create the set of file names to hide
    let file_names: HashSet<String> = file_names
        .into_iter()
        .map(|file_name| {
            if case_sensitive {
                file_name.to_lowercase()
            } else {
                file_name
            }
        })
        .collect();

    // Create the set of file extensions to hide. Don't need to add a dot to the beginning of the
    // extension if it doesn't have one because the file name check will take care of that.
    let file_extensions: HashSet<String> = file_extensions
        .into_iter()
        .map(|file_extension| {
            if case_sensitive {
                file_extension.to_lowercase()
            } else {
                file_extension
            }
        })
        .collect();

    Ok((directories, file_names, file_extensions))
}

// Helper function to check if a file or directory should be hidden
fn should_hide_file(
    path: &Path,
    file_names: &HashSet<String>,
    file_extensions: &HashSet<String>,
    case_sensitive: bool,
    hide_files: bool,
    hide_directories: bool,
) -> Result<bool> {
    // If both file names and file extensions are empty, then all files should be hidden
    if file_names.is_empty() && file_extensions.is_empty() {
        return Ok(true);
    }

    // Use fs::metadata instead of is_file and is_dir to catch file system errors
    let metadata = fs::metadata(path)
        .with_context(|| format!("Failed to get metadata for path {}", path.display()))?;

    // Check if the path is a file or directory
    if metadata.is_file() && hide_files {
        // Get the file name
        let file_name = path
            .file_name()
            .with_context(|| format!("Failed to get file name from path {}", path.display()))?
            .to_str()
            .with_context(|| {
                format!(
                    "Failed to convert file name to string in path {}",
                    path.display()
                )
            })?;

        // Check if the file name is in the set of file names to hide
        if file_names.contains(file_name) {
            Ok(true)
        } else {
            // Get the file extension
            let file_extension = path
                .extension()
                .with_context(|| {
                    format!("Failed to get file extension from path {}", path.display())
                })?
                .to_str()
                .with_context(|| {
                    format!(
                        "Failed to convert file extension to string in path {}",
                        path.display()
                    )
                })?;

            // Check if the file extension is in the set of file extensions to hide
            if case_sensitive {
                Ok(file_extensions.contains(file_extension))
            } else {
                Ok(file_extensions.contains(&file_extension.to_lowercase()))
            }
        }
    } else if metadata.is_dir() && hide_directories {
        // Get the directory name
        let directory_name = path
            .file_name()
            .with_context(|| format!("Failed to get directory name from path {}", path.display()))?
            .to_str()
            .with_context(|| {
                format!(
                    "Failed to convert directory name to string in path {}",
                    path.display()
                )
            })?;

        // Check if the directory name is in the set of directory names to hide
        if case_sensitive {
            Ok(file_names.contains(directory_name))
        } else {
            Ok(file_names.contains(&directory_name.to_lowercase()))
        }
    } else {
        Ok(false)
    }
}
