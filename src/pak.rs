use core::intrinsics::transmute;

use std::{default::Default, time::SystemTime};

pub struct PakRecord {
    pub name: String,
    pub filetime: SystemTime,
    pub data: Vec<u8>,
}

#[derive(Default)]
pub struct PakManager {
    pub records: Vec<PakRecord>,
}

impl PakManager {
    const CYPHER_BYTE: u8 = 0xF7;
    const MAGIC_HEADER: [u8; 8] = [0xC0, 0x4A, 0xC0, 0xBA, 0x00, 0x00, 0x00, 0x00];
    const ENTRY_FLAG: u8 = 0x00;
    const END_FLAG: u8 = 0x80;

    pub fn to_bytes(&self) -> Vec<u8> {
        let result_size: usize = self
            .records
            .iter()
            .map(|record| record.data.len() + record.name.len() + 14 as usize)
            .sum();

        let mut bytes = Vec::with_capacity(result_size + 9);
        bytes.extend(
            PakManager::MAGIC_HEADER
                .iter()
                .map(|x| x ^ PakManager::CYPHER_BYTE),
        );

        for record in &self.records {
            bytes.push(PakManager::ENTRY_FLAG ^ PakManager::CYPHER_BYTE);
            bytes.push(record.name.len() as u8 ^ PakManager::CYPHER_BYTE);
            bytes.extend(
                record
                    .name
                    .as_str()
                    .as_bytes()
                    .iter()
                    .map(|x| x ^ PakManager::CYPHER_BYTE),
            );
            bytes.extend(
                unsafe { transmute::<usize, [u8; 4]>(record.data.len()) }
                    .iter()
                    .map(|x| x ^ PakManager::CYPHER_BYTE),
            );
            bytes.extend(
                unsafe { transmute::<SystemTime, [u8; 8]>(record.filetime) }
                    .iter()
                    .map(|x| x ^ PakManager::CYPHER_BYTE),
            );
        }
        bytes.push(PakManager::END_FLAG ^ PakManager::CYPHER_BYTE);
        for record in &self.records {
            bytes.extend(record.data.iter().map(|x| x ^ PakManager::CYPHER_BYTE));
        }

        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut bytes = bytes.iter();
        let bytes = bytes.by_ref();

        let header: Vec<u8> = bytes.take(8).map(|x| x ^ PakManager::CYPHER_BYTE).collect();
        if header != PakManager::MAGIC_HEADER {
            return Default::default();
        }

        let mut flag: u8;
        if let Some(byte) = bytes.take(1).map(|x| x ^ PakManager::CYPHER_BYTE).next() {
            flag = byte;
        } else {
            return Default::default();
        }

        let mut entries = Vec::new();

        while flag == PakManager::ENTRY_FLAG {
            let filename_length =
                if let Some(byte) = bytes.take(1).map(|x| x ^ PakManager::CYPHER_BYTE).next() {
                    byte as usize
                } else {
                    return Default::default();
                };
            let filename = String::from_utf8_lossy(
                &bytes
                    .take(filename_length)
                    .map(|x| x ^ PakManager::CYPHER_BYTE)
                    .collect::<Vec<_>>(),
            )
            .into_owned();

            let filesize = unsafe {
                transmute::<[u8; 4], usize>(
                    bytes
                        .take(4)
                        .map(|x| x ^ PakManager::CYPHER_BYTE)
                        .collect::<Vec<_>>()
                        .as_slice()
                        .try_into()
                        .unwrap(),
                )
            };
            let filetime = unsafe {
                transmute::<[u8; 8], SystemTime>(
                    bytes
                        .take(8)
                        .map(|x| x ^ PakManager::CYPHER_BYTE)
                        .collect::<Vec<_>>()
                        .as_slice()
                        .try_into()
                        .unwrap(),
                )
            };
            entries.push((filename, filesize, filetime));
            if let Some(byte) = bytes.take(1).map(|x| x ^ PakManager::CYPHER_BYTE).next() {
                flag = byte;
            } else {
                return Default::default();
            }
        }

        if flag != PakManager::END_FLAG {
            return Default::default();
        }

        PakManager {
            records: entries
                .iter()
                .map(|(filename, filesize, filetime)| {
                    let data = bytes
                        .take(*filesize)
                        .map(|x| x ^ PakManager::CYPHER_BYTE)
                        .collect();
                    PakRecord {
                        name: filename.to_owned(),
                        filetime: *filetime,
                        data,
                    }
                })
                .collect(),
        }
    }
}
