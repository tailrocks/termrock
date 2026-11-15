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
    /// Nodes moved into Trash. They keep their inodes alive, so a later
    /// permanent removal of another link must not reclaim those blocks.
    retained_nodes: Vec<Node>,
}

/// Accounting for one removal. `removed_allocated_bytes` is the processed
/// estimate: each removed inode counts once, even when other links survive.
/// `reclaimed_bytes` counts only inodes whose last reference was removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RemovalStats {
    pub removed_entry_bytes: u64,
    pub removed_allocated_bytes: u64,
    pub reclaimed_bytes: u64,
}

struct Removal {
    keys: Vec<String>,
    nodes: Vec<Node>,
    stats: RemovalStats,
    reclaimed_by_mount: BTreeMap<String, u64>,
}

fn checked_total(a: u64, b: u64) -> Result<u64, String> {
    a.checked_add(b)
        .ok_or_else(|| "simulated allocation total exceeds u64 · nothing removed".to_owned())
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
    content_revision: u64,
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

    /// Counts the mutations that changed a node. Projection caches compare it
    /// instead of scanning the tree; a failed or no-op operation never moves it.
    pub fn content_revision(&self) -> u64 {
        self.content_revision
    }

    fn changed(&mut self) {
        self.content_revision = self.content_revision.wrapping_add(1);
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
        self.changed();
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
        if let Some(n) = self.nodes.get_mut(path)
            && n.allocated != allocated
        {
            n.allocated = allocated;
            self.changed();
        }
        self
    }

    pub fn text(&mut self, path: &str, text: &str, age_days: i64) -> &mut Self {
        self.file(path, text.len() as u64, age_days);
        if let Some(n) = self.nodes.get_mut(path) {
            n.content = Content::Text(text.into());
            self.changed();
        }
        self
    }

    pub fn bytes(&mut self, path: &str, bytes: Vec<u8>, age_days: i64) -> &mut Self {
        self.file(path, bytes.len() as u64, age_days);
        if let Some(n) = self.nodes.get_mut(path) {
            n.content = Content::Bytes(bytes);
            self.changed();
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
            self.changed();
        }
        self
    }

    pub fn unreadable(&mut self, path: &str) -> &mut Self {
        if let Some(n) = self.nodes.get_mut(path)
            && n.readable
        {
            n.readable = false;
            self.changed();
        }
        self
    }

    pub fn dataless(&mut self, path: &str) -> &mut Self {
        if let Some(n) = self.nodes.get_mut(path)
            && !n.dataless
        {
            n.dataless = true;
            self.changed();
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
            let mtime = EPOCH_SECS - age_days * 86_400;
            if n.mtime != mtime {
                n.mtime = mtime;
                self.changed();
            }
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
        let raw: &[u8] = match &node.content {
            Content::Text(t) => t.as_bytes(),
            Content::Bytes(b) => b.as_slice(),
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

    pub fn trash(&mut self, path: &str, now_secs: i64) -> Result<RemovalStats, String> {
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
        let removal = self.plan_removal(path)?;
        let name = path.rsplit('/').next().unwrap_or("item").to_owned();
        let trash_path = format!("~/.Trash/{name}");
        let stats = removal.stats;
        self.commit_removal(&removal.keys);
        self.trash.push(Trashed {
            original: path.into(),
            trash_path,
            bytes: stats.removed_allocated_bytes,
            at_secs: now_secs,
            retained_nodes: removal.nodes,
        });
        // Trash keeps every inode alive, so no block returns to the volume.
        Ok(RemovalStats {
            reclaimed_bytes: 0,
            ..stats
        })
    }

    pub fn remove_permanent(&mut self, path: &str) -> Result<RemovalStats, String> {
        let removal = self.plan_removal(path)?;
        self.commit_removal(&removal.keys);
        for (mount, bytes) in &removal.reclaimed_by_mount {
            if let Some(v) = self.volumes.iter_mut().find(|v| &v.mount == mount) {
                v.used = v.used.saturating_sub(*bytes);
            }
        }
        Ok(removal.stats)
    }

    fn commit_removal(&mut self, keys: &[String]) {
        for k in keys {
            self.nodes.remove(k);
        }
        self.changed();
    }

    /// Validates a removal and computes its accounting without mutating
    /// anything. An inode's blocks count as reclaimed only when no reference
    /// survives outside the removed set, where Trash-retained nodes count as
    /// surviving references.
    fn plan_removal(&self, path: &str) -> Result<Removal, String> {
        let keys = self.removal_keys(path)?;
        let key_set: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
        let mut nodes = Vec::with_capacity(keys.len());
        let mut removed_by_inode: BTreeMap<u64, &Node> = BTreeMap::new();
        let mut removed_entry_bytes = 0_u64;
        for k in &keys {
            let Some(node) = self.nodes.get(k) else {
                continue;
            };
            removed_entry_bytes = checked_total(removed_entry_bytes, node.apparent)?;
            removed_by_inode.entry(node.inode).or_insert(node);
            nodes.push(node.clone());
        }

        // Every live reference, removed or not, plus every Trash-retained one.
        // A live reference outside the removal, or any retained one, survives.
        let live_refs = self
            .nodes
            .iter()
            .map(|(p, n)| (n, !key_set.contains(p.as_str())));
        let retained_refs = self
            .trash
            .iter()
            .flat_map(|t| t.retained_nodes.iter())
            .map(|n| (n, true));
        let mut survivors: BTreeSet<u64> = BTreeSet::new();
        for (node, survives) in live_refs.chain(retained_refs) {
            let Some(first) = removed_by_inode.get(&node.inode) else {
                continue;
            };
            if std::mem::discriminant(&first.kind) != std::mem::discriminant(&node.kind)
                || first.allocated != node.allocated
            {
                return Err(format!(
                    "{path}: inode {} has inconsistent references · nothing removed",
                    node.inode
                ));
            }
            if self.volume_for(&first.path).map(|v| &v.mount)
                != self.volume_for(&node.path).map(|v| &v.mount)
            {
                return Err(format!(
                    "{path}: cross-volume hardlink is not modeled · nothing removed"
                ));
            }
            if survives {
                survivors.insert(node.inode);
            }
        }

        let mut removed_allocated_bytes = 0_u64;
        let mut reclaimed_bytes = 0_u64;
        let mut reclaimed_by_mount: BTreeMap<String, u64> = BTreeMap::new();
        for (inode, node) in &removed_by_inode {
            removed_allocated_bytes = checked_total(removed_allocated_bytes, node.allocated)?;
            if survivors.contains(inode) {
                continue;
            }
            reclaimed_bytes = checked_total(reclaimed_bytes, node.allocated)?;
            if let Some(v) = self.volume_for(&node.path) {
                let total = reclaimed_by_mount.entry(v.mount.clone()).or_insert(0);
                *total = checked_total(*total, node.allocated)?;
            }
        }

        Ok(Removal {
            keys,
            nodes,
            reclaimed_by_mount,
            stats: RemovalStats {
                removed_entry_bytes,
                removed_allocated_bytes,
                reclaimed_bytes,
            },
        })
    }

    fn removal_keys(&self, path: &str) -> Result<Vec<String>, String> {
        let node = self
            .nodes
            .get(path)
            .ok_or_else(|| FsError::NotFound(path.into()).message())?;
        if let Some(parent) = Self::parent(path)
            && self.nodes.get(&parent).is_some_and(|p| !p.readable)
        {
            return Err(FsError::PermissionDenied(parent).message());
        }
        let mut keys = vec![path.to_owned()];
        if node.is_dir() {
            if !node.readable {
                return Err(FsError::PermissionDenied(path.into()).message());
            }
            keys.extend(self.subtree(path).iter().map(|n| n.path.clone()));
        }
        Ok(keys)
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
mod link_reclaim_tests {
    use super::*;

    fn stats(entry: u64, allocated: u64, reclaimed: u64) -> RemovalStats {
        RemovalStats {
            removed_entry_bytes: entry,
            removed_allocated_bytes: allocated,
            reclaimed_bytes: reclaimed,
        }
    }

    fn used(fs: &Fs, mount: &str) -> u64 {
        fs.volumes.iter().find(|v| v.mount == mount).unwrap().used
    }

    #[test]
    fn fs_hardlink_reclaims_only_last_live_reference() {
        let mut fs = Fs::new();
        fs.dir("/w", 1)
            .file("/w/a", 100, 1)
            .hardlink("/w/b", "/w/a")
            .hardlink("/w/c", "/w/a")
            .volume("/", 1 << 30, 1 << 20);

        assert_eq!(
            fs.remove_permanent("/w/a").unwrap(),
            stats(100, BLOCK, 0),
            "first link processes its blocks but reclaims none"
        );
        assert_eq!(fs.remove_permanent("/w/b").unwrap(), stats(100, BLOCK, 0));
        assert_eq!(
            fs.remove_permanent("/w/c").unwrap(),
            stats(100, BLOCK, BLOCK),
            "the last reference reclaims the inode once"
        );
        assert_eq!(used(&fs, "/"), (1 << 20) - BLOCK);
    }

    #[test]
    fn fs_subtree_removal_counts_inodes_once() {
        let mut inside = Fs::new();
        inside
            .dir("/w/proj", 1)
            .file("/w/proj/a", 100, 1)
            .hardlink("/w/proj/b", "/w/proj/a")
            .volume("/", 1 << 30, 1 << 20);
        assert_eq!(
            inside.remove_permanent("/w/proj").unwrap(),
            stats(200, 2 * BLOCK, 2 * BLOCK),
            "dir plus one shared inode, each counted once, all links inside"
        );

        let mut outside = Fs::new();
        outside
            .dir("/w/proj", 1)
            .file("/w/proj/a", 100, 1)
            .hardlink("/w/proj/b", "/w/proj/a")
            .hardlink("/w/outside", "/w/proj/a")
            .volume("/", 1 << 30, 1 << 20);
        assert_eq!(
            outside.remove_permanent("/w/proj").unwrap(),
            stats(200, 2 * BLOCK, BLOCK),
            "a surviving link outside the subtree keeps the shared inode alive"
        );
        assert_eq!(used(&outside, "/"), (1 << 20) - BLOCK);
    }

    #[test]
    fn fs_trashed_hardlink_retains_allocation() {
        let mut fs = Fs::new();
        fs.dir("/w", 1)
            .file("/w/a", 100, 1)
            .hardlink("/w/b", "/w/a")
            .volume("/", 1 << 30, 1 << 20);

        let trashed = fs.trash("/w/a", 0).unwrap();
        assert_eq!(trashed, stats(100, BLOCK, 0), "trash never reclaims");
        assert_eq!(fs.trash.len(), 1);
        assert_eq!(fs.trash[0].bytes, BLOCK);
        assert_eq!(used(&fs, "/"), 1 << 20, "trash leaves volume usage alone");

        assert_eq!(
            fs.remove_permanent("/w/b").unwrap(),
            stats(100, BLOCK, 0),
            "the Trash record still owns the inode"
        );
        assert_eq!(used(&fs, "/"), 1 << 20);
    }

    #[test]
    fn fs_trash_directory_retains_descendant_inodes() {
        let mut fs = Fs::new();
        fs.dir("/w/d", 1)
            .file("/w/d/a", 100, 1)
            .hardlink("/w/out", "/w/d/a")
            .volume("/", 1 << 30, 1 << 20);

        assert_eq!(fs.trash("/w/d", 0).unwrap(), stats(100, 2 * BLOCK, 0));
        assert_eq!(
            fs.remove_permanent("/w/out").unwrap(),
            stats(100, BLOCK, 0),
            "a retained descendant inode cannot be reclaimed through a live link"
        );
        assert_eq!(used(&fs, "/"), 1 << 20);
    }

    #[test]
    fn fs_removal_permission_and_cross_volume_errors_are_atomic() {
        let mut locked = Fs::new();
        locked
            .dir("/w/locked", 1)
            .file("/w/locked/a", 100, 1)
            .unreadable("/w/locked")
            .volume("/", 1 << 30, 1 << 20);
        let before = locked.clone();
        assert!(locked.remove_permanent("/w/locked").is_err());
        assert_eq!(locked, before, "denied removal changes nothing");

        let mut cross = Fs::new();
        cross
            .dir("/w", 1)
            .file("/w/a", 100, 1)
            .volume("/", 1 << 30, 1 << 20)
            .volume("/ext", 1 << 30, 1 << 20)
            .hardlink("/ext/b", "/w/a");
        let before = cross.clone();
        let err = cross.remove_permanent("/w/a").unwrap_err();
        assert!(err.contains("cross-volume hardlink"), "{err}");
        assert_eq!(cross, before, "cross-volume refusal changes nothing");
    }

    #[test]
    fn fs_removal_size_overflow_is_atomic() {
        let mut fs = Fs::new();
        fs.dir("/w/d", 1)
            .sparse("/w/d/a", 1, u64::MAX - 10, 1)
            .sparse("/w/d/b", 1, 20, 1)
            .volume("/", 1 << 30, 1 << 20);
        let before = fs.clone();
        let err = fs.remove_permanent("/w/d").unwrap_err();
        assert!(err.contains("exceeds u64"), "{err}");
        assert_eq!(fs, before, "overflow refusal changes nothing");
    }

    #[test]
    fn fs_inconsistent_inode_refuses_before_mutation() {
        for outside in ["/w/a_outside", "/w/z_outside"] {
            let mut fs = Fs::new();
            fs.dir("/w/d", 1)
                .file("/w/d/a", 100, 1)
                .hardlink(outside, "/w/d/a")
                .volume("/", 1 << 30, 1 << 20);
            fs.nodes.get_mut(outside).unwrap().allocated = 2 * BLOCK;
            let before = fs.clone();
            let err = fs.remove_permanent("/w/d").unwrap_err();
            assert!(err.contains("inconsistent references"), "{err}");
            assert_eq!(fs, before, "an inconsistent inode refuses without mutation");
        }
    }

    #[test]
    fn fs_subtree_reclaims_each_volume_independently() {
        let mut fs = Fs::new();
        fs.dir("/work", 1)
            .file("/work/one", 100, 1)
            .dir("/work/ext", 1)
            .file("/work/ext/two", 100, 1)
            .volume("/", 1 << 30, 100 * BLOCK)
            .volume("/work/ext", 1 << 30, 50 * BLOCK);

        assert_eq!(
            fs.remove_permanent("/work").unwrap(),
            stats(200, 4 * BLOCK, 4 * BLOCK)
        );
        assert_eq!(
            used(&fs, "/"),
            98 * BLOCK,
            "root volume loses its own two inodes"
        );
        assert_eq!(
            used(&fs, "/work/ext"),
            48 * BLOCK,
            "the mounted volume loses only its own two inodes"
        );
    }
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

#[cfg(test)]
mod tests;
