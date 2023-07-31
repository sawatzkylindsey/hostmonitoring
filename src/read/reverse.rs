use crate::read::filelike::FileLike;
use futures::Stream;
use serde::{ser, Serialize, Serializer};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc::{Receiver, Sender};

/// The number of bytes to read off the file.
/// 1024 selected arbitrarily (this may possibly be optimized, especially based off underlying system/hardware).
const BUFFER_LENGTH: usize = 1024;

/// Read the file-like from the bottom to top (aka: in reverse), sending items onto the provided channel as they become available.
///
/// Reads the bytes of the file in reverse, using utf-8 to decode them into lines.
/// Lines are separated by the new line (\n) delimiter, and whitespace is trimmed off each line.
/// As lines are processed they are sent onto the channel via `LineResult::ok`.
///
/// If an invalid utf-8 sequence is encountered (see `std::str::from_utf8`) then an error is put on the channel via `LineResult::error`, and the method execution is finished.
pub(crate) async fn reverse_read_runner<F: FileLike>(sender: Sender<LineResult>, mut file: Box<F>) {
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
            let result = match suffix.as_ref() {
                Some(value) => decode(&buffer[0..chunk_end], value.as_slice()),
                None => decode(&buffer[0..chunk_end], &[]),
            };

            let Ok((remaining, line)) = result else {
                // When decode encounters an invalid utf-8 sequence.
                // Send an error on the channel so the receiver can handle this.
                sender.send(LineResult::error()).await.expect("log channel must still be open");
                // And stop from any further processing.
                return;
            };

            // When the decoding succeeded (didn't encounter an invalid utf-8 sequence).
            match line {
                // When a line was decoded.
                Some(line) => {
                    suffix = None;
                    chunk_end = remaining;

                    if sender.send(LineResult::ok(line)).await.is_err() {
                        // The log channel is closed.
                        // Presumably, the client has hung up on us.
                        // In either case, we can't do anything more.
                        return;
                    }
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
        if sender.send(LineResult::ok(line.to_string())).await.is_err() {
            // The log channel is closed.
            // Presumably, the client has hung up on us.
            // In either case, we can't do anything more.
            return;
        }
    }
}

/// A struct that turns the receiver side of a channel into a `Stream`.
pub(crate) struct ChannelReceiverStream {
    receiver: Receiver<LineResult>,
}

impl ChannelReceiverStream {
    pub(crate) fn new(receiver: Receiver<LineResult>) -> Self {
        Self { receiver }
    }
}

impl Stream for ChannelReceiverStream {
    type Item = LineResult;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Defer to the channel receiver's poll_receive [sic].
        // This simply means items are streamed out as they are put onto the channel.
        self.receiver.poll_recv(cx)
    }
}

/// A small wrapper around a `Result<String, ()>`.
/// Used so we can implement a serializer that errors when an invalid utf-8 sequence is encountered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineResult(Result<String, ()>);

impl LineResult {
    pub fn ok(value: String) -> Self {
        Self(Ok(value))
    }

    fn error() -> Self {
        Self(Err(()))
    }
}

