use std::io;

use serde::{de::DeserializeOwned, Serialize};

/// Write a length-prefixed JSON message to a writer.
/// Format: 4-byte big-endian length + JSON bytes
pub fn write_message<W: io::Write, T: Serialize>(writer: &mut W, msg: &T) -> io::Result<()> {
    let json = serde_json::to_vec(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let len = json.len() as u32;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(&json)?;
    writer.flush()?;
    Ok(())
}

/// Read a length-prefixed JSON message from a reader.
/// Returns None on EOF (zero-length read on the length prefix).
pub fn read_message<R: io::Read, T: DeserializeOwned>(reader: &mut R) -> io::Result<Option<T>> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > 16 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Message too large: {} bytes", len),
        ));
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;

    let msg =
        serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some(msg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_roundtrip() {
        let msg = "hello world".to_string();
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();

        let mut cursor = Cursor::new(buf);
        let result: Option<String> = read_message(&mut cursor).unwrap();
        assert_eq!(result, Some("hello world".to_string()));
    }

    #[test]
    fn test_eof_returns_none() {
        let mut cursor = Cursor::new(Vec::<u8>::new());
        let result: Option<String> = read_message(&mut cursor).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_multiple_messages() {
        let mut buf = Vec::new();
        write_message(&mut buf, &1u32).unwrap();
        write_message(&mut buf, &2u32).unwrap();
        write_message(&mut buf, &3u32).unwrap();

        let mut cursor = Cursor::new(buf);
        assert_eq!(read_message::<_, u32>(&mut cursor).unwrap(), Some(1));
        assert_eq!(read_message::<_, u32>(&mut cursor).unwrap(), Some(2));
        assert_eq!(read_message::<_, u32>(&mut cursor).unwrap(), Some(3));
        assert_eq!(read_message::<_, u32>(&mut cursor).unwrap(), None);
    }
}
