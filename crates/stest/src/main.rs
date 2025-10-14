use std::{
	cmp::{Ordering, PartialOrd},
	ffi::OsStr,
	fs::{FileType, Metadata},
	io,
	os::unix::fs::{FileTypeExt, MetadataExt},
	path::Path,
	process::exit,
};

use getopts::{Matches, Options};
use walkdir::WalkDir;

static mut MATCH: u8 = 0;

#[derive(Clone)]
struct File {
	path: Box<Path>,
}

impl File {
	pub fn new(path: Box<Path>) -> Self {
		File { path }
	}

	fn meta(&self) -> io::Result<Metadata> {
		self.path.metadata()
	}

	fn file_type(&self) -> io::Result<FileType> {
		self.meta().map(|meta| meta.file_type())
	}

	fn mode(&self) -> io::Result<u32> {
		self.meta().map(|meta| meta.mode())
	}

	fn is_hidden(&self) -> bool {
		self.path
			.file_name()
			.is_some_and(|name| name.to_string_lossy().starts_with("."))
	}

	fn is_block(&self) -> bool {
		self.file_type().as_ref().is_ok_and(FileTypeExt::is_block_device)
	}

	fn is_char(&self) -> bool {
		self.file_type().as_ref().is_ok_and(FileTypeExt::is_char_device)
	}

	fn is_dir(&self) -> bool {
		self.path.is_dir()
	}

	fn exists(&self) -> bool {
		self.path.try_exists().is_ok_and(|x| x)
	}

	fn is_file(&self) -> bool {
		self.path.is_file()
	}

	fn has_setgid(&self) -> bool {
		self.mode().is_ok_and(|mode| mode & 0o2000 != 0)
	}

	fn is_symlink(&self) -> bool {
		self.path.is_symlink()
	}

	fn is_pipe(&self) -> bool {
		self.file_type().as_ref().is_ok_and(FileTypeExt::is_fifo)
	}

	fn is_readable(&self) -> bool {
		self.mode().is_ok_and(|mode| mode & 0o0444 != 0)
	}

	fn has_setuid(&self) -> bool {
		self.mode().is_ok_and(|mode| mode & 0o4000 != 0)
	}

	fn is_non_empty(&self) -> bool {
		self.meta().is_ok_and(|meta| meta.len() > 0)
	}

	fn is_writable(&self) -> bool {
		self.mode().is_ok_and(|mode| mode & 0o0222 != 0)
	}

	fn is_executable(&self) -> bool {
		self.mode().is_ok_and(|mode| mode & 0o0111 != 0)
	}
}

impl<T: AsRef<OsStr>> From<T> for File {
	fn from(value: T) -> Self {
		File::new(Box::from(Path::new(&value)))
	}
}

impl PartialEq for File {
	fn eq(&self, other: &Self) -> bool {
		self.partial_cmp(other) == Some(Ordering::Equal)
	}
}

impl PartialOrd for File {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		self.meta().ok().and_then(|meta| {
			other.meta().ok().and_then(|other_meta| {
				meta.mtime().partial_cmp(&other_meta.mtime())
			})
		})
	}
}

fn test(file: &File, flags: &Matches, new: Option<&File>, old: Option<&File>) {
	if ((!flags.opt_present("a") || file.is_hidden())                  // hidden files
		&& (!flags.opt_present("b") || file.is_block())                // block special
		&& (!flags.opt_present("c") || file.is_char())                 // character special
		&& (!flags.opt_present("d") || file.is_dir())                  // directory
		&& (!flags.opt_present("e") || file.exists())                  // exists
		&& (!flags.opt_present("f") || file.is_file())                 // regular file
		&& (!flags.opt_present("g") || file.has_setgid())	           // set-group-id flag
		&& (!flags.opt_present("h") || file.is_symlink())	           // symbolic link
		&& (!flags.opt_present("n") || new.is_some_and(|n| file > n)) // newer than file
		&& (!flags.opt_present("o") || old.is_some_and(|o| file < o)) // older than file
		&& (!flags.opt_present("p") || file.is_pipe())                 // named pipe
		&& (!flags.opt_present("r") || file.is_readable())	           // readable
		&& (!flags.opt_present("s") || file.is_non_empty())            // not empty
		&& (!flags.opt_present("u") || file.has_setuid())	           // set-user-id flag
		&& (!flags.opt_present("w") || file.is_writable())	           // writable
		&& (!flags.opt_present("x") || file.is_executable()))          // executable
		!= flags.opt_present("v")
	{
		if flags.opt_present("q") {
			exit(0)
		}
		unsafe {
			MATCH = 1;
		}
		println!("{}", file.path.to_string_lossy());
	}
}

fn usage(program: &str, opts: Options) {
	let brief = format!(
		"usage: {} [-abcdefghlpqrsuvwx] [-n file] [-o file] [file...]",
		program
	);
	print!("{}", opts.usage(&brief));
}

fn main() {
	let args: Vec<_> = std::env::args().collect();
	let program = &args[0];

	let mut opts = Options::new();
	opts.optflag("a", "hidden", "hidden");
	opts.optflag("b", "block", "block device");
	opts.optflag("c", "char", "char device");
	opts.optflag("d", "dir", "directory");
	opts.optflag("e", "exists", "exists");
	opts.optflag("f", "file", "file");
	opts.optflag("g", "has-setgid", "setgid");
	opts.optflag("h", "symlink", "symlink");
	opts.optflag("l", "recurse", "test directory contents");
	opts.optflagopt("n", "newer", "newer", "file");
	opts.optflagopt("o", "older", "older", "file");
	opts.optflag("p", "pipe", "pipe");
	opts.optflag("q", "quiet", "quiet");
	opts.optflag("r", "readable", "readable");
	opts.optflag("s", "non-empty", "non-empty");
	opts.optflag("u", "has-setuid", "setuid");
	opts.optflag("v", "inverted", "invert");
	opts.optflag("w", "writable", "writable");
	opts.optflag("x", "executable", "executable");

	let matches = match opts.parse(std::env::args()) {
		Ok(m) => m,
		_ => {
			usage(program, opts);
			std::process::exit(2);
		}
	};

	let newer = matches.opt_str("n").map(File::from);
	let older = matches.opt_str("o").map(File::from);
	let mut paths =
		matches.free.iter().skip(1).map(File::from).collect::<Vec<_>>();

	if paths.is_empty() {
		let mut line = String::with_capacity(128);
		let stdin = io::stdin();
		while let Ok(len) = stdin.read_line(&mut line) {
			if len == 0 || line == "\n" {
				break;
			}
			paths.push(File::from(line.trim()));
			line.clear();
		}
	}

	for path in paths {
		if matches.opt_present("l") && path.is_dir() {
			WalkDir::new(path.path)
				.into_iter()
				.filter_map(|e| e.ok())
				.map(|entry| File::from(entry.path()))
				.for_each(|file| {
					test(&file, &matches, newer.as_ref(), older.as_ref())
				});
		} else {
			test(&path, &matches, newer.as_ref(), older.as_ref());
		}
	}

	std::process::exit((unsafe { MATCH } == 0) as i32)
}
