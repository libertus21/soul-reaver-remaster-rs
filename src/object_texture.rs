//! Legacy object texture pages from the monolithic `TEXTURES.BIG` bank
//! (`object-textures`). Page index is `textureWord & 0x07FF`; page `p` is the
//! existing fixed 131,072-byte A1R5G5B5 slot, decoded by [`crate::textures`].
//! No new format, no loose `.DDS` fallback. Synthetic bytes in tests only.

use std::path::{Path, PathBuf};

use crate::dds::DecodedImage;
use crate::srm::Material;
use crate::textures::{self, BigHeader, TexturesError};
use crate::textures_object::MaterialTextures;

/// Flat fallback colour for room, markers and untextured faces.
const FLAT_RGBA: [u8; 4] = [140, 191, 230, 255];
const FLAT_NORMAL: [u8; 4] = [128, 128, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

/// An opened bank: only the verified header and the path are kept.
pub struct PageBank {
    path: PathBuf,
    header: BigHeader,
}

impl PageBank {
    pub fn open(path: &Path) -> Result<Self, TexturesError> {
        let layout = textures::describe(path)?;
        Ok(Self { path: path.to_path_buf(), header: layout.header })
    }

    /// Decode one 256x256 page to RGBA8. A page outside the header count is a
    /// distinct loud error; an `empty`/`zero` slot is refused by the decoder.
    pub fn decode_page(&self, page: u16) -> Result<DecodedImage, TexturesError> {
        if page >= self.header.entry_count {
            return Err(TexturesError::SlotIndexOutOfRange {
                index: usize::from(page),
                count: self.header.entry_count,
            });
        }
        let (_, rgba) = textures::decode_slot_at(&self.path, &self.header, page)?;
        Ok(DecodedImage { width: 256, height: 256, rgba8: rgba })
    }
}

/// Nearest RGBA8 texel at byte UV, top-down (row = v), no flip.
pub fn sample(rgba: &[u8], u: u8, v: u8) -> [u8; 4] {
    let at = (usize::from(u) + usize::from(v) * 256) * 4;
    [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
}

fn flat(rgba: [u8; 4]) -> DecodedImage {
    DecodedImage { width: 1, height: 1, rgba8: rgba.to_vec() }
}

/// Material table for a scene: slot 0 (material 1) is the flat fallback, then
/// one material per page in `pages` order. Legacy object faces carry no
/// normal or specular map, so both stay neutral.
pub fn scene(
    bank: &PageBank,
    pages: &[u16],
) -> Result<(Vec<Material>, Vec<MaterialTextures>), TexturesError> {
    let mut textures = vec![MaterialTextures {
        diffuse: flat(FLAT_RGBA), normal: flat(FLAT_NORMAL), specular: flat(BLACK),
    }];
    let mut materials = vec![Material { name: "flat".to_string(), flags: 1 }];
    for &page in pages {
        textures.push(MaterialTextures {
            diffuse: bank.decode_page(page)?,
            normal: flat(FLAT_NORMAL),
            specular: flat(BLACK),
        });
        materials.push(Material { name: format!("page{page}"), flags: 1 });
    }
    Ok((materials, textures))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::textures::{SLOT_DATA_START, SLOT_SIZE};

    /// Synthetic bank with every slot as `data` and the requested texels set.
    fn bank_bytes(count: u16, texels: &[(u16, u8, u8, u16)]) -> Vec<u8> {
        let mut bytes = vec![0u8; SLOT_DATA_START as usize + usize::from(count) * SLOT_SIZE as usize];
        bytes[0..2].copy_from_slice(&512u16.to_le_bytes());
        bytes[2..4].copy_from_slice(&count.to_le_bytes());
        for pixel in bytes[SLOT_DATA_START as usize..].chunks_exact_mut(2) {
            pixel.copy_from_slice(&0x8001u16.to_le_bytes());
        }
        for &(page, u, v, value) in texels {
            let at = SLOT_DATA_START as usize
                + usize::from(page) * SLOT_SIZE as usize
                + 2 * (usize::from(u) + 256 * usize::from(v));
            bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    fn write_temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("object-texture-{name}-{}", std::process::id()));
        std::fs::write(&path, bytes).expect("synthetic bank must be writable");
        path
    }

    #[test]
    fn decodes_a_page_and_samples_the_addressed_texel() {
        let path = write_temp("sample", &bank_bytes(2, &[(1, 2, 3, 0xFC00)]));
        let bank = PageBank::open(&path).expect("bank must open");
        let page = bank.decode_page(1).expect("page must decode");
        assert_eq!((page.width, page.height, page.rgba8.len()), (256, 256, 262144));
        assert_eq!(sample(&page.rgba8, 2, 3), [255, 0, 0, 255]);
        assert_eq!(sample(&page.rgba8, 0, 0), [0, 0, 8, 255], "bit-replicated 0x8001");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn refuses_pages_outside_the_header_and_empty_slots() {
        let path = write_temp("bounds", &bank_bytes(1, &[]));
        let bank = PageBank::open(&path).expect("bank must open");
        assert!(matches!(
            bank.decode_page(1),
            Err(TexturesError::SlotIndexOutOfRange { index: 1, count: 1 })
        ));
        let _ = std::fs::remove_file(&path);

        let mut bytes = bank_bytes(1, &[]);
        bytes[SLOT_DATA_START as usize..SLOT_DATA_START as usize + SLOT_SIZE as usize].fill(0xFF);
        let path = write_temp("empty", &bytes);
        let bank = PageBank::open(&path).expect("bank must still open");
        assert!(matches!(
            bank.decode_page(0),
            Err(TexturesError::SlotNoPayload { index: 0, .. })
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn scene_puts_flat_at_material_one_and_pages_after() {
        let path = write_temp("scene", &bank_bytes(2, &[]));
        let bank = PageBank::open(&path).expect("bank must open");
        let (materials, textures) = scene(&bank, &[1]).expect("scene must build");
        assert_eq!(materials.len(), 2);
        assert_eq!(textures.len(), 2);
        assert_eq!((textures[0].diffuse.width, textures[0].diffuse.height), (1, 1));
        assert_eq!((textures[1].diffuse.width, textures[1].diffuse.height), (256, 256));
        assert!(matches!(
            scene(&bank, &[2]),
            Err(TexturesError::SlotIndexOutOfRange { index: 2, count: 2 })
        ));
        let _ = std::fs::remove_file(&path);
    }
}
