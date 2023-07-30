use crate::read::filelike::FileLike;

/// The number of bytes to read off the file.
/// 1024 selected arbitrarily (this may possibly be optimized, especially based off underlying system/hardware).
const BUFFER_LENGTH: usize = 1024;

/// Read the file-like from the bottom to top (aka: in reverse).
///
/// Reads the bytes of the file in reverse, using utf-8 to decode them into lines.
/// Lines are separated by the new line (\n) delimiter, and whitespace is trimmed off each line.
/// The final result is a `Vec<String>` of lines, starting with the last line of the file, and ending with the first line of the file.
///
/// # Panic
/// This method panics if it encounters an invalid utf-8 sequence.
// TODO: Change this method to stream the results rather than hold them all in memory.
pub(crate) async fn reverse_read<F: FileLike>(file: &mut F) -> Vec<String> {
    let mut contents = Vec::default();
    let mut buffer = [0; BUFFER_LENGTH as usize];
    let mut suffix: Option<Vec<u8>> = None;
    let size = file.length().await.expect("file must provide length");
    let mut offset = size;

    loop {
        let previous_offset = offset;
        offset = offset.saturating_sub(BUFFER_LENGTH);
        file.seek(offset).await.expect("invalid seek");
        let read_bytes = file
            .read(&mut buffer[0..previous_offset - offset])
            .await
            .expect("invalid read");
        let mut chunk_end = read_bytes;

        loop {
            let (remaining, line) = match suffix.as_ref() {
                Some(value) => decode(&buffer[0..chunk_end], value.as_slice()),
                None => decode(&buffer[0..chunk_end], &[]),
            };

            match line {
                // When a line was decoded.
                Some(line) => {
                    suffix = None;
                    contents.push(line);
                    chunk_end = remaining;
                }
                // When a line couldn't be decoded (there was no line break to split on).
                None => {
                    // If a line couldn't be decoded because we used up the whole buffer (precisely).
                    if remaining == 0 {
                        break;
                    } else {
                        // Otherwise, we need to build out the suffix.
                        match suffix {
                            // When there already is a suffix, prepend to it.
                            // We allocate a Vec every time this happens.
                            // TODO: Optimize: In the case of multi-buffer lines, we may be able to improve performance (using only 1 Vec allocation at the end).
                            Some(ref mut value) => {
                                let mut new_suffix = buffer[0..chunk_end].to_vec();
                                new_suffix.append(value);
                                suffix = Some(new_suffix);
                            }
                            // When there isn't already a suffix, create it.
                            None => {
                                suffix.replace(buffer[0..chunk_end].to_vec());
                            }
                        }

                        // If no chunks could be decoded off the buffer.
                        if remaining == chunk_end {
                            break;
                        }
                    }
                }
            }
        }

        // If we've hit the start of the file.
        if offset == 0 {
            break;
        }
    }

    // Process whatever is left in the suffix (aka: the first 'line' of the file).
    if let Some(value) = suffix {
        let line = std::str::from_utf8(&value[..]).expect("log bytes must decode utf8");
        contents.push(line.to_string());
    }

    contents
}