impl Serialize for LineResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Our special serializer implementation.
        match &self.0 {
            // Serialize like normal when we encounter a `LineResult::ok`.
            Ok(value) => serializer.serialize_str(value),
            // Return a serialization error when we encounter a `LineResult::error`
            Err(_) => Err(ser::Error::custom("invalid utf-8")),
        }
    }
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
/// Returns an error if an invalid utf-8 sequence is encountered (see `std::str::from_utf8`).
///
/// ### Examples
/// ```ignore
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
fn decode(bytes: &[u8], suffix: &[u8]) -> Result<(usize, Option<String>), ()> {
    let bytes_length = bytes.len();
    let mut iter = bytes.rsplit(|c| c == &0xA);
    let right = iter.next().expect("split must have next()");

    match iter.next() {
        // If there is a left side, then it means we successfully split across a line break.
        Some(_left) => {
            let part_length = right.len();

            // If there is not suffix, we can prevent an unnecessary Vec allocation.
            if suffix.is_empty() {
                let line = std::str::from_utf8(&right[..]).map_err(|_| ())?;
                Ok((
                    bytes_length.saturating_sub(part_length + 1),
                    Some(line.trim().to_string()),
                ))
            } else {
                // We need to create a new Vec to assemble part with the suffix.
                let target: Vec<u8> = right.iter().chain(suffix.iter()).copied().collect();
                let line = std::str::from_utf8(&target[..]).map_err(|_| ())?;
                Ok((
                    bytes_length.saturating_sub(part_length + 1),
                    Some(line.trim().to_string()),
                ))
            }
        }
        // If there isn't a left side, then we can't decode anything yet.
        None => Ok((bytes_length, None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::filelike::InMemoryFile;
    use futures::future::join_all;
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    use rstest::rstest;
    use tokio::sync::mpsc::channel;

    #[test]
    fn decode_non_utf8() {
        let (remaining, line) = decode(&[0xA, 0xED, 0x9F, 0xBF], &[]).unwrap();
        assert_eq!(remaining, 0);
        // It "decodes", but it looks like a junk character.
        println!("{}", line.unwrap());
    }

    #[test]
    fn decode_invalid_utf8() {
        decode(&[0xA, 0x80], &[]).unwrap_err();
    }

    #[test]
    fn decode_bytes_empty() {
        let (remaining, line) = decode(&[], &[]).unwrap();
        assert_eq!(remaining, 0);
        assert_eq!(line, None);
    }

    #[test]
    fn decode_bytes() {
        let bytes = b"hello\nworld";
        let (remaining, line) = decode(bytes, &[]).unwrap();
        assert_eq!(remaining, 5);
        assert_eq!(line, Some("world".to_string()));

        let bytes = b"\nhelloworld";
        let (remaining, line) = decode(bytes, &[]).unwrap();
        assert_eq!(remaining, 0);
        assert_eq!(line, Some("helloworld".to_string()));

        let bytes = b"helloworld\n";
        let (remaining, line) = decode(bytes, &[]).unwrap();
        assert_eq!(remaining, 10);
        assert_eq!(line, Some("".to_string()));
    }

    #[test]
    fn decode_bytes_with_suffix() {
        let bytes = b"hello\nworld";
        let (remaining, line) = decode(bytes, b"suffix").unwrap();
        assert_eq!(remaining, 5);
        assert_eq!(line, Some("worldsuffix".to_string()));

        let bytes = b"\nhelloworld";
        let (remaining, line) = decode(bytes, b"suffix").unwrap();
        assert_eq!(remaining, 0);
        assert_eq!(line, Some("helloworldsuffix".to_string()));

        let bytes = b"helloworld\n";
        let (remaining, line) = decode(bytes, b"suffix").unwrap();
        assert_eq!(remaining, 10);
        assert_eq!(line, Some("suffix".to_string()));
    }

    #[test]
    fn decode_bytes_without_newline() {
        let bytes = b"helloworld";
        let (remaining, line) = decode(bytes, &[]).unwrap();
        assert_eq!(remaining, 10);
        assert_eq!(line, None);

        let bytes = b"helloworld";
        let (remaining, line) = decode(bytes, b"suffix").unwrap();
        assert_eq!(remaining, 10);
        assert_eq!(line, None);
    }

    #[test]
    fn decode_bytes_carriage_return() {
        let bytes = b"hello\r\r\nworld";
        let (remaining, line) = decode(bytes, &[]).unwrap();
        assert_eq!(remaining, 7);
        assert_eq!(line, Some("world".to_string()));

        let bytes = b"\r\r\nhelloworld";
        let (remaining, line) = decode(bytes, &[]).unwrap();
        assert_eq!(remaining, 2);
        assert_eq!(line, Some("helloworld".to_string()));

        let bytes = b"helloworld\r\r\n";
        let (remaining, line) = decode(bytes, &[]).unwrap();
        assert_eq!(remaining, 12);
        assert_eq!(line, Some("".to_string()));

        let bytes = b"hello\rworld";
        let (remaining, line) = decode(bytes, &[]).unwrap();
        assert_eq!(remaining, 11);
        assert_eq!(line, None);
    }

    #[test]
    fn decode_bytes_whitespace() {
        let bytes = b"hello\n world ";
        let (remaining, line) = decode(bytes, &[]).unwrap();
        assert_eq!(remaining, 5);
        assert_eq!(line, Some("world".to_string()));

        let bytes = b"helloworld\n ";
        let (remaining, line) = decode(bytes, &[]).unwrap();
        assert_eq!(remaining, 10);
        assert_eq!(line, Some("".to_string()));

        let bytes = b"hello\n world\r ";
        let (remaining, line) = decode(bytes, &[]).unwrap();
        assert_eq!(remaining, 5);
        assert_eq!(line, Some("world".to_string()));

        let bytes = b"helloworld\n\r ";
        let (remaining, line) = decode(bytes, &[]).unwrap();
        assert_eq!(remaining, 10);
        assert_eq!(line, Some("".to_string()));
    }

    #[test]
    fn decode_bytes_multiple_newline() {
        let bytes = b"hello\n\n\nworld";
        let (remaining, line) = decode(bytes, &[]).unwrap();
        assert_eq!(remaining, 7);
        assert_eq!(line, Some("world".to_string()));

        let (remaining, line) = decode(&bytes[0..remaining], &[]).unwrap();
        assert_eq!(remaining, 6);
        assert_eq!(line, Some("".to_string()));

        let (remaining, line) = decode(&bytes[0..remaining], &[]).unwrap();
        assert_eq!(remaining, 5);
        assert_eq!(line, Some("".to_string()));

        let (remaining, line) = decode(&bytes[0..remaining], &[]).unwrap();
        assert_eq!(remaining, 5);
        assert_eq!(line, None);
    }

    #[test]
    fn decode_bytes_unicode() {
        let bytes = "你好👍\n你好👍".as_bytes();
        let (remaining, line) = decode(bytes, &[]).unwrap();
        assert_eq!(remaining, 10);
        assert_eq!(line, Some("你好👍".to_string()));
    }

    #[test]
    fn decode_bytes_spanning_unicode() {
        let (remaining, line) = decode(&[0xA, 0xE4], &[0xBD, 0xA0]).unwrap();
        assert_eq!(remaining, 0);
        assert_eq!(line, Some("你".to_string()));

        let (remaining, line) = decode(&[0xA, 0xE4, 0xBD], &[0xA0]).unwrap();
        assert_eq!(remaining, 0);
        assert_eq!(line, Some("你".to_string()));
    }

    #[tokio::test]
    async fn reverse_empty() {
        // Setup
        let (sender, mut receiver) = channel::<LineResult>(1);
        let file = InMemoryFile::new(Vec::default());

        // Execute
        tokio::spawn(reverse_read_runner(sender, Box::new(file)))
            .await
            .unwrap();

        // Verify
        assert!(receiver.recv().await.is_none());
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
        let (sender, mut receiver) = channel::<LineResult>(1);
        let file = InMemoryFile::new(input.as_bytes().to_vec());

        // Execute
        tokio::spawn(reverse_read_runner(sender, Box::new(file)))
            .await
            .unwrap();

        // Verify
        assert_eq!(
            receiver.recv().await.unwrap(),
            LineResult::ok("helloworld".to_string())
        );
        assert!(receiver.recv().await.is_none());
    }

    #[tokio::test]
    async fn reverse_multi_line() {
        // Setup
        // Use a singly bounded channel to test our multi-threading.
        let (sender, mut receiver) = channel::<LineResult>(1);
        let file = InMemoryFile::new(b"\nhelloworld\nabc\n".to_vec());

        // Execute
        let execute = tokio::spawn(reverse_read_runner(sender, Box::new(file)));

        // Verify
        let verify = tokio::spawn(async move {
            assert_eq!(
                receiver.recv().await.unwrap(),
                LineResult::ok("".to_string())
            );
            assert_eq!(
                receiver.recv().await.unwrap(),
                LineResult::ok("abc".to_string())
            );
            assert_eq!(
                receiver.recv().await.unwrap(),
                LineResult::ok("helloworld".to_string())
            );
            assert!(receiver.recv().await.is_none());
        });
        join_all(vec![execute, verify]).await;
    }

    #[tokio::test]
    async fn reverse_full_buffer() {
        // Setup
        let content: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(BUFFER_LENGTH)
            .map(char::from)
            .collect();
        let (sender, mut receiver) = channel::<LineResult>(1);
        let file = InMemoryFile::new(content.as_bytes().to_vec());

        // Execute
        tokio::spawn(reverse_read_runner(sender, Box::new(file)))
            .await
            .unwrap();

        // Verify
        assert_eq!(receiver.recv().await.unwrap(), LineResult::ok(content));
        assert!(receiver.recv().await.is_none());
    }

    #[tokio::test]
    async fn reverse_beyond_buffer() {
        // Setup
        let content: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(BUFFER_LENGTH + 1)
            .map(char::from)
            .collect();
        let (sender, mut receiver) = channel::<LineResult>(1);
        let file = InMemoryFile::new(content.as_bytes().to_vec());

        // Execute
        tokio::spawn(reverse_read_runner(sender, Box::new(file)))
            .await
            .unwrap();

        // Verify
        assert_eq!(receiver.recv().await.unwrap(), LineResult::ok(content));
        assert!(receiver.recv().await.is_none());
    }

    #[tokio::test]
    async fn reverse_multiple_beyond_buffer() {
        // Setup
        let content: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(BUFFER_LENGTH * 3)
            .map(char::from)
            .collect();
        let (sender, mut receiver) = channel::<LineResult>(1);
        let file = InMemoryFile::new(content.as_bytes().to_vec());

        // Execute
        tokio::spawn(reverse_read_runner(sender, Box::new(file)))
            .await
            .unwrap();

        // Verify
        assert_eq!(receiver.recv().await.unwrap(), LineResult::ok(content));
        assert!(receiver.recv().await.is_none());
    }
}
