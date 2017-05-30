use std::io::prelude::*;
use std::io::SeekFrom;
use std::fs::{File, OpenOptions};
use std::path::Path;

use bincode::{self, Infinite};
use flate2::write::ZlibEncoder;
use flate2::read::ZlibDecoder;
use flate2::Compression;

use region::*;
use traits::{ManagedChunk, Index};

/// Pads the given byte vec with zeroes to the next multiple of the given sector
/// size.
fn pad_byte_vec(bytes: &mut Vec<u8>, size: usize) {
    for _ in 0..(size - (bytes.len() % size)) {
        bytes.push(0);
    }
}

fn serialize_u32(val: u32) -> [u8; 4] {
    let bits = u32::from_be(val);
    [(bits >> 24) as u8, (bits >> 16) as u8, (bits >> 8) as u8, bits as u8]
}

fn deserialize_u32(buf: &[u8]) -> u32 {
    (((buf[0] as u32) << 24) |
     ((buf[1] as u32) << 16) |
     ((buf[2] as u32) <<  8) |
     ((buf[3] as u32) <<  0)).to_be()
}

fn compress_data(bytes: &Vec<u8>) -> SerialResult<Vec<u8>> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::Default);
    e.write(bytes.as_slice())?;
    let buf = e.finish().map_err(SerialError::from)?;

    let size: u32 = buf.len() as u32;
    let mut header = serialize_u32(size).to_vec();
    header.extend(buf.as_slice());

    Ok(header)
}

fn decompress_data(bytes: &Vec<u8>) -> SerialResult<Vec<u8>> {
    let (header, _) = bytes.split_at(4);
    let data_length = deserialize_u32(header) as usize;

    let mut d = ZlibDecoder::new(&bytes[4..4 + data_length]);
    let mut buf = Vec::new();
    d.read_to_end(&mut buf).map_err(SerialError::from)?;

    Ok(buf)
}

