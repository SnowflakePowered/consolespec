//! Primitives every section is built from: LEB128 numbers, delta columns, and
//! front-coded string tables.

use crate::{Error, Result};

/// Marks an optional column slot as empty. Present values are stored as
/// `zigzag(delta) + 1`, so a repeated id — a zero delta — cannot be mistaken
/// for an absent one.
const ABSENT: u64 = 0;

#[cfg(any(feature = "compile", test))]
#[derive(Default)]
pub(crate) struct Writer {
    pub(crate) bytes: Vec<u8>,
}

#[cfg(any(feature = "compile", test))]
impl Writer {
    #[cfg(feature = "compile")]
    pub(crate) fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn uleb(&mut self, mut value: u64) {
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                self.bytes.push(byte);
                return;
            }
            self.bytes.push(byte | 0x80);
        }
    }

    pub(crate) fn raw(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    /// Writes strings as shared-prefix length, suffix length, then every
    /// suffix. Sorted input turns sibling paths and neighbouring firmware
    /// references into a handful of bytes each.
    pub(crate) fn strings<'a>(&mut self, values: impl IntoIterator<Item = &'a str>) {
        let mut previous: &[u8] = b"";
        let mut suffixes = Vec::new();
        for value in values {
            let value = value.as_bytes();
            let shared = value
                .iter()
                .zip(previous)
                .take_while(|(left, right)| left == right)
                .count();
            self.uleb(shared as u64);
            self.uleb((value.len() - shared) as u64);
            suffixes.extend_from_slice(&value[shared..]);
            previous = value;
        }
        self.raw(&suffixes);
    }
}

/// Accumulates one column of ids, delta-encoded against the previous value.
#[cfg(any(feature = "compile", test))]
#[derive(Default)]
pub(crate) struct Column {
    writer: Writer,
    previous: u64,
}

#[cfg(any(feature = "compile", test))]
impl Column {
    pub(crate) fn push(&mut self, id: Option<u64>) {
        match id {
            None => self.writer.uleb(ABSENT),
            Some(id) => {
                let value = id + 1;
                self.writer
                    .uleb(zigzag(value as i64 - self.previous as i64) + 1);
                self.previous = value;
            }
        }
    }

    /// Pushes a value that is always present, without the absent-slot bias.
    pub(crate) fn push_required(&mut self, id: u64) {
        self.writer.uleb(zigzag(id as i64 - self.previous as i64));
        self.previous = id;
    }

    /// Restarts the delta run without splitting the stream, so consecutive
    /// runs can share one buffer.
    #[cfg(feature = "compile")]
    pub(crate) fn reset(&mut self) {
        self.previous = 0;
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.writer.bytes
    }
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| Error::new("archive section ends mid-value"))?;
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    pub(crate) fn u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().expect("four bytes")))
    }

    pub(crate) fn count(&mut self) -> Result<usize> {
        usize::try_from(self.u32()?)
            .map_err(|_| Error::new("archive holds a table wider than this target"))
    }

    pub(crate) fn uleb(&mut self) -> Result<u64> {
        let mut value = 0u64;
        let mut shift = 0;
        loop {
            let byte = *self
                .bytes
                .get(self.offset)
                .ok_or_else(|| Error::new("archive section ends mid-value"))?;
            self.offset += 1;
            if shift >= 64 {
                return Err(Error::new("archive holds an over-long LEB128 value"));
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
    }

    pub(crate) fn usize(&mut self) -> Result<usize> {
        usize::try_from(self.uleb()?)
            .map_err(|_| Error::new("archive holds a length wider than this target"))
    }

    pub(crate) fn bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        self.take(len)
    }

    pub(crate) fn digests<const N: usize>(&mut self, count: usize) -> Result<Vec<[u8; N]>> {
        let len = count
            .checked_mul(N)
            .ok_or_else(|| Error::new("archive holds a digest table wider than this target"))?;
        Ok(self
            .take(len)?
            .chunks_exact(N)
            .map(|chunk| chunk.try_into().expect("chunk is N bytes"))
            .collect())
    }

    /// Reads the counterpart of [`Writer::strings`].
    pub(crate) fn strings(&mut self, count: usize) -> Result<Vec<String>> {
        let mut lengths = Vec::with_capacity(count);
        let mut total = 0usize;
        for _ in 0..count {
            let shared = self.usize()?;
            let suffix = self.usize()?;
            total = total
                .checked_add(suffix)
                .ok_or_else(|| Error::new("archive holds an oversized string table"))?;
            lengths.push((shared, suffix));
        }
        let suffixes = self.take(total)?;
        let mut values: Vec<String> = Vec::with_capacity(count);
        let mut cursor = 0;
        for (shared, suffix) in lengths {
            let previous = values.last().map(String::as_bytes).unwrap_or(b"");
            if shared > previous.len() {
                return Err(Error::new(
                    "archive string shares more of its prefix than exists",
                ));
            }
            let mut bytes = previous[..shared].to_vec();
            bytes.extend_from_slice(&suffixes[cursor..cursor + suffix]);
            cursor += suffix;
            values.push(
                String::from_utf8(bytes)
                    .map_err(|_| Error::new("archive string is not valid UTF-8"))?,
            );
        }
        Ok(values)
    }
}

