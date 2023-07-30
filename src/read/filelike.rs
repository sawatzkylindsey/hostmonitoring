use axum::async_trait;
use std::io::SeekFrom;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum FileLikeError {
    #[error("Operation extends out of the file-like bounds.")]
    OutOfBounds,
    #[error("Error from the underlying filesystem: {0}")]
    IoError(String),
}

/// A 'file like' object.
/// Used to abstract file operations away from a literal `tokio::fs::File`/`std::fs::File`.
#[async_trait]
pub(crate) trait FileLike {
    /// Get the total number of bytes within the file.
    async fn length(&self) -> Result<usize, FileLikeError>;

    /// Seek to an arbitrary byte index within the file.
    /// The next call to `read` will start from this position.
    async fn seek(&mut self, offset: usize) -> Result<(), FileLikeError>;

    /// Read as many bytes from the file that fit into the buffer.
    /// Starts at the byte index established by the previous `seek`.
    /// The internal byte index is incremented by the number of bytes read.
    /// Returns the number of bytes read into buffer (may be less than the buffer size).
    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, FileLikeError>;
}

pub(crate) struct InMemoryFile {
    bytes: Vec<u8>,
    position: usize,
}

impl InMemoryFile {
    #[cfg(test)]
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, position: 0 }
    }
}

#[async_trait]
impl FileLike for InMemoryFile {
    async fn length(&self) -> Result<usize, FileLikeError> {
        Ok(self.bytes.len())
    }

    async fn seek(&mut self, offset: usize) -> Result<(), FileLikeError> {
        if offset > self.bytes.len() {
            Err(FileLikeError::OutOfBounds)
        } else {
            self.position = offset;
            Ok(())
        }
    }

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, FileLikeError> {
        let end = std::cmp::min(self.bytes.len(), self.position + buffer.len());
        let target = end - self.position;
        buffer[0..target].copy_from_slice(&self.bytes[self.position..end]);
        self.position += target;
        Ok(target)
    }
}

#[async_trait]
impl FileLike for tokio::fs::File {
    async fn length(&self) -> Result<usize, FileLikeError> {
        self.metadata()
            .await
            .map(|metadata| metadata.len() as usize)
            .map_err(|e| FileLikeError::IoError(e.to_string()))
    }

