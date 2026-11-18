use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WidgetId(u64);

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

const fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

impl WidgetId {
    pub const fn of(path: &str) -> Self {
        Self(fnv1a(FNV_OFFSET, path.as_bytes()))
    }

    pub const fn child(self, index: usize) -> Self {
        Self(fnv1a(self.0, &index.to_le_bytes()))
    }

    pub const fn sub(self, name: &str) -> Self {
        Self(fnv1a(self.0, name.as_bytes()))
    }

    pub const fn part(self, p: PartRef) -> Self {
        let base = match p.item {
            None => self,
            Some(ItemKey::Index(i)) => self.child(i),
            Some(ItemKey::Num(n)) => Self(fnv1a(fnv1a(self.0, b"#num"), &n.to_le_bytes())),
            Some(ItemKey::Text(h)) => Self(fnv1a(fnv1a(self.0, b"#text"), &h.to_le_bytes())),
        };
        match (p.part, p.item) {
            (Part::CONTAINER, None) => self,
            (Part::ROW | Part::TAB, Some(_)) => base,
            _ => Self(fnv1a(fnv1a(base.0, b"#part"), &p.part.raw().to_le_bytes())),
        }
    }

    pub fn item_of(
        self,
        id: WidgetId,
        part: Part,
        items: impl IntoIterator<Item = usize>,
    ) -> Option<usize> {
        items
            .into_iter()
            .find(|&i| self.part(PartRef::item(part, ItemKey::Index(i))) == id)
    }
}

impl fmt::Debug for WidgetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WidgetId({:016x})", self.0)
    }
}

pub const fn custom_hash16(name: &str) -> u16 {
    let h = fnv1a(FNV_OFFSET, name.as_bytes());
    0x8000 | ((h ^ (h >> 16) ^ (h >> 32) ^ (h >> 48)) as u16 & 0x7FFF)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum ItemKey {
    Index(usize),

    Num(u64),

    Text(u64),
}

impl ItemKey {
    pub const fn index(i: usize) -> Self {
        ItemKey::Index(i)
    }

    pub const fn num(n: u64) -> Self {
        ItemKey::Num(n)
    }

    pub const fn text(s: &str) -> Self {
        ItemKey::Text(fnv1a(FNV_OFFSET, s.as_bytes()))
    }

    pub const fn pair(a: u64, b: u64) -> Self {
        let h = fnv1a(FNV_OFFSET, &a.to_le_bytes());
        ItemKey::Text(fnv1a(h, &b.to_le_bytes()))
    }

    pub const fn as_index(self) -> Option<usize> {
        match self {
            ItemKey::Index(i) => Some(i),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Part(u16);

macro_rules! parts {
    ($( $(#[$m:meta])* $name:ident = $v:expr ),* $(,)?) => {
        impl Part {
            $( $(#[$m])* pub const $name: Part = Part($v); )*

            pub const ALL: &'static [Part] = &[ $( Part::$name ),* ];

            pub const fn name(self) -> Option<&'static str> {
                match self {
                    $( Part::$name => Some(stringify!($name)), )*
                    _ => None,
                }
            }
        }
    };
}

parts! {

    CONTAINER = 0,

    BACKDROP = 2,

    MARKER = 4,

    ICON = 5,

    TITLE = 9,

    ACTIONS = 11,

    FIELD = 12,

    ROW = 15,

    CELL = 16,

    HEADER = 17,

    TRACK = 18,

    THUMB = 19,

    TAB = 21,

    CLOSE = 22,

    OVERFLOW = 25,

    NEW = 26,

    QUERY = 28,

    SEAM = 29,

    KEY = 32,

    ROW_NUMBER = 35,
}

impl Part {
    pub const fn custom(name: &str) -> Part {
        Part(custom_hash16(name))
    }

    pub const fn raw(self) -> u16 {
        self.0
    }
}

impl fmt::Debug for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => write!(f, "Part::{name}"),
            None => write!(f, "Part({:#06x})", self.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PartRef {
    pub part: Part,

    pub item: Option<ItemKey>,
}

impl PartRef {
    pub const OVERFLOW_BEFORE: PartRef = PartRef::item(Part::OVERFLOW, ItemKey::Index(0));

    pub const OVERFLOW_AFTER: PartRef = PartRef::item(Part::OVERFLOW, ItemKey::Index(1));

    pub const fn of(part: Part) -> Self {
        Self { part, item: None }
    }

    pub const fn item(part: Part, key: ItemKey) -> Self {
        Self {
            part,
            item: Some(key),
        }
    }
}
