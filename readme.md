# Filehider

A simple tool written in rust to hide files in a directory, or multiple directories, by marking them as hidden on windows, and by prepending a dot to the filename on linux. 

By default it will hide all files it can find, but you can specify a list of file names to hide and/or a list of file extensions to hide.

Can run in immediate mode, where it walks the tree and hides all files and directories that match the given file names and extensions, or in watch mode, where it watches for changes to the files and directories and automatically hides them. Or it can run both, starting in immediate mode and then switching to watch mode.

## Usage

```
Usage: filehider.exe [OPTIONS] <DIRECTORIES>...

Arguments:
  <DIRECTORIES>...  The directories to watch
                    (e.g. "C:\Users\user\Documents" or "test/test")

Options:
  -n, --file-names <FILE_NAMES>...
          The file names to automatically hide
          (e.g. "file.txt" or "file")
  -x, --file-extensions <FILE_EXTENSIONS>...
          The file extensions to automatically hide
          (e.g. "txt" or ".txt")
  -r, --recursive
          Switch to enable recursive watching
          (i.e. watch all subdirectories)
          [default: false]
  -c, --case-sensitive
          Switch to enable case sensitivity in file names and extensions
          (e.g. "file.txt" and "FILE.TXT" are the same)
          [default: false]
      --test
          Switch to enable test mode. In test mode, the program will not actually hide files
          and will instead print the paths of the files that would be hidden.
          [default: false]
  -w, --watch
          Switch to enable watch mode, which will watch for changes to the files and directories
          and automatically hide them.
          [default: false]
  -i, --immediate
          Switch to enable immediate mode, which will immediately hide all files and directories
          that match the given file names and extensions.
          [default: true]
  -t, --file-types <FILE_TYPES>...
          Types of files to hide [default: "file directory"] [possible values: file, directory]
  -h, --help
          Print help
  -V, --version
          Print version
```