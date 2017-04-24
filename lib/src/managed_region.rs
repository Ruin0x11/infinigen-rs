use std::io::prelude::*;
use std::io::SeekFrom;
use std::fs::{File, OpenOptions};
use std::path::Path;

use bincode::{self, Infinite};
use serde::Serialize;
use serde::de::Deserialize;

use region::*;
use traits::Index;

/// Pads the given byte vec with zeroes to the next multiple of the given sector
/// size.
fn pad_byte_vec(bytes: &mut Vec<u8>, size: usize) {
    for _ in 0..(size - (bytes.len() % size)) {
        bytes.push(0);
    }
}

pub trait ManagedRegion<'a, C, H, I: Index>
    where H: Seek + Write + Read,
          C: Serialize + Deserialize{

    const SECTOR_SIZE: usize = 4096;

    /// The number of chunks per row inside regions.
    const REGION_WIDTH: i32 = 16;

    fn lookup_table_size() -> u64 { (Self::REGION_WIDTH * Self::REGION_WIDTH) as u64 * 2 }

    fn chunk_unsaved(&self, index: &I) -> bool;
    fn mark_as_saved(&mut self, index: &I);
    fn mark_as_unsaved(&mut self, index: &I);
    fn handle(&mut self) -> &mut H;

    fn create_lookup_table_entry(&self, eof: u64, sector_count: u8) -> [u8; 2] {
        let offset: u8 = ((eof - Self::lookup_table_size()) / Self::SECTOR_SIZE as u64) as u8;

        [offset, sector_count]
    }

    fn get_region_index(chunk_index: &I) -> RegionIndex {
        let conv = |mut q: i32, d: i32| {
            // Divide by a larger number to make sure that negative division is
            // handled properly. Chunk index (-1, -1) should map to region index
            // (-1, -1), but -1 / self.width() = 0.
            if q < 0 {
                q -= Self::REGION_WIDTH;
            }

            (q / d)
        };
        RegionIndex(conv(chunk_index.x(), Self::REGION_WIDTH),
                    conv(chunk_index.y(), Self::REGION_WIDTH))
    }

    fn get_region_file(filename: String) -> File {
        if !Path::new(&filename).exists() {
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(filename) .unwrap();
            file.write(&vec![0u8; Self::lookup_table_size() as usize]).unwrap();
            file
        } else {
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(filename).unwrap()
        }
    }

    /// Obtain this chunk's index relative to this region's index.
    fn normalize_chunk_index(&self, chunk_index: &I) -> RegionLocalIndex {
        let conv = |i: i32| {
            let i_new = i % Self::REGION_WIDTH;
            if i_new < 0 {
                Self::REGION_WIDTH + i_new
            } else {
                i_new
            }
        };
        RegionLocalIndex(conv(chunk_index.x()), conv(chunk_index.y()))
    }

    fn write_chunk(&mut self, chunk: C, index: &I) -> SerialResult<()>{
        let i = index.clone().into();
        assert!(self.chunk_unsaved(index));

        let encoded: Vec<u8> = bincode::serialize(&chunk, Infinite)?;
        // FIXME: Compression makes chunk unloading nondeterministic, because
        // there is no way to know the amount of padding added and the
        // decompressor treats the padding as part of the file.

        // let mut compressed = compress_data(&mut encoded)?;
        let mut compressed = encoded;
        pad_byte_vec(&mut compressed, Self::SECTOR_SIZE);

        let normalized_idx = self.normalize_chunk_index(i);

        let (offset, size) = self.read_chunk_offset(&normalized_idx);
        // println!("WRITE idx: {} offset: {} exists: {}", normalized_idx, offset, size.is_some());

        match size {
            Some(size) => {
                assert!(size >= compressed.len(), "Chunk data grew past allocated sector_count!");
                self.update_chunk(compressed, offset)?;
            },
            None       => { self.append_chunk(compressed, &normalized_idx)?; },
        }
        self.mark_as_saved(index);
        Ok(())
    }

    fn append_chunk(&mut self, chunk_data: Vec<u8>, index: &RegionLocalIndex) -> SerialResult<()> {
        let sector_count = (chunk_data.len() as f32 / Self::SECTOR_SIZE as f32).ceil() as u32;
        assert!(sector_count < 256, "Sector count overflow!");
        assert!(sector_count > 0, "Sector count zero! Len: {}", chunk_data.len());
        let sector_count = sector_count as u8;

        let new_offset = self.handle().seek(SeekFrom::End(0))?;
        // println!("APPEND idx: {} offset: {}", index, new_offset);

        self.handle().write(chunk_data.as_slice())?;
        self.write_chunk_offset(index, new_offset, sector_count)?;

        let (o, v) = self.read_chunk_offset(index);
        assert_eq!(new_offset, o, "index: {} new: {} old: {}", index, new_offset, o);
        assert_eq!(sector_count as usize * Self::SECTOR_SIZE, v.unwrap());
        Ok(())
    }

    fn update_chunk(&mut self, chunk_data: Vec<u8>, byte_offset: u64) -> SerialResult<()> {
        self.handle().seek(SeekFrom::Start(byte_offset))?;
        self.handle().write(chunk_data.as_slice())?;
        Ok(())
    }

    fn read_chunk(&mut self, index: &I) -> SerialResult<C> {
        assert!(!self.chunk_unsaved(index));

        let normalized_idx = self.normalize_chunk_index(index);
        let (offset, size_opt) = self.read_chunk_offset(&normalized_idx);
        // println!("OFFSET: {}", offset);
        let size = match size_opt {
            Some(s) => s,
            None    => return Err(NoChunkInSavefile(normalized_idx.clone())),
        };

        // println!("READ idx: {} offset: {}", normalized_idx, offset);
        let buf = self.read_bytes(offset, size);

        // let decompressed = decompress_data(&buf)?;
        match bincode::deserialize(buf.clone().as_slice()) {
            Ok(dat) => {
                self.mark_as_unsaved(index);
                Ok(dat)
            },
            Err(e)  => Err(SerialError::from(e)),
        }
    }

    fn read_chunk_offset(&mut self, index: &RegionLocalIndex) -> (u64, Option<usize>) {
        let offset = Self::get_chunk_offset(index);
        let data = self.read_bytes(offset, 2);

        // the byte offset should be u64 for Seek::seek, otherwise it will just
        // be cast every time.
        let offset = Self::lookup_table_size() + (data[0] as usize * Self::SECTOR_SIZE) as u64;
        let size = if data[1] == 0 {
            None
        } else {
            Some(data[1] as usize * Self::SECTOR_SIZE)
        };
        // println!("idx: {} offset: {} size: {}", index, offset, data[1]);
        (offset, size)
    }

    fn write_chunk_offset(&mut self, index: &RegionLocalIndex, new_offset: u64, sector_count: u8) -> SerialResult<()> {
        // println!("offset: {} sectors: {}", new_offset, sector_count);
        let val = self.create_lookup_table_entry(new_offset, sector_count);
        let offset = Self::get_chunk_offset(index);
        self.handle().seek(SeekFrom::Start(offset))?;
        self.handle().write(&val)?;
        Ok(())
    }

    fn get_chunk_offset(index: &RegionLocalIndex) -> u64 {
        2 * ((index.0 % Self::REGION_WIDTH) +
             ((index.1 % Self::REGION_WIDTH) * Self::REGION_WIDTH)) as u64
    }

    fn read_bytes(&mut self, offset: u64, size: usize) -> Vec<u8>;

    /// Notifies this Region that a chunk was created, so that its lifetime
    /// should be tracked by the Region.
    fn receive_created_chunk(&mut self, index: &I);

    fn is_empty(&self) -> bool;
}