/// Reads back one column written by [`Column`].
///
/// Columns are concatenated without lengths — LEB128 makes their byte size
/// data-dependent — so this borrows the section reader and leaves it pointing
/// at the next column.
pub(crate) struct ColumnReader<'a, 'r> {
    reader: &'r mut Reader<'a>,
    previous: u64,
}

impl<'a, 'r> ColumnReader<'a, 'r> {
    pub(crate) fn new(reader: &'r mut Reader<'a>) -> Self {
        Self {
            reader,
            previous: 0,
        }
    }

    pub(crate) fn next(&mut self) -> Result<Option<u64>> {
        let raw = self.reader.uleb()?;
        if raw == ABSENT {
            return Ok(None);
        }
        Ok(Some(self.step(raw - 1)? - 1))
    }

    pub(crate) fn next_required(&mut self) -> Result<u64> {
        let raw = self.reader.uleb()?;
        self.step(raw)
    }

    /// Mirrors [`Column::reset`].
    pub(crate) fn reset(&mut self) {
        self.previous = 0;
    }

    fn step(&mut self, raw: u64) -> Result<u64> {
        let value = self
            .previous
            .checked_add_signed(unzigzag(raw))
            .ok_or_else(|| Error::new("archive column delta leaves the table"))?;
        self.previous = value;
        Ok(value)
    }
}

#[cfg(any(feature = "compile", test))]
pub(crate) fn zigzag(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

pub(crate) fn unzigzag(value: u64) -> i64 {
    ((value >> 1) as i64) ^ -((value & 1) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_round_trip_including_repeats_and_gaps() {
        let values = [Some(0), Some(7), Some(7), None, Some(3), None, Some(9)];
        let mut column = Column::default();
        for value in values {
            column.push(value);
        }
        let bytes = column.finish();
        let mut reader = Reader::new(&bytes);
        let mut column = ColumnReader::new(&mut reader);
        for value in values {
            assert_eq!(column.next().unwrap(), value);
        }
    }

    #[test]
    fn required_columns_round_trip() {
        let values = [0, 4, 4, 9, 2, 2, 300];
        let mut column = Column::default();
        for value in values {
            column.push_required(value);
        }
        let bytes = column.finish();
        let mut reader = Reader::new(&bytes);
        let mut column = ColumnReader::new(&mut reader);
        for value in values {
            assert_eq!(column.next_required().unwrap(), value);
        }
    }

    #[test]
    fn front_coded_strings_round_trip() {
        let values = [".", "./app", "./app/NPXS10000", "./bin", "./bin/x"];
        let mut writer = Writer::default();
        writer.strings(values);
        let mut reader = Reader::new(&writer.bytes);
        assert_eq!(reader.strings(values.len()).unwrap(), values);
    }

    #[test]
    fn zigzag_round_trips_across_the_sign() {
        for value in [i64::MIN, -1, 0, 1, i64::MAX] {
            assert_eq!(unzigzag(zigzag(value)), value);
        }
    }
}
