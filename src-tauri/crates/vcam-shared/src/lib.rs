pub mod error;
pub mod ring_buffer;

#[cfg(windows)]
pub mod owner;
#[cfg(windows)]
pub mod reader;
#[cfg(windows)]
pub mod writer;

pub use error::Error;
pub use ring_buffer::{PixelFormat, SharedFrameHeader, HEADER_SIZE, MAGIC, VERSION};

#[cfg(windows)]
pub use owner::SharedMemoryOwner;
#[cfg(windows)]
pub use reader::SharedMemoryReader;
#[cfg(windows)]
pub use writer::SharedMemoryWriter;

/// CLSID for the Cameras App virtual camera media source.
///
/// {7B2E3A1F-4D5C-4E8B-9A6F-1C2D3E4F5A6B}
///
/// Shared between the main app (which passes it to `MFCreateVirtualCamera`)
/// and the COM DLL (which uses it in `DllGetClassObject`).
pub const VCAM_SOURCE_CLSID: u128 = 0x7B2E_3A1F_4D5C_4E8B_9A6F_1C2D_3E4F_5A6B;

/// Well-known file path for shared memory IPC between app and COM DLL.
///
/// Both the app and COM DLL use this path to find each other. File paths
/// are universal — they work across all sessions and kernel object namespaces.
pub const SHARED_MEMORY_FILE_PATH: &str = r"C:\ProgramData\CamerasApp\vcam_shm_0.bin";

/// Well-known shared memory name — kept for tests using `Local\` kernel objects.
pub const SHARED_MEMORY_NAME: &str = r"Global\CamerasApp_VCam_0";

/// Default frame width when no shared memory is connected.
pub const DEFAULT_WIDTH: u32 = 1920;

/// Default frame height.
pub const DEFAULT_HEIGHT: u32 = 1080;

/// Target frame rate (frames per second).
pub const TARGET_FPS: u32 = 30;

#[cfg(all(test, windows))]
mod tests {
    use std::sync::atomic::Ordering;
    use std::thread;

    use super::*;

