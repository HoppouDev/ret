use super::*;

use std::io::Write as _;

use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// An archive with one file per entry, compressed the same way as the platform tools
fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn extracts_the_entries_under_their_directories() {
    let dir = std::env::temp_dir().join(format!("ret-adb-download-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    extract(
        &archive(&[
            ("platform-tools/readme", b"data"),
            ("platform-tools/adb", b"tool"),
        ]),
        &dir,
    )
    .unwrap();
    assert_eq!(
        fs::read(dir.join("platform-tools/readme")).unwrap(),
        b"data"
    );
    assert_eq!(fs::read(dir.join("platform-tools/adb")).unwrap(), b"tool");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn skips_the_entries_which_escape_the_archive() {
    let dir = std::env::temp_dir().join(format!("ret-adb-escape-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    extract(&archive(&[("../escaped", b"data")]), &dir).unwrap();
    assert!(!dir.parent().unwrap().join("escaped").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn reads_unknown_and_known_lengths_and_publishes_actual_final_bytes() {
    for total in [None, Some(0), Some(4)] {
        let mut updates = Vec::new();
        let bytes = read_archive(&b"data"[..], total, 4, &mut |p| updates.push(p)).unwrap();
        assert_eq!(bytes, b"data");
        let total = total.filter(|total| *total > 0);
        assert_eq!(
            updates.first(),
            Some(&Progress::Downloading {
                downloaded: 0,
                total
            })
        );
        assert_eq!(
            updates.last(),
            Some(&Progress::Downloading {
                downloaded: 4,
                total
            })
        );
    }
}

#[test]
fn rejects_oversized_bodies_regardless_of_reported_length() {
    for total in [None, Some(0), Some(1), Some(5)] {
        let mut reader = io::Cursor::new(b"large");
        let error = read_archive(&mut reader, total, 4, &mut |_| {}).unwrap_err();
        assert!(error.to_string().contains("size limit"));
        assert!(reader.position() <= 5);
    }
}

#[test]
fn propagates_truncated_and_failed_reads_without_final_progress() {
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "connection lost",
            ))
        }
    }
    let mut updates = Vec::new();
    let error = read_archive(&b"short"[..], Some(6), 8, &mut |p| updates.push(p)).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    assert_eq!(
        updates,
        [Progress::Downloading {
            downloaded: 0,
            total: Some(6)
        }]
    );
    let error = read_archive(Broken, None, 8, &mut |_| {}).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::ConnectionReset);
    assert_eq!(error.to_string(), "connection lost");
}

#[test]
fn installation_publishes_failure_then_retry_extraction_and_ready() {
    let dir = std::env::temp_dir().join(format!("ret-adb-progress-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let mut updates = Vec::new();
    let mut publish = |p| updates.push(p);
    let error = install(&dir, |_| Err(io::Error::other("offline")), &mut publish).unwrap_err();
    assert_eq!(error.to_string(), "offline");
    assert_eq!(updates.last(), Some(&Progress::Failed("offline".into())));
    assert!(!dir.join("platform-tools").join(ADB).exists());
    updates.clear();
    let path = install(
        &dir,
        |_| Ok(archive(&[(&format!("platform-tools/{ADB}"), b"tool")])),
        &mut |p| updates.push(p),
    )
    .unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"tool");
    assert_eq!(
        updates,
        [
            Progress::Downloading {
                downloaded: 0,
                total: None
            },
            Progress::Extracting,
            Progress::Ready,
        ]
    );
    updates.clear();
    assert_eq!(
        install(
            &dir,
            |_| panic!("cached tools must not download"),
            &mut |p| updates.push(p)
        )
        .unwrap(),
        path
    );
    assert!(updates.is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn extraction_and_rename_failures_never_publish_ready() {
    for (suffix, bytes, block_destination) in [
        ("invalid", b"not a zip".to_vec(), false),
        (
            "missing",
            archive(&[("platform-tools/readme", b"data")]),
            false,
        ),
        (
            "rename",
            archive(&[(&format!("platform-tools/{ADB}"), b"tool")]),
            true,
        ),
    ] {
        let dir = std::env::temp_dir().join(format!("ret-adb-{suffix}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let destination = if block_destination {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt as _;
                options.share_mode(0);
            }
            Some(options.open(dir.join("platform-tools")).unwrap())
        } else {
            None
        };
        let mut updates = Vec::new();
        install(&dir, |_| Ok(bytes), &mut |p| updates.push(p)).unwrap_err();
        assert!(matches!(updates.last(), Some(Progress::Failed(_))));
        assert!(updates.contains(&Progress::Extracting));
        assert!(!updates.contains(&Progress::Ready));
        drop(destination);
        let _ = fs::remove_dir_all(&dir);
    }
}
