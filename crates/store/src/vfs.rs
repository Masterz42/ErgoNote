//! Cross-platform filesystem façade: direct `std::fs` on native platforms and an
//! in-memory virtual filesystem on `wasm32-unknown-unknown` (where `std::fs` always fails
//! with `Unsupported`).
//!
//! Having notebooks, settings, pages and ink go through this module lets ErgoNote run in a
//! browser (e.g. iPad Safari) without rewriting any of the folder-based store logic.

#[cfg(not(target_arch = "wasm32"))]
pub use std::fs::{
    File, Metadata, OpenOptions, canonicalize, copy, create_dir, create_dir_all, metadata, read, read_dir, read_to_string, remove_dir,
    remove_dir_all, remove_file, rename, symlink_metadata, write,
};

#[cfg(not(target_arch = "wasm32"))]
#[inline]
pub fn is_dir(path: &std::path::Path) -> bool {
    path.is_dir()
}

#[cfg(not(target_arch = "wasm32"))]
#[inline]
pub fn is_file(path: &std::path::Path) -> bool {
    path.is_file()
}

#[cfg(not(target_arch = "wasm32"))]
#[inline]
pub fn exists(path: &std::path::Path) -> bool {
    path.exists()
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::collections::{BTreeMap, BTreeSet};
    use std::ffi::OsString;
    use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
    use std::path::{Component, Path, PathBuf};
    use std::sync::{LazyLock, Mutex};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    struct MemFs {
        dirs: BTreeSet<PathBuf>,
        files: BTreeMap<PathBuf, (Vec<u8>, u64)>,
        seq: u64,
    }

    impl MemFs {
        fn new() -> Self {
            let mut dirs = BTreeSet::new();
            dirs.insert(PathBuf::from("/"));
            Self { dirs, files: BTreeMap::new(), seq: 1 }
        }

        fn next_seq(&mut self) -> u64 {
            self.seq += 1;
            self.seq
        }
    }

    static FS: LazyLock<Mutex<MemFs>> = LazyLock::new(|| Mutex::new(MemFs::new()));

    fn lock_fs() -> std::sync::MutexGuard<'static, MemFs> {
        FS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn normalize(path: &Path) -> PathBuf {
        let mut parts: Vec<String> = Vec::new();
        for comp in path.components() {
            match comp {
                Component::Prefix(_) | Component::RootDir => parts.clear(),
                Component::CurDir => {}
                Component::ParentDir => {
                    parts.pop();
                }
                Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            }
        }
        let mut out = PathBuf::from("/");
        for p in parts {
            out.push(p);
        }
        out
    }

    fn not_found(path: &Path) -> io::Error {
        io::Error::new(io::ErrorKind::NotFound, format!("not found: {}", path.display()))
    }

    fn already_exists(path: &Path) -> io::Error {
        io::Error::new(io::ErrorKind::AlreadyExists, format!("already exists: {}", path.display()))
    }

    #[derive(Clone, Debug)]
    pub struct FileType {
        dir: bool,
    }

    impl FileType {
        pub fn is_dir(&self) -> bool {
            self.dir
        }
        pub fn is_file(&self) -> bool {
            !self.dir
        }
        pub fn is_symlink(&self) -> bool {
            false
        }
    }

    #[derive(Clone, Debug)]
    pub struct Metadata {
        dir: bool,
        len: u64,
        seq: u64,
    }

    impl Metadata {
        pub fn is_dir(&self) -> bool {
            self.dir
        }
        pub fn is_file(&self) -> bool {
            !self.dir
        }
        pub fn len(&self) -> u64 {
            self.len
        }
        pub fn is_empty(&self) -> bool {
            self.len == 0
        }
        pub fn file_type(&self) -> FileType {
            FileType { dir: self.dir }
        }
        pub fn modified(&self) -> io::Result<SystemTime> {
            Ok(UNIX_EPOCH + Duration::from_secs(self.seq))
        }
    }

    #[derive(Clone, Debug)]
    pub struct DirEntry {
        path: PathBuf,
        dir: bool,
        len: u64,
        seq: u64,
    }

    impl DirEntry {
        pub fn path(&self) -> PathBuf {
            self.path.clone()
        }
        pub fn file_name(&self) -> OsString {
            self.path.file_name().map(OsString::from).unwrap_or_default()
        }
        pub fn file_type(&self) -> io::Result<FileType> {
            Ok(FileType { dir: self.dir })
        }
        pub fn metadata(&self) -> io::Result<Metadata> {
            Ok(Metadata { dir: self.dir, len: self.len, seq: self.seq })
        }
    }

    pub struct ReadDir {
        inner: std::vec::IntoIter<io::Result<DirEntry>>,
    }

    impl Iterator for ReadDir {
        type Item = io::Result<DirEntry>;
        fn next(&mut self) -> Option<Self::Item> {
            self.inner.next()
        }
    }

    pub struct File {
        path: PathBuf,
        cursor: Cursor<Vec<u8>>,
        writable: bool,
    }

    impl File {
        pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
            OpenOptions::new().read(true).open(path)
        }

        pub fn create<P: AsRef<Path>>(path: P) -> io::Result<Self> {
            OpenOptions::new().write(true).create(true).truncate(true).open(path)
        }

        fn sync_to_fs(&self) -> io::Result<()> {
            if !self.writable {
                return Ok(());
            }
            let mut fs = lock_fs();
            let seq = fs.next_seq();
            fs.files.insert(self.path.clone(), (self.cursor.get_ref().clone(), seq));
            Ok(())
        }
    }

    impl Read for File {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.cursor.read(buf)
        }
    }

    impl Write for File {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if !self.writable {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "file not opened for writing"));
            }
            let n = self.cursor.write(buf)?;
            self.sync_to_fs()?;
            Ok(n)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.sync_to_fs()
        }
    }

    impl Seek for File {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.cursor.seek(pos)
        }
    }

    impl Drop for File {
        fn drop(&mut self) {
            if self.writable {
                let _ = self.sync_to_fs();
            }
        }
    }

    #[derive(Clone, Debug, Default)]
    pub struct OpenOptions {
        read: bool,
        write: bool,
        create: bool,
        create_new: bool,
        truncate: bool,
        append: bool,
    }

    impl OpenOptions {
        pub fn new() -> Self {
            Self::default()
        }
        pub fn read(&mut self, read: bool) -> &mut Self {
            self.read = read;
            self
        }
        pub fn write(&mut self, write: bool) -> &mut Self {
            self.write = write;
            self
        }
        pub fn create(&mut self, create: bool) -> &mut Self {
            self.create = create;
            self
        }
        pub fn create_new(&mut self, create_new: bool) -> &mut Self {
            self.create_new = create_new;
            self
        }
        pub fn truncate(&mut self, truncate: bool) -> &mut Self {
            self.truncate = truncate;
            self
        }
        pub fn append(&mut self, append: bool) -> &mut Self {
            self.append = append;
            self
        }

        pub fn open<P: AsRef<Path>>(&self, path: P) -> io::Result<File> {
            let norm = normalize(path.as_ref());
            let mut fs = lock_fs();
            if fs.dirs.contains(&norm) {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "is a directory"));
            }
            let existing = fs.files.get(&norm).map(|(b, _)| b.clone());
            if self.create_new {
                if existing.is_some() {
                    return Err(already_exists(&norm));
                }
                if let Some(parent) = norm.parent()
                    && !fs.dirs.contains(parent)
                {
                    return Err(not_found(parent));
                }
                let seq = fs.next_seq();
                fs.files.insert(norm.clone(), (Vec::new(), seq));
                return Ok(File { path: norm, cursor: Cursor::new(Vec::new()), writable: true });
            }
            if let Some(mut data) = existing {
                if self.truncate && self.write {
                    data.clear();
                    let seq = fs.next_seq();
                    fs.files.insert(norm.clone(), (Vec::new(), seq));
                }
                let pos = if self.append { data.len() as u64 } else { 0 };
                let mut cursor = Cursor::new(data);
                cursor.set_position(pos);
                return Ok(File { path: norm, cursor, writable: self.write || self.append });
            }
            if (self.create || self.append) && self.write {
                if let Some(parent) = norm.parent()
                    && !fs.dirs.contains(parent)
                {
                    return Err(not_found(parent));
                }
                let seq = fs.next_seq();
                fs.files.insert(norm.clone(), (Vec::new(), seq));
                return Ok(File { path: norm, cursor: Cursor::new(Vec::new()), writable: true });
            }
            Err(not_found(&norm))
        }
    }

    pub fn canonicalize<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
        let norm = normalize(path.as_ref());
        let fs = lock_fs();
        if fs.dirs.contains(&norm) || fs.files.contains_key(&norm) { Ok(norm) } else { Err(not_found(&norm)) }
    }

    pub fn create_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
        let norm = normalize(path.as_ref());
        let mut fs = lock_fs();
        if fs.dirs.contains(&norm) || fs.files.contains_key(&norm) {
            return Err(already_exists(&norm));
        }
        if let Some(parent) = norm.parent()
            && !fs.dirs.contains(parent)
        {
            return Err(not_found(parent));
        }
        fs.dirs.insert(norm);
        Ok(())
    }

    pub fn create_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
        let norm = normalize(path.as_ref());
        let mut fs = lock_fs();
        let mut cur = PathBuf::from("/");
        for comp in norm.components() {
            if let Component::Normal(part) = comp {
                cur.push(part);
                if fs.files.contains_key(&cur) {
                    return Err(already_exists(&cur));
                }
                fs.dirs.insert(cur.clone());
            }
        }
        Ok(())
    }

    pub fn metadata<P: AsRef<Path>>(path: P) -> io::Result<Metadata> {
        let norm = normalize(path.as_ref());
        let fs = lock_fs();
        if fs.dirs.contains(&norm) {
            return Ok(Metadata { dir: true, len: 0, seq: 1 });
        }
        if let Some((bytes, seq)) = fs.files.get(&norm) {
            return Ok(Metadata { dir: false, len: bytes.len() as u64, seq: *seq });
        }
        Err(not_found(&norm))
    }

    pub fn symlink_metadata<P: AsRef<Path>>(path: P) -> io::Result<Metadata> {
        metadata(path)
    }

    pub fn read<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
        let norm = normalize(path.as_ref());
        let fs = lock_fs();
        fs.files.get(&norm).map(|(b, _)| b.clone()).ok_or_else(|| not_found(&norm))
    }

    pub fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
        let bytes = read(path)?;
        String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> io::Result<()> {
        let norm = normalize(path.as_ref());
        let mut fs = lock_fs();
        if fs.dirs.contains(&norm) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "is a directory"));
        }
        if let Some(parent) = norm.parent()
            && !fs.dirs.contains(parent)
        {
            return Err(not_found(parent));
        }
        let seq = fs.next_seq();
        fs.files.insert(norm, (contents.as_ref().to_vec(), seq));
        Ok(())
    }

    pub fn copy<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<u64> {
        let bytes = read(from)?;
        let len = bytes.len() as u64;
        write(to, bytes)?;
        Ok(len)
    }

    pub fn read_dir<P: AsRef<Path>>(path: P) -> io::Result<ReadDir> {
        let norm = normalize(path.as_ref());
        let fs = lock_fs();
        if !fs.dirs.contains(&norm) {
            return Err(not_found(&norm));
        }
        let mut out = Vec::new();
        for dir in &fs.dirs {
            if dir != &norm && dir.parent() == Some(norm.as_path()) {
                out.push(Ok(DirEntry { path: dir.clone(), dir: true, len: 0, seq: 1 }));
            }
        }
        for (file, (bytes, seq)) in &fs.files {
            if file.parent() == Some(norm.as_path()) {
                out.push(Ok(DirEntry { path: file.clone(), dir: false, len: bytes.len() as u64, seq: *seq }));
            }
        }
        Ok(ReadDir { inner: out.into_iter() })
    }

    pub fn remove_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
        let norm = normalize(path.as_ref());
        let mut fs = lock_fs();
        if fs.files.remove(&norm).is_some() { Ok(()) } else { Err(not_found(&norm)) }
    }

    pub fn remove_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
        let norm = normalize(path.as_ref());
        let mut fs = lock_fs();
        if !fs.dirs.contains(&norm) {
            return Err(not_found(&norm));
        }
        let has_children =
            fs.dirs.iter().any(|d| d != &norm && d.parent() == Some(norm.as_path())) || fs.files.keys().any(|f| f.parent() == Some(norm.as_path()));
        if has_children {
            return Err(io::Error::new(io::ErrorKind::DirectoryNotEmpty, "directory not empty"));
        }
        fs.dirs.remove(&norm);
        Ok(())
    }

    pub fn remove_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
        let norm = normalize(path.as_ref());
        let mut fs = lock_fs();
        if !fs.dirs.contains(&norm) {
            return Err(not_found(&norm));
        }
        fs.dirs.retain(|d| d != &norm && !d.starts_with(&norm));
        fs.files.retain(|f, _| !f.starts_with(&norm));
        Ok(())
    }

    pub fn rename<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<()> {
        let src = normalize(from.as_ref());
        let dst = normalize(to.as_ref());
        let mut fs = lock_fs();
        if let Some((bytes, _)) = fs.files.remove(&src) {
            if let Some(parent) = dst.parent()
                && !fs.dirs.contains(parent)
            {
                fs.files.insert(src, (bytes, 1));
                return Err(not_found(parent));
            }
            let seq = fs.next_seq();
            fs.files.insert(dst, (bytes, seq));
            return Ok(());
        }
        if fs.dirs.contains(&src) {
            if let Some(parent) = dst.parent()
                && !fs.dirs.contains(parent)
            {
                return Err(not_found(parent));
            }
            let moved_dirs: Vec<PathBuf> = fs.dirs.iter().filter(|d| d.starts_with(&src)).cloned().collect();
            for d in &moved_dirs {
                fs.dirs.remove(d);
            }
            for d in moved_dirs {
                let rel = d.strip_prefix(&src).unwrap_or_else(|_| Path::new(""));
                fs.dirs.insert(if rel.as_os_str().is_empty() { dst.clone() } else { dst.join(rel) });
            }
            let moved_files: Vec<(PathBuf, (Vec<u8>, u64))> =
                fs.files.iter().filter(|(f, _)| f.starts_with(&src)).map(|(k, v)| (k.clone(), v.clone())).collect();
            for (f, _) in &moved_files {
                fs.files.remove(f);
            }
            for (f, val) in moved_files {
                let rel = f.strip_prefix(&src).unwrap_or_else(|_| Path::new(""));
                fs.files.insert(dst.join(rel), val);
            }
            return Ok(());
        }
        Err(not_found(&src))
    }

    pub fn is_dir(path: &Path) -> bool {
        let norm = normalize(path);
        lock_fs().dirs.contains(&norm)
    }

    pub fn is_file(path: &Path) -> bool {
        let norm = normalize(path);
        lock_fs().files.contains_key(&norm)
    }

    pub fn exists(path: &Path) -> bool {
        let norm = normalize(path);
        let fs = lock_fs();
        fs.dirs.contains(&norm) || fs.files.contains_key(&norm)
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::{
    DirEntry, File, FileType, Metadata, OpenOptions, ReadDir, canonicalize, copy, create_dir, create_dir_all, exists, is_dir, is_file, metadata,
    read, read_dir, read_to_string, remove_dir, remove_dir_all, remove_file, rename, symlink_metadata, write,
};
