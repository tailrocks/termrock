#[cfg(test)]
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use crate::scenario::EPOCH_SECS;

pub const BLOCK: u64 = 4096;

pub const PREVIEW_MAX_BYTES: usize = 256 * 1024;
pub const PREVIEW_MAX_LINES: usize = 2000;
pub const PREVIEW_MAX_LINE_CHARS: usize = 4096;

pub const COUNT_CAP: usize = 2048;

pub const FIND_MAX_RESULTS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    File,
    Dir,
    Symlink { target: String },
    Fifo,
    Device,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    None,
    Text(String),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub path: String,
    pub kind: NodeKind,
    pub apparent: u64,
    pub allocated: u64,
    pub mtime: i64,
    pub readable: bool,

    pub inode: u64,

    pub dataless: bool,
    pub content: Content,
}

impl Node {
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
    pub fn hidden(&self) -> bool {
        self.name().starts_with('.')
    }
    pub fn is_dir(&self) -> bool {
        self.kind == NodeKind::Dir
    }
    pub fn is_file(&self) -> bool {
        self.kind == NodeKind::File
    }
    pub fn kind_word(&self) -> &'static str {
        match self.kind {
            NodeKind::File => "file",
            NodeKind::Dir => "directory",
            NodeKind::Symlink { .. } => "symlink",
            NodeKind::Fifo => "fifo",
            NodeKind::Device => "device",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsError {
    NotFound(String),
    PermissionDenied(String),
    NotADirectory(String),

    SpecialFile(String),

    BrokenLink(String),
    Dataless(String),
}

impl FsError {
    pub fn message(&self) -> String {
        match self {
            FsError::NotFound(p) => format!("{p}: no such file or directory"),
            FsError::PermissionDenied(p) => format!("{p}: permission denied"),
            FsError::NotADirectory(p) => format!("{p}: not a directory"),
            FsError::SpecialFile(p) => format!("{p}: special file · not read"),
            FsError::BrokenLink(p) => format!("{p}: broken symbolic link"),
            FsError::Dataless(p) => format!("{p}: dataless · not materialised"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trashed {
    pub original: String,
    pub trash_path: String,
    pub bytes: u64,
    pub at_secs: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    pub mount: String,
    pub total: u64,
    pub used: u64,

    pub purgeable: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fs {
    nodes: BTreeMap<String, Node>,
    next_inode: u64,
    pub trash: Vec<Trashed>,
    pub volumes: Vec<Volume>,

    pub trash_broken: Option<String>,

    pub trash_collisions: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanNode {
    pub path: String,
    pub is_dir: bool,
    pub allocated: u64,
    pub apparent: u64,

    pub entries: u64,
    pub error: Option<FsError>,
    pub children: Vec<ScanNode>,

    pub link: bool,
}

impl ScanNode {
    pub fn errors(&self) -> usize {
        usize::from(self.error.is_some())
            + self.children.iter().map(ScanNode::errors).sum::<usize>()
    }
    pub fn find(&self, path: &str) -> Option<&ScanNode> {
        if self.path == path {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(path))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScanOptions {
    pub include_hidden: bool,

    pub skip: Vec<String>,
    pub max_depth: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Preview {
    Text {
        lines: Vec<String>,
        bytes_shown: usize,
        total_bytes: u64,
        truncated_bytes: bool,
        truncated_lines: bool,
        long_lines: usize,

        link: Option<String>,
    },
    Empty,
    Binary {
        reason: String,
    },
    Error(FsError),
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirCount {
    pub visible: usize,
    pub hidden: usize,

    pub capped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindHit {
    pub path: String,
    pub is_dir: bool,

    pub matched: Vec<usize>,
    pub score: u32,
}

impl Fs {
    pub fn new() -> Self {
        Self::default()
    }

    fn alloc(&mut self) -> u64 {
        self.next_inode += 1;
        self.next_inode
    }

    fn ensure_parents(&mut self, path: &str) {
        let mut cur = String::new();
        for seg in path.trim_start_matches('/').split('/') {
            if seg.is_empty() {
                continue;
            }
            let next = format!("{cur}/{seg}");
            if next != path && !self.nodes.contains_key(&next) {
                let inode = self.alloc();
                self.nodes.insert(
                    next.clone(),
                    Node {
                        path: next.clone(),
                        kind: NodeKind::Dir,
                        apparent: 0,
                        allocated: BLOCK,
                        mtime: EPOCH_SECS - 30 * 86_400,
                        readable: true,
                        inode,
                        dataless: false,
                        content: Content::None,
                    },
                );
            }
            cur = next;
        }
        if !self.nodes.contains_key("/") {
            let inode = self.alloc();
            self.nodes.insert(
                "/".into(),
                Node {
                    path: "/".into(),
                    kind: NodeKind::Dir,
                    apparent: 0,
                    allocated: BLOCK,
                    mtime: EPOCH_SECS - 365 * 86_400,
                    readable: true,
                    inode,
                    dataless: false,
                    content: Content::None,
                },
            );
        }
    }

    fn put(&mut self, mut node: Node) {
        self.ensure_parents(&node.path);
        if node.inode == 0 {
            node.inode = self.alloc();
        }
        self.nodes.insert(node.path.clone(), node);
    }

    pub fn dir(&mut self, path: &str, age_days: i64) -> &mut Self {
        self.put(Node {
            path: path.into(),
            kind: NodeKind::Dir,
            apparent: 0,
            allocated: BLOCK,
            mtime: EPOCH_SECS - age_days * 86_400,
            readable: true,
            inode: 0,
            dataless: false,
            content: Content::None,
        });
        self
    }

    pub fn file(&mut self, path: &str, bytes: u64, age_days: i64) -> &mut Self {
        self.put(Node {
            path: path.into(),
            kind: NodeKind::File,
            apparent: bytes,
            allocated: bytes.div_ceil(BLOCK) * BLOCK,
            mtime: EPOCH_SECS - age_days * 86_400,
            readable: true,
            inode: 0,
            dataless: false,
            content: Content::None,
        });
        self
    }

    pub fn sparse(
        &mut self,
        path: &str,
        apparent: u64,
        allocated: u64,
        age_days: i64,
    ) -> &mut Self {
        self.file(path, apparent, age_days);
        if let Some(n) = self.nodes.get_mut(path) {
            n.allocated = allocated;
        }
        self
    }

    pub fn text(&mut self, path: &str, text: &str, age_days: i64) -> &mut Self {
        self.file(path, text.len() as u64, age_days);
        if let Some(n) = self.nodes.get_mut(path) {
            n.content = Content::Text(text.into());
        }
        self
    }

    pub fn bytes(&mut self, path: &str, bytes: Vec<u8>, age_days: i64) -> &mut Self {
        self.file(path, bytes.len() as u64, age_days);
        if let Some(n) = self.nodes.get_mut(path) {
            n.content = Content::Bytes(bytes);
        }
        self
    }

    pub fn symlink(&mut self, path: &str, target: &str) -> &mut Self {
        self.put(Node {
            path: path.into(),
            kind: NodeKind::Symlink {
                target: target.into(),
            },

            apparent: 0,
            allocated: 0,
            mtime: EPOCH_SECS - 86_400,
            readable: true,
            inode: 0,
            dataless: false,
            content: Content::None,
        });
        self
    }

    pub fn fifo(&mut self, path: &str) -> &mut Self {
        self.put(Node {
            path: path.into(),
            kind: NodeKind::Fifo,
            apparent: 0,
            allocated: 0,
            mtime: EPOCH_SECS,
            readable: true,
            inode: 0,
            dataless: false,
            content: Content::None,
        });
        self
    }

    pub fn device(&mut self, path: &str) -> &mut Self {
        self.put(Node {
            path: path.into(),
            kind: NodeKind::Device,
            apparent: 0,
            allocated: 0,
            mtime: EPOCH_SECS,
            readable: true,
            inode: 0,
            dataless: false,
            content: Content::None,
        });
        self
    }

    pub fn hardlink(&mut self, path: &str, of: &str) -> &mut Self {
        if let Some(src) = self.nodes.get(of).cloned() {
            let mut n = src;
            n.path = path.into();
            self.ensure_parents(path);
            self.nodes.insert(path.into(), n);
        }
        self
    }

    pub fn unreadable(&mut self, path: &str) -> &mut Self {
        if let Some(n) = self.nodes.get_mut(path) {
            n.readable = false;
        }
        self
    }

    pub fn dataless(&mut self, path: &str) -> &mut Self {
        if let Some(n) = self.nodes.get_mut(path) {
            n.dataless = true;
        }
        self
    }

    pub fn volume(&mut self, mount: &str, total: u64, used: u64) -> &mut Self {
        self.volumes.push(Volume {
            mount: mount.into(),
            total,
            used,
            purgeable: 0,
        });
        self
    }

    pub fn touch(&mut self, path: &str, age_days: i64) {
        if let Some(n) = self.nodes.get_mut(path) {
            n.mtime = EPOCH_SECS - age_days * 86_400;
        }
    }

    pub fn get(&self, path: &str) -> Option<&Node> {
        self.nodes.get(path)
    }

    pub fn exists(&self, path: &str) -> bool {
        self.nodes.contains_key(path)
    }

    pub fn is_dir(&self, path: &str) -> bool {
        self.nodes.get(path).is_some_and(Node::is_dir)
    }

    pub fn is_file(&self, path: &str) -> bool {
        self.nodes.get(path).is_some_and(Node::is_file)
    }

    pub fn parent(path: &str) -> Option<String> {
        if path == "/" {
            return None;
        }
        match path.rsplit_once('/') {
            Some(("", _)) => Some("/".into()),
            Some((p, _)) => Some(p.to_owned()),
            None => None,
        }
    }

    pub fn join(dir: &str, name: &str) -> String {
        if dir == "/" {
            format!("/{name}")
        } else {
            format!("{dir}/{name}")
        }
    }

    pub fn list(&self, dir: &str) -> Result<Vec<&Node>, FsError> {
        let d = self
            .nodes
            .get(dir)
            .ok_or_else(|| FsError::NotFound(dir.into()))?;
        if !d.readable {
            return Err(FsError::PermissionDenied(dir.into()));
        }
        if !d.is_dir() {
            return Err(FsError::NotADirectory(dir.into()));
        }
        let prefix = if dir == "/" {
            "/".to_owned()
        } else {
            format!("{dir}/")
        };
        let mut v: Vec<&Node> = self
            .nodes
            .range(prefix.clone()..)
            .take_while(|(p, _)| p.starts_with(&prefix))
            .filter(|(p, _)| !p[prefix.len()..].contains('/'))
            .map(|(_, n)| n)
            .collect();
        v.sort_by(|a, b| {
            b.is_dir()
                .cmp(&a.is_dir())
                .then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
                .then_with(|| a.name().cmp(b.name()))
        });
        Ok(v)
    }

    pub fn subtree(&self, root: &str) -> Vec<&Node> {
        let prefix = if root == "/" {
            "/".to_owned()
        } else {
            format!("{root}/")
        };
        self.nodes
            .range(prefix.clone()..)
            .take_while(|(p, _)| p.starts_with(&prefix))
            .map(|(_, n)| n)
            .collect()
    }

    pub fn count(&self, dir: &str) -> Result<DirCount, FsError> {
        let all = self.list(dir)?;
        let mut c = DirCount {
            visible: 0,
            hidden: 0,
            capped: false,
        };
        for n in all {
            if c.visible + c.hidden >= COUNT_CAP {
                c.capped = true;
                break;
            }
            if n.hidden() {
                c.hidden += 1;
            } else {
                c.visible += 1;
            }
        }
        Ok(c)
    }

    pub fn canonical_parent(&self, path: &str) -> Result<(String, Vec<String>), FsError> {
        let Some(parent) = Self::parent(path) else {
            return Ok(("/".into(), vec![]));
        };
        let mut resolved = String::new();
        let mut links = vec![];
        for seg in parent.trim_start_matches('/').split('/') {
            if seg.is_empty() {
                continue;
            }
            let next = format!("{resolved}/{seg}");
            match self.nodes.get(&next) {
                Some(Node {
                    kind: NodeKind::Symlink { target },
                    ..
                }) => {
                    links.push(next.clone());
                    if !self.nodes.contains_key(target) {
                        return Err(FsError::BrokenLink(next));
                    }
                    resolved = target.clone();
                }
                Some(_) => resolved = next,
                None => return Err(FsError::NotFound(next)),
            }
        }
        if resolved.is_empty() {
            resolved = "/".into();
        }
        Ok((resolved, links))
    }

    pub fn resolve(&self, path: &str) -> Result<&Node, FsError> {
        let mut cur = path.to_owned();
        for _ in 0..16 {
            match self.nodes.get(&cur) {
                None => return Err(FsError::BrokenLink(path.into())),
                Some(Node {
                    kind: NodeKind::Symlink { target },
                    ..
                }) => cur = target.clone(),
                Some(n) => return Ok(n),
            }
        }
        Err(FsError::BrokenLink(path.into()))
    }

    pub fn preview(&self, path: &str) -> Preview {
        let mut link = None;
        let node = match self.nodes.get(path) {
            None => return Preview::Error(FsError::NotFound(path.into())),
            Some(Node {
                kind: NodeKind::Symlink { target },
                ..
            }) => {
                link = Some(target.clone());
                match self.resolve(path) {
                    Ok(n) => n,
                    Err(e) => return Preview::Error(e),
                }
            }
            Some(n) => n,
        };

        match node.kind {
            NodeKind::Fifo | NodeKind::Device => {
                return Preview::Error(FsError::SpecialFile(path.into()));
            }
            NodeKind::Dir => return Preview::Directory,
            _ => {}
        }
        if !node.readable {
            return Preview::Error(FsError::PermissionDenied(path.into()));
        }
        if node.dataless {
            return Preview::Error(FsError::Dataless(path.into()));
        }
        let raw: Vec<u8> = match &node.content {
            Content::Text(t) => t.as_bytes().to_vec(),
            Content::Bytes(b) => b.clone(),
            Content::None => {
                if node.apparent == 0 {
                    return Preview::Empty;
                }
                return Preview::Binary {
                    reason: "content not modelled · treated as binary".into(),
                };
            }
        };
        if raw.is_empty() {
            return Preview::Empty;
        }
        let total = raw.len() as u64;
        let truncated_bytes = raw.len() > PREVIEW_MAX_BYTES;
        let mut cut = raw.len().min(PREVIEW_MAX_BYTES);

        while cut < raw.len() && cut > 0 && (raw[cut] & 0b1100_0000) == 0b1000_0000 {
            cut -= 1;
        }
        let head = &raw[..cut];
        if head.contains(&0) {
            return Preview::Binary {
                reason: "NUL byte in the first 256 KiB".into(),
            };
        }
        let text = match std::str::from_utf8(head) {
            Ok(t) => t,
            Err(_) => {
                return Preview::Binary {
                    reason: "invalid UTF-8".into(),
                };
            }
        };
        let mut lines = vec![];
        let mut long_lines = 0;
        let mut truncated_lines = false;
        for l in text.split('\n') {
            if lines.len() >= PREVIEW_MAX_LINES {
                truncated_lines = true;
                break;
            }
            let mut s = sanitize(l);
            if s.chars().count() > PREVIEW_MAX_LINE_CHARS {
                s = s.chars().take(PREVIEW_MAX_LINE_CHARS).collect::<String>() + " …";
                long_lines += 1;
            }
            lines.push(s);
        }
        if lines.last().is_some_and(|l| l.is_empty()) && text.ends_with('\n') {
            lines.pop();
        }
        Preview::Text {
            lines,
            bytes_shown: cut,
            total_bytes: total,
            truncated_bytes,
            truncated_lines,
            long_lines,
            link,
        }
    }

    pub fn scan(&self, root: &str, opts: &ScanOptions) -> ScanNode {
        let mut seen = BTreeSet::new();
        self.scan_node(root, opts, 0, &mut seen)
    }

    fn scan_node(
        &self,
        path: &str,
        opts: &ScanOptions,
        depth: usize,
        seen: &mut BTreeSet<u64>,
    ) -> ScanNode {
        let Some(node) = self.nodes.get(path) else {
            return ScanNode {
                path: path.into(),
                is_dir: false,
                allocated: 0,
                apparent: 0,
                entries: 0,
                error: Some(FsError::NotFound(path.into())),
                children: vec![],
                link: false,
            };
        };
        match &node.kind {
            NodeKind::Symlink { .. } => {
                return ScanNode {
                    path: path.into(),
                    is_dir: false,
                    allocated: node.allocated,
                    apparent: node.apparent,
                    entries: 1,
                    error: None,
                    children: vec![],
                    link: true,
                };
            }
            NodeKind::Dir => {}
            _ => {
                let dup = !seen.insert(node.inode);
                return ScanNode {
                    path: path.into(),
                    is_dir: false,
                    allocated: if dup { 0 } else { node.allocated },
                    apparent: if dup { 0 } else { node.apparent },
                    entries: 1,
                    error: None,
                    children: vec![],
                    link: false,
                };
            }
        }
        if node.dataless {
            return ScanNode {
                path: path.into(),
                is_dir: true,
                allocated: node.allocated,
                apparent: 0,
                entries: 0,
                error: Some(FsError::Dataless(path.into())),
                children: vec![],
                link: false,
            };
        }
        let mut out = ScanNode {
            path: path.into(),
            is_dir: true,
            allocated: node.allocated,
            apparent: node.apparent,
            entries: 0,
            error: None,
            children: vec![],
            link: false,
        };
        if !node.readable {
            out.error = Some(FsError::PermissionDenied(path.into()));
            return out;
        }
        if opts.max_depth.is_some_and(|d| depth >= d) {
            for n in self.subtree(path) {
                if matches!(n.kind, NodeKind::Symlink { .. }) {
                    out.entries += 1;
                    continue;
                }
                if opts.skip.iter().any(|s| n.path.starts_with(s)) {
                    continue;
                }
                if !opts.include_hidden && n.hidden() {
                    continue;
                }
                if n.is_dir() {
                    out.allocated = out.allocated.saturating_add(n.allocated);
                    continue;
                }
                if seen.insert(n.inode) {
                    out.allocated = out.allocated.saturating_add(n.allocated);
                    out.apparent = out.apparent.saturating_add(n.apparent);
                }
                out.entries += 1;
            }
            return out;
        }
        let Ok(children) = self.list(path) else {
            out.error = Some(FsError::PermissionDenied(path.into()));
            return out;
        };
        for c in children {
            if opts
                .skip
                .iter()
                .any(|s| c.path == *s || c.path.starts_with(&format!("{s}/")))
            {
                continue;
            }
            if !opts.include_hidden && c.hidden() {
                continue;
            }
            let child = self.scan_node(&c.path, opts, depth + 1, seen);
            out.allocated = out.allocated.saturating_add(child.allocated);
            out.apparent = out.apparent.saturating_add(child.apparent);
            out.entries = out.entries.saturating_add(child.entries);
            out.children.push(child);
        }
        out.children.sort_by(|a, b| {
            b.allocated
                .cmp(&a.allocated)
                .then_with(|| a.path.cmp(&b.path))
        });
        out
    }

    pub fn size_of(&self, path: &str) -> u64 {
        self.scan(
            path,
            &ScanOptions {
                include_hidden: true,
                ..Default::default()
            },
        )
        .allocated
    }

    pub fn volume_for(&self, path: &str) -> Option<&Volume> {
        self.volumes
            .iter()
            .filter(|v| {
                path == v.mount
                    || path.starts_with(&format!("{}/", v.mount.trim_end_matches('/')))
                    || v.mount == "/"
            })
            .max_by_key(|v| v.mount.len())
    }

    pub fn trash(&mut self, path: &str, now_secs: i64) -> Result<u64, String> {
        if let Some(reason) = &self.trash_broken {
            return Err(format!("Trash unavailable: {reason}"));
        }
        let mut attempts = 0;
        while self.trash_collisions > 0 {
            self.trash_collisions -= 1;
            attempts += 1;
            if attempts >= 3 {
                return Err("Trash: AlreadyExists after 3 attempts".into());
            }
        }
        let bytes = self.detach(path)?;
        let name = path.rsplit('/').next().unwrap_or("item").to_owned();
        let trash_path = format!("~/.Trash/{name}");
        self.trash.push(Trashed {
            original: path.into(),
            trash_path,
            bytes,
            at_secs: now_secs,
        });
        Ok(bytes)
    }

    pub fn remove_permanent(&mut self, path: &str) -> Result<u64, String> {
        let bytes = self.detach(path)?;
        if let Some(v) = self.volume_mut_for(path) {
            v.used = v.used.saturating_sub(bytes);
        }
        Ok(bytes)
    }

    fn volume_mut_for(&mut self, path: &str) -> Option<&mut Volume> {
        let mount = self.volume_for(path).map(|v| v.mount.clone())?;
        self.volumes.iter_mut().find(|v| v.mount == mount)
    }

    fn detach(&mut self, path: &str) -> Result<u64, String> {
        let node = self
            .nodes
            .get(path)
            .cloned()
            .ok_or_else(|| FsError::NotFound(path.into()).message())?;
        if let Some(parent) = Self::parent(path)
            && self.nodes.get(&parent).is_some_and(|p| !p.readable)
        {
            return Err(FsError::PermissionDenied(parent).message());
        }
        match node.kind {
            NodeKind::Symlink { .. } => {
                self.nodes.remove(path);
                Ok(0)
            }
            NodeKind::Dir => {
                if !node.readable {
                    return Err(FsError::PermissionDenied(path.into()).message());
                }
                let bytes = self.size_of(path);
                let doomed: Vec<String> =
                    self.subtree(path).iter().map(|n| n.path.clone()).collect();
                for p in doomed {
                    self.nodes.remove(&p);
                }
                self.nodes.remove(path);
                Ok(bytes)
            }
            _ => {
                self.nodes.remove(path);
                Ok(node.allocated)
            }
        }
    }

    pub fn home_index(&self, home: &str) -> Vec<String> {
        #[cfg(test)]
        HOME_INDEX_CALLS.with(|c| c.set(c.get() + 1));
        let mut out = vec![];
        let ignores = self.ignore_names(home);
        self.index_dir(home, home, &ignores, &mut out, 0);
        out
    }

    fn ignore_names(&self, home: &str) -> BTreeSet<String> {
        let mut set = BTreeSet::new();
        if let Some(Node {
            content: Content::Text(t),
            ..
        }) = self.nodes.get(&format!("{home}/.ignore"))
        {
            for l in t.lines() {
                let l = l.trim();
                if !l.is_empty() && !l.starts_with('#') {
                    set.insert(l.trim_end_matches('/').to_owned());
                }
            }
        }
        set
    }

    fn index_dir(
        &self,
        home: &str,
        dir: &str,
        ignores: &BTreeSet<String>,
        out: &mut Vec<String>,
        depth: usize,
    ) {
        let Ok(children) = self.list(dir) else {
            return;
        };
        for c in children {
            if c.hidden() || ignores.contains(c.name()) {
                continue;
            }
            if dir == home && c.name() == "Library" {
                if let Ok(lib) = self.list(&c.path) {
                    for l in lib {
                        if l.name().starts_with("Mobile Documents") || l.hidden() {
                            continue;
                        }
                        out.push(l.path.clone());
                        if l.is_dir() && depth < 12 {
                            self.index_dir(home, &l.path, ignores, out, depth + 1);
                        }
                    }
                }
                continue;
            }
            match c.kind {
                NodeKind::Symlink { .. } => {
                    out.push(c.path.clone());
                }
                NodeKind::Dir => {
                    out.push(c.path.clone());
                    if depth < 12 {
                        self.index_dir(home, &c.path, ignores, out, depth + 1);
                    }
                }
                _ => out.push(c.path.clone()),
            }
        }
    }

    pub fn find(&self, index: &[String], query: &str) -> Vec<FindHit> {
        #[cfg(test)]
        FIND_CALLS.with(|c| c.set(c.get() + 1));
        let q = query.trim();
        if q.is_empty() {
            return vec![];
        }
        let lq = q.to_lowercase();
        let mut hits: Vec<FindHit> = vec![];
        for p in index {
            let name = p.rsplit('/').next().unwrap_or(p);
            let lname = name.to_lowercase();
            let stem = lname.rsplit_once('.').map(|(s, _)| s).unwrap_or(&lname);
            let (score, matched) = if lname == lq || stem == lq {
                (
                    0,
                    junie_tui::ui::text::fuzzy(name, q)
                        .map(|(_, m)| m)
                        .unwrap_or_default(),
                )
            } else if let Some(i) = lname.find(&lq) {
                (
                    10,
                    junie_tui::ui::text::fuzzy(name, q)
                        .map(|(_, m)| m)
                        .unwrap_or_else(|| (i..i + lq.len()).collect()),
                )
            } else if let Some((s, m)) = junie_tui::ui::text::fuzzy(name, q) {
                (50 + s, m)
            } else {
                continue;
            };
            hits.push(FindHit {
                path: p.clone(),
                is_dir: self.is_dir(p),
                matched,
                score,
            });
        }
        hits.sort_by(|a, b| a.score.cmp(&b.score).then_with(|| a.path.cmp(&b.path)));
        hits.truncate(FIND_MAX_RESULTS);
        hits
    }
}

pub fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\t' => out.push_str("    "),
            c if c.is_control() => out.push('�'),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
thread_local! {
    static HOME_INDEX_CALLS: Cell<u64> = const { Cell::new(0) };
    static FIND_CALLS: Cell<u64> = const { Cell::new(0) };
}

/// Calls to `home_index` and `find` on the current test thread. Counters are
/// per thread, so parallel tests do not see each other's calls.
#[cfg(test)]
pub fn call_counts() -> (u64, u64) {
    (HOME_INDEX_CALLS.with(Cell::get), FIND_CALLS.with(Cell::get))
}