    async fn seek(&mut self, offset: usize) -> Result<(), FileLikeError> {
        AsyncSeekExt::seek(self, SeekFrom::Start(offset as u64))
            .await
            .map(|_| ())
            .map_err(|e| FileLikeError::IoError(e.to_string()))
    }

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, FileLikeError> {
        AsyncReadExt::read(self, buffer)
            .await
            .map_err(|e| FileLikeError::IoError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    use rstest::rstest;
    use rstest_reuse::{apply, template};
    use std::path::PathBuf;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;

    enum FileLikeType {
        InMemory,
        TokioFile,
    }

    // Convenience utility to instrument multiple tests with the same parameters.
    // We use this to reference test our different implementations of FileLike.
    #[template]
    #[rstest]
    #[case::in_memory(FileLikeType::InMemory)]
    #[case::tokio_file(FileLikeType::TokioFile)]
    fn file_like_implementation(#[case] flt: FileLikeType) {}

    async fn setup(flt: FileLikeType, bytes: Vec<u8>) -> Box<dyn FileLike> {
        match flt {
            FileLikeType::InMemory => Box::new(InMemoryFile { bytes, position: 0 }),
            FileLikeType::TokioFile => {
                let filename: String = thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(12)
                    .map(char::from)
                    .collect();
                let path = PathBuf::from(format!(".generated/{filename}"));
                let mut create_file = tokio::fs::File::create(path.clone()).await.unwrap();
                create_file.write(&bytes[..]).await.unwrap();
                create_file.flush().await.unwrap();
                Box::new(tokio::fs::File::open(path).await.unwrap())
            }
        }
    }

    #[apply(file_like_implementation)]
    #[tokio::test]
    async fn read(flt: FileLikeType) {
        // Setup
        let mut file = setup(flt, b"helloworld".to_vec()).await;
        let mut buffer = [0; 8];

        // Execute
        let result = file.read(&mut buffer).await.unwrap();

        // Verify
        assert_eq!(result, buffer.len());
        assert_eq!(&buffer, b"hellowor");

        // A second read moves ahead.
        let mut buffer = [0; 8];
        let result = file.read(&mut buffer).await.unwrap();
        let mut expected = b"ld".to_vec();
        (0..6).for_each(|_| expected.push(0));
        assert_eq!(result, 2);
        assert_eq!(buffer, expected.as_slice());
    }

    #[tokio::test]
    async fn in_memory_read_from_offset_inside() {
        // Setup
        let mut file = InMemoryFile {
            bytes: b"helloworld".to_vec(),
            position: 1,
        };
        let mut buffer = [0; 8];

        // Execute
        let result = file.read(&mut buffer).await.unwrap();

        // Verify
        assert_eq!(result, buffer.len());
        assert_eq!(&buffer, b"elloworl");
    }

    #[apply(file_like_implementation)]
    #[tokio::test]
    async fn seek_read(flt: FileLikeType) {
        // Setup
        let mut file = setup(flt, b"helloworld".to_vec()).await;
        let mut buffer = [0; 9];

        // Execute
        file.seek(1).await.unwrap();
        let result = file.read(&mut buffer).await.unwrap();

        // Verify
        assert_eq!(result, buffer.len());
        assert_eq!(&buffer, b"elloworld");
    }

    #[tokio::test]
    async fn in_memory_read_from_offset_beyond() {
        // Setup
        let mut file = InMemoryFile {
            bytes: b"helloworld".to_vec(),
            position: 2,
        };
        let mut buffer = [0; 9];

        // Execute
        let result = file.read(&mut buffer).await.unwrap();

        // Verify
        assert_eq!(result, 8);
        let mut expected = b"lloworld".to_vec();
        expected.push(0);
        assert_eq!(buffer, expected.as_slice());
    }

    #[apply(file_like_implementation)]
    #[tokio::test]
    async fn read_beyond(flt: FileLikeType) {
        // Setup
        let mut file = setup(flt, b"helloworld".to_vec()).await;
        let mut buffer = [0; 12];

        // Execute
        let result = file.read(&mut buffer).await.unwrap();

        // Verify
        assert_eq!(result, 10);
        let mut expected = b"helloworld".to_vec();
        expected.push(0);
        expected.push(0);
        assert_eq!(buffer, expected.as_slice());
    }

    #[tokio::test]
    async fn in_memory_seek() {
        // Setup
        let mut file = InMemoryFile {
            bytes: b"helloworld".to_vec(),
            position: 0,
        };

        // Execute
        file.seek(1).await.unwrap();

        // Verify
        assert_eq!(file.position, 1);
    }

    #[rstest]
    #[case(FileLikeType::InMemory, "", 0)]
    #[case(FileLikeType::InMemory, "hello", 5)]
    #[case(FileLikeType::InMemory, "hello👍", 9)]
    #[case(FileLikeType::InMemory, "你好", 6)]
    #[case(FileLikeType::TokioFile, "", 0)]
    #[case(FileLikeType::TokioFile, "hello", 5)]
    #[case(FileLikeType::TokioFile, "hello👍", 9)]
    #[case(FileLikeType::TokioFile, "你好", 6)]
    #[tokio::test]
    async fn length(#[case] flt: FileLikeType, #[case] value: &str, #[case] expected: usize) {
        // Setup
        let mut file = setup(flt, value.as_bytes().to_vec()).await;

        // Execute
        let result = file.length().await.unwrap();

        // Verify
        assert_eq!(result, expected);
        assert_eq!(result, value.as_bytes().len());
    }
}