/// Utf-8 decode the "right side" of chunk of bytes with its suffix.
/// Private method intended for use directly within `reverse_read`.
///
/// This method attempts to find the right side of a chunk of bytes, based off its right-most line break (\n).
/// If found, it decodes the right side as a String using utf-8.
/// Additionally, if a non-empty suffix is provided, the suffix is combined into the decoding.
/// When decoding succeeds (in any of the above variants), then the decoded String is returned as well as the size of the remaining "left side" of the chunk of bytes.
///
/// If the bytes cannot be split into a left/right side based off a line break, then no String is returned (`None`).
///
/// # Panic
/// This method panics if it encounters an invalid utf-8 sequence.
///
/// ### Examples
/// ```no-run
/// let (remaining, line) = decode(b"hello\nworld", &[]);
/// assert_eq!(remaining, 5);
/// assert_eq!(line, Some("world".to_string()));
///
/// let (remaining, line) = decode(b"hello\nworld", b"suffix");
/// assert_eq!(remaining, 5);
/// assert_eq!(line, Some("worldsuffix".to_string()));
///
/// let (remaining, line) = decode(b"helloworld", &[]);
/// assert_eq!(remaining, 10);
/// assert_eq!(line, None);
/// ```
fn decode(bytes: &[u8], suffix: &[u8]) -> (usize, Option<String>) {
    let bytes_length = bytes.len();
    let mut iter = bytes.rsplit(|c| c == &0xA);
    let right = iter.next().expect("split must have next()");

    match iter.next() {
        // If there is a left side, then it means we successfully split across a line break.
        Some(_left) => {
            let part_length = right.len();

            // If there is not suffix, we can prevent an unnecessary Vec allocation.
            if suffix.is_empty() {
                let line = std::str::from_utf8(&right[..]).expect("log bytes must decode utf8");
                (
                    bytes_length.saturating_sub(part_length + 1),
                    Some(line.trim().to_string()),
                )
            } else {
                // We need to create a new Vec to assemble part with the suffix.
                let target: Vec<u8> = right.iter().chain(suffix.iter()).copied().collect();
                let line = std::str::from_utf8(&target[..]).expect("log bytes must decode utf8");
                (
                    bytes_length.saturating_sub(part_length + 1),
                    Some(line.trim().to_string()),
                )
            }
        }
        // If there isn't a left side, then we can't decode anything yet.
        None => (bytes_length, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::filelike::InMemoryFile;
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    use rstest::rstest;

    #[test]
    fn decode_non_utf8() {
        let (remaining, line) = decode(&[0xA, 0xED, 0x9F, 0xBF], &[]);
        assert_eq!(remaining, 0);
        // It "decodes", but it looks like a junk character.
        println!("{}", line.unwrap());
    }

    #[test]
    #[should_panic]
    fn decode_invalid_utf8() {
        decode(&[0xA, 0x80], &[]);
    }

    #[test]
    fn decode_bytes_empty() {
        let (remaining, line) = decode(&[], &[]);
        assert_eq!(remaining, 0);
        assert_eq!(line, None);
    }

    #[test]
    fn decode_bytes() {
        let bytes = b"hello\nworld";
        let (remaining, line) = decode(bytes, &[]);
        assert_eq!(remaining, 5);
        assert_eq!(line, Some("world".to_string()));

        let bytes = b"\nhelloworld";
        let (remaining, line) = decode(bytes, &[]);
        assert_eq!(remaining, 0);
        assert_eq!(line, Some("helloworld".to_string()));

        let bytes = b"helloworld\n";
        let (remaining, line) = decode(bytes, &[]);
        assert_eq!(remaining, 10);
        assert_eq!(line, Some("".to_string()));
    }

    #[test]
    fn decode_bytes_with_suffix() {
        let bytes = b"hello\nworld";
        let (remaining, line) = decode(bytes, b"suffix");
        assert_eq!(remaining, 5);
        assert_eq!(line, Some("worldsuffix".to_string()));

        let bytes = b"\nhelloworld";
        let (remaining, line) = decode(bytes, b"suffix");
        assert_eq!(remaining, 0);
        assert_eq!(line, Some("helloworldsuffix".to_string()));

        let bytes = b"helloworld\n";
        let (remaining, line) = decode(bytes, b"suffix");
        assert_eq!(remaining, 10);
        assert_eq!(line, Some("suffix".to_string()));
    }

    #[test]
    fn decode_bytes_without_newline() {
        let bytes = b"helloworld";
        let (remaining, line) = decode(bytes, &[]);
        assert_eq!(remaining, 10);
        assert_eq!(line, None);

        let bytes = b"helloworld";
        let (remaining, line) = decode(bytes, b"suffix");
        assert_eq!(remaining, 10);
        assert_eq!(line, None);
    }

    #[test]
    fn decode_bytes_carriage_return() {
        let bytes = b"hello\r\r\nworld";
        let (remaining, line) = decode(bytes, &[]);
        assert_eq!(remaining, 7);
        assert_eq!(line, Some("world".to_string()));

        let bytes = b"\r\r\nhelloworld";
        let (remaining, line) = decode(bytes, &[]);
        assert_eq!(remaining, 2);
        assert_eq!(line, Some("helloworld".to_string()));

        let bytes = b"helloworld\r\r\n";
        let (remaining, line) = decode(bytes, &[]);
        assert_eq!(remaining, 12);
        assert_eq!(line, Some("".to_string()));

        let bytes = b"hello\rworld";
        let (remaining, line) = decode(bytes, &[]);
        assert_eq!(remaining, 11);
        assert_eq!(line, None);
    }

    #[test]
    fn decode_bytes_whitespace() {
        let bytes = b"hello\n world ";
        let (remaining, line) = decode(bytes, &[]);
        assert_eq!(remaining, 5);
        assert_eq!(line, Some("world".to_string()));

        let bytes = b"helloworld\n ";
        let (remaining, line) = decode(bytes, &[]);
        assert_eq!(remaining, 10);
        assert_eq!(line, Some("".to_string()));

        let bytes = b"hello\n world\r ";
        let (remaining, line) = decode(bytes, &[]);
        assert_eq!(remaining, 5);
        assert_eq!(line, Some("world".to_string()));

        let bytes = b"helloworld\n\r ";
        let (remaining, line) = decode(bytes, &[]);
        assert_eq!(remaining, 10);
        assert_eq!(line, Some("".to_string()));
    }

    #[test]
    fn decode_bytes_multiple_newline() {
        let bytes = b"hello\n\n\nworld";
        let (remaining, line) = decode(bytes, &[]);
        assert_eq!(remaining, 7);
        assert_eq!(line, Some("world".to_string()));

        let (remaining, line) = decode(&bytes[0..remaining], &[]);
        assert_eq!(remaining, 6);
        assert_eq!(line, Some("".to_string()));

        let (remaining, line) = decode(&bytes[0..remaining], &[]);
        assert_eq!(remaining, 5);
        assert_eq!(line, Some("".to_string()));

        let (remaining, line) = decode(&bytes[0..remaining], &[]);
        assert_eq!(remaining, 5);
        assert_eq!(line, None);
    }

    #[test]
    fn decode_bytes_unicode() {
        let bytes = "你好👍\n你好👍".as_bytes();
        let (remaining, line) = decode(bytes, &[]);
        assert_eq!(remaining, 10);
        assert_eq!(line, Some("你好👍".to_string()));
    }

    #[test]
    fn decode_bytes_spanning_unicode() {
        let (remaining, line) = decode(&[0xA, 0xE4], &[0xBD, 0xA0]);
        assert_eq!(remaining, 0);
        assert_eq!(line, Some("你".to_string()));

        let (remaining, line) = decode(&[0xA, 0xE4, 0xBD], &[0xA0]);
        assert_eq!(remaining, 0);
        assert_eq!(line, Some("你".to_string()));
    }

    #[tokio::test]
    async fn reverse_empty() {
        // Setup
        let mut file = InMemoryFile::new(Vec::default());

        // Execute
        let result = reverse_read(&mut file).await;

        // Verify
        let empty: Vec<String> = Vec::default();
        assert_eq!(result, empty);
    }

    #[rstest]
    #[case("helloworld")]
    #[case("\nhelloworld")]
    #[case("\n\rhelloworld")]
    #[case("\nhelloworld\r")]
    #[case("\n helloworld ")]
    #[tokio::test]
    async fn reverse(#[case] input: &str) {
        // Setup
        let mut file = InMemoryFile::new(input.as_bytes().to_vec());

        // Execute
        let result = reverse_read(&mut file).await;

        // Verify
        assert_eq!(result, vec!["helloworld".to_string()]);
    }

    #[tokio::test]
    async fn reverse_multi_line() {
        // Setup
        let mut file = InMemoryFile::new(b"\nhelloworld\nabc\n".to_vec());

        // Execute
        let result = reverse_read(&mut file).await;

        // Verify
        assert_eq!(
            result,
            vec!["".to_string(), "abc".to_string(), "helloworld".to_string(),]
        );
    }

    #[tokio::test]
    async fn reverse_full_buffer() {
        // Setup
        let content: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(BUFFER_LENGTH)
            .map(char::from)
            .collect();
        let mut file = InMemoryFile::new(content.as_bytes().to_vec());

        // Execute
        let result = reverse_read(&mut file).await;

        // Verify
        assert_eq!(result, vec![content]);
    }

    #[tokio::test]
    async fn reverse_beyond_buffer() {
        // Setup
        let content: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(BUFFER_LENGTH + 1)
            .map(char::from)
            .collect();
        let mut file = InMemoryFile::new(content.as_bytes().to_vec());

        // Execute
        let result = reverse_read(&mut file).await;

        // Verify
        assert_eq!(result, vec![content]);
    }

    #[tokio::test]
    async fn reverse_multiple_beyond_buffer() {
        // Setup
        let content: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(BUFFER_LENGTH * 3)
            .map(char::from)
            .collect();
        let mut file = InMemoryFile::new(content.as_bytes().to_vec());

        // Execute
        let result = reverse_read(&mut file).await;

        // Verify
        assert_eq!(result, vec![content]);
    }
}