    /// Generate a unique shared memory name per test to avoid collisions.
    fn test_name(suffix: &str) -> String {
        format!(
            "Local\\VcamTest_{}_{}",
            suffix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    #[test]
    fn header_fields_are_correct() {
        let name = test_name("header");
        let writer = SharedMemoryWriter::new(&name, 640, 480, 3).unwrap();

        let reader = SharedMemoryReader::open(&name).unwrap();
        let header = reader.header();

        assert_eq!(header.magic, MAGIC);
        assert_eq!(header.version, VERSION);
        assert_eq!(header.width, 640);
        assert_eq!(header.height, 480);
        assert_eq!(header.format, PixelFormat::Nv12 as u32);
        assert_eq!(header.frame_size, 640 * 480 * 3 / 2);
        assert_eq!(header.slot_count, 3);
        assert_eq!(header.write_index.load(Ordering::Relaxed), 0);
        assert_eq!(header.sequence.load(Ordering::Relaxed), 0);

        drop(reader);
        drop(writer);
    }

    #[test]
    fn writer_reader_roundtrip() {
        let name = test_name("roundtrip");
        let width = 4;
        let height = 2;
        let frame_size = SharedFrameHeader::nv12_frame_size(width, height) as usize; // 12 bytes

        let writer = SharedMemoryWriter::new(&name, width, height, 3).unwrap();
        let reader = SharedMemoryReader::open(&name).unwrap();

        // Initially no frame available.
        assert!(reader.read_frame().is_none());

        // Write a frame with known pattern.
        let frame: Vec<u8> = (0..frame_size).map(|i| (i % 256) as u8).collect();
        writer.write_frame(&frame);

        // Read it back.
        let read = reader.read_frame().expect("frame should be available");
        assert_eq!(read, &frame[..]);

        drop(reader);
        drop(writer);
    }

    #[test]
    fn sequence_increases_monotonically() {
        let name = test_name("sequence");
        let width = 4;
        let height = 2;
        let frame_size = SharedFrameHeader::nv12_frame_size(width, height) as usize;

        let writer = SharedMemoryWriter::new(&name, width, height, 2).unwrap();

        assert_eq!(writer.sequence(), 0);

        let frame = vec![0xABu8; frame_size];
        for i in 1..=5u64 {
            writer.write_frame(&frame);
            assert_eq!(writer.sequence(), i);
        }

        drop(writer);
    }

    #[test]
    fn write_index_wraps_around() {
        let name = test_name("wrap");
        let width = 4;
        let height = 2;
        let slot_count = 3u32;
        let frame_size = SharedFrameHeader::nv12_frame_size(width, height) as usize;

        let writer = SharedMemoryWriter::new(&name, width, height, slot_count).unwrap();

        let frame = vec![0u8; frame_size];

        // Write slot_count + 2 frames to verify wrapping.
        let indices: Vec<u32> = (0..slot_count + 2)
            .map(|_| {
                writer.write_frame(&frame);
                writer.write_index()
            })
            .collect();

        // write_index after each write: 1, 2, 0, 1, 2
        // (stored as (prev_slot + 1) mod slot_count via wrapping store)
        // Actually: stored as slot+1 raw. Let's verify wrapping behaviour:
        // Write 1: slot=0, stored=1
        // Write 2: slot=1%3=1, stored=2
        // Write 3: slot=2%3=2, stored=3
        // Write 4: slot=3%3=0, stored=1
        // Write 5: slot=1%3=1, stored=2
        assert_eq!(indices, vec![1, 2, 3, 1, 2]);
        assert_eq!(writer.sequence(), 5);

        drop(writer);
    }

    #[test]
    fn file_backed_owner_reader_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_shm.bin");

        let owner = SharedMemoryOwner::new(&path, 4, 2, 3).unwrap();
        let reader = SharedMemoryReader::open_file(&path).unwrap();

        // Initially no frame.
        assert!(reader.read_frame().is_none());

        // Write via owner.
        let frame_size = SharedFrameHeader::nv12_frame_size(4, 2) as usize;
        let frame: Vec<u8> = (0..frame_size).map(|i| (i % 256) as u8).collect();
        owner.write_frame(&frame);

        // Read back.
        let read = reader.read_frame().expect("frame should be available");
        assert_eq!(read, &frame[..]);

        // Verify sequence.
        assert_eq!(owner.sequence(), 1);
        assert_eq!(reader.header().sequence.load(Ordering::Acquire), 1);

        drop(reader);
        drop(owner);
    }

    #[test]
    fn file_backed_owner_writes_and_reads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_owner.bin");

        let owner = SharedMemoryOwner::new(&path, 4, 2, 3).unwrap();

        // Initially no frame.
        assert!(owner.read_frame().is_none());

        let frame_size = SharedFrameHeader::nv12_frame_size(4, 2) as usize;
        let frame: Vec<u8> = (0..frame_size).map(|i| (i % 256) as u8).collect();
        owner.write_frame(&frame);

        // Owner can read back its own frame.
        let read = owner.read_frame().expect("frame should be available");
        assert_eq!(read, &frame[..]);

        drop(owner);
    }

    #[test]
    fn file_backed_open_nonexistent_path_returns_error() {
        let result =
            SharedMemoryReader::open_file(std::path::Path::new(r"C:\nonexistent\path\shm.bin"));
        assert!(result.is_err());
    }

    #[test]
    fn file_backed_owner_deletes_file_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_cleanup.bin");

        {
            let _owner = SharedMemoryOwner::new(&path, 4, 2, 3).unwrap();
            assert!(path.exists(), "file should exist while owner is alive");
        }

        assert!(!path.exists(), "file should be deleted after owner drops");
    }

    #[test]
    fn writer_reader_cross_thread() {
        let name = test_name("crossthread");
        let width = 4;
        let height = 2;
        let frame_size = SharedFrameHeader::nv12_frame_size(width, height) as usize;
        let name_clone = name.clone();

        let writer = SharedMemoryWriter::new(&name, width, height, 3).unwrap();

        let reader_thread = thread::spawn(move || {
            let reader = SharedMemoryReader::open(&name_clone).unwrap();

            // Wait for a frame with 5s timeout.
            assert!(reader.wait_frame(5000), "timed out waiting for frame");

            let frame = reader.read_frame().expect("frame should be available");
            assert_eq!(frame.len(), frame_size);
            // Verify the pattern.
            assert!(frame.iter().all(|&b| b == 0x42));

            drop(reader);
        });

        // Small delay to let reader thread start.
        thread::sleep(std::time::Duration::from_millis(50));

        let frame = vec![0x42u8; frame_size];
        writer.write_frame(&frame);

        reader_thread.join().unwrap();
        drop(writer);
    }
}
