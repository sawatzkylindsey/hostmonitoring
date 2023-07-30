use axum::async_trait;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum FileLikeError {
    #[error("Operation extends out of the file-like bounds.")]
    OutOfBounds,
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
    /// Returns the number of bytes read (may be less than the buffer size).
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
        Ok(target)
    }
}

// TODO: Fill in this implementation.
#[async_trait]
impl FileLike for tokio::fs::File {
    async fn length(&self) -> Result<usize, FileLikeError> {
        todo!()
    }

    async fn seek(&mut self, offset: usize) -> Result<(), FileLikeError> {
        todo!()
    }

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, FileLikeError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[tokio::test]
    async fn in_memory_read() {
        // Setup
        let mut file = InMemoryFile {
            bytes: b"helloworld".to_vec(),
            position: 0,
        };
        let mut buffer = [0; 8];

        // Execute
        let result = file.read(&mut buffer).await.unwrap();

        // Verify
        assert_eq!(result, buffer.len());
        assert_eq!(&buffer, b"hellowor");
        // A second read produces the same result - we haven't moved via seek.
        let result = file.read(&mut buffer).await.unwrap();
        assert_eq!(result, buffer.len());
        assert_eq!(&buffer, b"hellowor");
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

    #[tokio::test]
    async fn in_memory_read_from_offset() {
        // Setup
        let mut file = InMemoryFile {
            bytes: b"helloworld".to_vec(),
            position: 1,
        };
        let mut buffer = [0; 9];

        // Execute
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

    #[tokio::test]
    async fn in_memory_read_beyond() {
        // Setup
        let mut file = InMemoryFile {
            bytes: b"helloworld".to_vec(),
            position: 0,
        };
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
            position: 1,
        };

        // Execute
        file.seek(1).await.unwrap();

        // Verify
        assert_eq!(file.position, 1);
    }

    #[rstest]
    #[case("", 0)]
    #[case("hello", 5)]
    #[case("hello👍", 9)]
    #[case("你好", 6)]
    #[tokio::test]
    async fn in_memory_length(#[case] value: &str, #[case] length: usize) {
        // Setup
        let file = InMemoryFile {
            bytes: value.as_bytes().to_vec(),
            position: 0,
        };

        // Execute
        let result = file.length().await.unwrap();

        // Verify
        assert_eq!(result, length);
        assert_eq!(result, value.as_bytes().len());
    }
}