/// Describes a struct responsible for saving and loading a set of chunks in an
/// area of infinite terrain.
///
/// The main ideas for this system were taken from [this
/// post](https://www.reddit.com/r/gamedev/comments/1s63cn/creating_a_region_file_system_for_a_voxel_game/).
///
/// The idea behind regions is instead of saving each piece of terrain as a
/// separate file, they can be grouped together as a single unit that manages a
/// certain area. This reduces the number of open file handles and allows the
/// file to remain open as large parts of terrain are saved to disk.
///
/// Information about the size and offset of the chunk data is stored as a
/// lookup table at the start of each region file. The lookup table indexes
/// 16-bit integers, where the low 8 bits count the number of sectors the data
/// occupies and the high 8 bits provide the offset in sectors from the end of
/// the lookup table in the file. Both indices are currently limited to 255.
/// Data is aligned to a specified number of bytes, the sector size, for better
/// performance and easier encoding of offsets and sizes.
pub trait ManagedRegion<'a, I, C>
    where I: Index,
          C: ManagedChunk {

    fn chunk_unsaved(&self, index: &I) -> bool;
    fn mark_as_saved(&mut self, index: &I);
    fn mark_as_unsaved(&mut self, index: &I);
    fn handle(&mut self) -> &mut File;

    fn lookup_table_size() -> u64 { (C::REGION_WIDTH * C::REGION_WIDTH) as u64 * 2 }

    fn create_lookup_table_entry(&self, eof: u64, sector_count: u8) -> [u8; 2] {
        let offset: u8 = ((eof - Self::lookup_table_size()) / C::SECTOR_SIZE as u64) as u8;

        [offset, sector_count]
    }

    /// Returns the index of the region that manages the chunk at the given
    /// chunk index.
    fn get_region_index(chunk_index: &I) -> RegionIndex {
        let conv = |mut q: i32, d: i32| {
            // Divide by a larger number to make sure that negative division is
            // handled properly. Chunk index (-1, -1) should map to region index
            // (-1, -1), but -1 / self.width() = 0.
            if q < 0 {
                q -= C::REGION_WIDTH;
            }

            (q / d)
        };
        RegionIndex(conv(chunk_index.x(), C::REGION_WIDTH),
                    conv(chunk_index.y(), C::REGION_WIDTH))
    }

    /// Returns the handle to a region file. If it doesn't exist, it is created
    /// and the lookup table initialized.
    fn get_region_file<T: AsRef<Path>>(path: T) -> File {
        if !path.as_ref().exists() {
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(path.as_ref()).unwrap();
            file.write(&vec![0u8; Self::lookup_table_size() as usize]).unwrap();
            file
        } else {
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(path.as_ref()).unwrap()
        }
    }

    /// Obtain this chunk's index relative to this region's index.
    fn normalize_chunk_index(&self, chunk_index: &I) -> RegionLocalIndex {
        let conv = |i: i32| {
            let i_new = i % C::REGION_WIDTH;
            if i_new < 0 {
                C::REGION_WIDTH + i_new
            } else {
                i_new
            }
        };
        RegionLocalIndex(conv(chunk_index.x()), conv(chunk_index.y()))
    }

    /// Writes a chunk at an index to disk as marks it as saved.
    fn write_chunk(&mut self, chunk: C, index: &I) -> SerialResult<()>{
        assert!(self.chunk_unsaved(index));

        let mut encoded: Vec<u8> = bincode::serialize(&chunk, Infinite)?;

        let mut compressed = compress_data(&mut encoded)?;
        pad_byte_vec(&mut compressed, C::SECTOR_SIZE);

        let normalized_idx = self.normalize_chunk_index(index);

        let (offset, size) = self.read_chunk_offset(&normalized_idx);

        match size {
            Some(size) => {
                assert!(size >= compressed.len(),
                        "Chunk data grew larger than allocated sector size! \
                         Consider using a larger sector size.");
                self.update_chunk(compressed, offset)?;
            },
            None       => { self.append_chunk(compressed, &normalized_idx)?; },
        }
        self.mark_as_saved(index);
        Ok(())
    }

    fn append_chunk(&mut self, chunk_data: Vec<u8>, index: &RegionLocalIndex) -> SerialResult<()> {
        let sector_count = (chunk_data.len() as f32 / C::SECTOR_SIZE as f32).ceil() as u32;
        assert!(sector_count < 256, "Sector count overflow!");
        assert!(sector_count > 0, "Sector count zero! Len: {}", chunk_data.len());
        let sector_count = sector_count as u8;

        let new_offset = self.handle().seek(SeekFrom::End(0))?;

        self.handle().write(chunk_data.as_slice())?;
        self.write_chunk_offset(index, new_offset, sector_count)?;

        let (o, v) = self.read_chunk_offset(index);
        assert_eq!(new_offset, o, "index: {} new: {} old: {}", index, new_offset, o);
        assert_eq!(sector_count as usize * C::SECTOR_SIZE, v.unwrap());
        Ok(())
    }

    fn update_chunk(&mut self, chunk_data: Vec<u8>, byte_offset: u64) -> SerialResult<()> {
        self.handle().seek(SeekFrom::Start(byte_offset))?;
        self.handle().write(chunk_data.as_slice())?;
        Ok(())
    }

    /// Reads a chunk from disk and marks it as unsaved.
    fn read_chunk(&mut self, index: &I) -> SerialResult<C> {
        assert!(!self.chunk_unsaved(index));

        let normalized_idx = self.normalize_chunk_index(index);
        let (offset, size_opt) = self.read_chunk_offset(&normalized_idx);
        let size = match size_opt {
            Some(s) => s,
            None    => return Err(NoChunkInSavefile(normalized_idx.clone())),
        };

        let buf = self.read_bytes(offset, size);

        let decompressed = decompress_data(&buf)?;
        match bincode::deserialize(decompressed.as_slice()) {
            Ok(dat) => {
                self.mark_as_unsaved(index);
                Ok(dat)
            },
            Err(e)  => {
                println!("Bincode error");
                Err (SerialError::from(e))
            },
        }
    }

    /// Reads the offset and size of the specified chunk inside this region.
    fn read_chunk_offset(&mut self, index: &RegionLocalIndex) -> (u64, Option<usize>) {
        let offset = Self::get_chunk_offset(index);
        let data = self.read_bytes(offset, 2);

        // the byte offset should be u64 for Seek::seek, otherwise it will just
        // be cast every time.
        let offset = Self::lookup_table_size() + (data[0] as usize * C::SECTOR_SIZE) as u64;
        let size = if data[1] == 0 {
            None
        } else {
            Some(data[1] as usize * C::SECTOR_SIZE)
        };
        (offset, size)
    }

    fn write_chunk_offset(&mut self, index: &RegionLocalIndex, new_offset: u64, sector_count: u8) -> SerialResult<()> {
        let val = self.create_lookup_table_entry(new_offset, sector_count);
        let offset = Self::get_chunk_offset(index);
        self.handle().seek(SeekFrom::Start(offset))?;
        self.handle().write(&val)?;
        Ok(())
    }

    /// Gets the offset into the lookup table for the chunk at an index.
    fn get_chunk_offset(index: &RegionLocalIndex) -> u64 {
        2 * ((index.0 % C::REGION_WIDTH) +
             ((index.1 % C::REGION_WIDTH) * C::REGION_WIDTH)) as u64
    }

    fn read_bytes(&mut self, offset: u64, size: usize) -> Vec<u8> {
        self.handle().seek(SeekFrom::Start(offset)).unwrap();
        let mut buf = vec![0u8; size];
        self.handle().read(buf.as_mut_slice()).unwrap();
        buf
    }

    /// Notifies this Region that a chunk was created, so that its lifetime
    /// should be tracked by the Region.
    fn receive_created_chunk(&mut self, index: &I);

    fn is_empty(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decompress() {
        let data = vec![1,2,3,4];

        let mut e = ZlibEncoder::new(Vec::new(), Compression::Default);
        e.write(data.as_slice()).unwrap();
        let buf = e.finish().map_err(SerialError::from).unwrap();

        println!("{:?}", buf);

        let compress = compress_data(&data).unwrap();
        println!("{:?}", compress);

        let decompress = decompress_data(&compress).unwrap();
        assert_eq!(decompress, data);
    }
}
