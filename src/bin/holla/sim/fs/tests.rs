use super::*;
use crate::alloc_probe::allocated_by;

const PATH: &str = "/work/file.txt";
const MIB: usize = 1024 * 1024;
const CAP: usize = PREVIEW_MAX_BYTES;

fn fs_text(text: &str) -> Fs {
    let mut fs = Fs::new();
    fs.text(PATH, text, 0);
    fs
}

fn fs_bytes(bytes: Vec<u8>) -> Fs {
    let mut fs = Fs::new();
    fs.bytes(PATH, bytes, 0);
    fs
}

fn preview_of(fs: &Fs) -> Preview {
    fs.preview(PATH)
}

#[test]
fn fs_preview_borrowed_prefix_matches_existing_contract() {
    assert_eq!(preview_of(&fs_text("")), Preview::Empty);

    let mut empty_modelled = Fs::new();
    empty_modelled.file(PATH, 0, 0);
    assert_eq!(preview_of(&empty_modelled), Preview::Empty);

    let mut unmodelled = Fs::new();
    unmodelled.file(PATH, 10, 0);
    assert_eq!(
        preview_of(&unmodelled),
        Preview::Binary {
            reason: "content not modelled · treated as binary".into(),
        }
    );

    assert_eq!(
        preview_of(&fs_text("alpha\nbeta\n")),
        Preview::Text {
            lines: vec!["alpha".into(), "beta".into()],
            bytes_shown: 11,
            total_bytes: 11,
            truncated_bytes: false,
            truncated_lines: false,
            long_lines: 0,
            link: None,
        }
    );

    assert_eq!(
        preview_of(&fs_bytes(b"ab\0cd".to_vec())),
        Preview::Binary {
            reason: "NUL byte in the first 256 KiB".into(),
        }
    );
    assert_eq!(
        preview_of(&fs_bytes(vec![b'o', b'k', 0xFF, b'\n'])),
        Preview::Binary {
            reason: "invalid UTF-8".into(),
        }
    );

    let long = format!("{}\nshort\n", "x".repeat(5000));
    assert_eq!(
        preview_of(&fs_text(&long)),
        Preview::Text {
            lines: vec![format!("{} …", "x".repeat(4096)), "short".into()],
            bytes_shown: long.len(),
            total_bytes: long.len() as u64,
            truncated_bytes: false,
            truncated_lines: false,
            long_lines: 1,
            link: None,
        }
    );

    let many_lines = "ab\n".repeat(2500);
    assert_eq!(
        preview_of(&fs_text(&many_lines)),
        Preview::Text {
            lines: vec!["ab".into(); PREVIEW_MAX_LINES],
            bytes_shown: many_lines.len(),
            total_bytes: many_lines.len() as u64,
            truncated_bytes: false,
            truncated_lines: true,
            long_lines: 0,
            link: None,
        }
    );

    let mut straddle = "y".repeat(CAP - 2);
    straddle.push('€');
    straddle.push_str("tail");
    assert_eq!(
        preview_of(&fs_text(&straddle)),
        Preview::Text {
            lines: vec![format!("{} …", "y".repeat(4096))],
            bytes_shown: CAP - 2,
            total_bytes: straddle.len() as u64,
            truncated_bytes: true,
            truncated_lines: false,
            long_lines: 1,
            link: None,
        }
    );

    let mut suffix_invalid = b"a\n".repeat(CAP);
    suffix_invalid.push(0xFF);
    assert_eq!(
        preview_of(&fs_bytes(suffix_invalid.clone())),
        Preview::Text {
            lines: vec!["a".into(); PREVIEW_MAX_LINES],
            bytes_shown: CAP,
            total_bytes: suffix_invalid.len() as u64,
            truncated_bytes: true,
            truncated_lines: true,
            long_lines: 0,
            link: None,
        }
    );

    let mut linked = Fs::new();
    linked.text("/work/target.txt", "hi\n", 0);
    linked.symlink(PATH, "/work/target.txt");
    assert_eq!(
        preview_of(&linked),
        Preview::Text {
            lines: vec!["hi".into()],
            bytes_shown: 3,
            total_bytes: 3,
            truncated_bytes: false,
            truncated_lines: false,
            long_lines: 0,
            link: Some("/work/target.txt".into()),
        }
    );

    let mut broken = Fs::new();
    broken.symlink(PATH, "/work/missing");
    assert_eq!(
        preview_of(&broken),
        Preview::Error(FsError::BrokenLink(PATH.into()))
    );

    let mut unreadable = fs_text("secret\n");
    unreadable.unreadable(PATH);
    assert_eq!(
        preview_of(&unreadable),
        Preview::Error(FsError::PermissionDenied(PATH.into()))
    );

    let mut dataless = fs_text("cloud\n");
    dataless.dataless(PATH);
    assert_eq!(
        preview_of(&dataless),
        Preview::Error(FsError::Dataless(PATH.into()))
    );

    let mut fifo = Fs::new();
    fifo.fifo(PATH);
    assert_eq!(
        preview_of(&fifo),
        Preview::Error(FsError::SpecialFile(PATH.into()))
    );

    let mut device = Fs::new();
    device.device(PATH);
    assert_eq!(
        preview_of(&device),
        Preview::Error(FsError::SpecialFile(PATH.into()))
    );

    let mut dir = Fs::new();
    dir.dir(PATH, 0);
    assert_eq!(preview_of(&dir), Preview::Directory);

    assert_eq!(
        Fs::new().preview(PATH),
        Preview::Error(FsError::NotFound(PATH.into()))
    );
}

fn text_fixture(total: usize) -> Fs {
    let unit = "the same visible prefix of a large file\n";
    let mut text = unit.repeat(total / unit.len() + 1);
    text.truncate(total);
    fs_text(&text)
}

fn bytes_fixture(total: usize) -> Fs {
    let unit = b"the same visible prefix of a large file\n";
    let bytes: Vec<u8> = unit.iter().cycle().take(total).copied().collect();
    fs_bytes(bytes)
}

#[test]
fn fs_preview_source_copy_is_independent_of_total_length() {
    let fixtures = [
        ("text", text_fixture as fn(usize) -> Fs),
        ("bytes", bytes_fixture),
    ];
    for (kind, make) in fixtures {
        let small = make(MIB);
        let big = make(16 * MIB);

        let (small_out, small_requested) = allocated_by(|| small.preview(PATH));
        let (big_out, big_requested) = allocated_by(|| big.preview(PATH));

        let (
            Preview::Text {
                lines: small_lines,
                bytes_shown: small_shown,
                ..
            },
            Preview::Text {
                lines: big_lines,
                bytes_shown: big_shown,
                total_bytes,
                truncated_bytes,
                ..
            },
        ) = (small_out, big_out)
        else {
            panic!("{kind}: both previews are text");
        };
        assert_eq!(big_lines, small_lines, "{kind}: same visible prefix");
        assert_eq!(big_shown, small_shown, "{kind}: same bytes shown");
        assert_eq!(big_shown, CAP, "{kind}: the cap bounds what is shown");
        assert_eq!(
            total_bytes,
            16 * MIB as u64,
            "{kind}: metadata keeps the full length"
        );
        assert!(truncated_bytes, "{kind}: truncation is reported");

        assert_eq!(
            big_requested, small_requested,
            "{kind}: bytes requested from the allocator must not depend on total length \
             (1 MiB {small_requested} B, 16 MiB {big_requested} B)"
        );
        assert!(
            big_requested < 4 * CAP as u64,
            "{kind}: {big_requested} B requested for a 16 MiB file, expected under 4 x the cap"
        );
    }
}
