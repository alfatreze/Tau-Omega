//! Meter pack bundles (`tau-packs.bin`, a `TPKB` file; Tau Alpha `docs/features/TAU_PACKS_FORMAT.md`, reference reader `tools/tau_assets.py parse_packs`):
//! reading one, splitting it into its packs, and composing a bundle from chosen packs by the rules Tau Alpha stated for Omega (interop message 005,
//! Q-019): packs back to back in slot order, `meter id` = slot = the registry index, each pack's `origin` = `0x24900000 + slot * 0x10000`, ABI major 1
//! and minor not above 2, at most 8 slots, a pack for another slot refused, no outer checksum, the file length equals the sum of headers and images.
//!
//! Omega only writes a bundle where Tau Alpha's manifest says the file is user-selectable (decision P1, still to be built on their side): until then
//! it leaves `tau-packs.bin` alone (`tests/packs_untouched.rs`). Nothing here touches a card.

use crate::{ErrorCode, TauError};

pub const BUNDLE_MAGIC: &[u8; 4] = b"TPKB";
pub const PACK_MAGIC: &[u8; 4] = b"TMPK";
pub const BUNDLE_VERSION: u16 = 1;
pub const HEADER_LEN: usize = 48;
/// The pack ABI this Omega understands (`requires_omega.packs_abi` major, and the highest minor Tau Alpha allows today).
pub const SUPPORTED_ABI_MAJOR: u16 = 1;
pub const SUPPORTED_ABI_MINOR: u16 = 2;
pub const MAX_SLOTS: usize = 8;
const ORIGIN_BASE: u32 = 0x2490_0000;
const SLOT_SIZE: u32 = 0x1_0000;

fn bad(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidPathReference, msg)
}
fn u16at(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}
fn u32at(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// The facts of one pack's 48-byte header.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PackInfo {
    /// The meter's index in the registry, which is also the slot the pack belongs to.
    pub meter_id: u16,
    pub abi_major: u16,
    pub abi_minor: u16,
    /// Bytes of image after the header.
    pub image_size: u32,
    pub origin: u32,
    pub entry_offset: u32,
    pub image_crc: u32,
}

/// Checks one pack (`TMPK` header and image, nothing after) and returns its header facts. Refusals follow the firmware's order: header, ABI, meter id,
/// origin, size, CRC.
pub fn check_pack(blob: &[u8]) -> Result<PackInfo, TauError> {
    if blob.len() < HEADER_LEN || &blob[..4] != PACK_MAGIC {
        return Err(bad("a pack header is missing or malformed"));
    }
    let abi = u16at(blob, 4);
    let (major, minor) = (abi >> 8, abi & 0xFF);
    if major != SUPPORTED_ABI_MAJOR || minor > SUPPORTED_ABI_MINOR {
        return Err(bad(format!(
            "a pack needs ABI {major}.{minor}; this Tau Omega knows ABI {SUPPORTED_ABI_MAJOR}.0 to {SUPPORTED_ABI_MAJOR}.{SUPPORTED_ABI_MINOR}"
        )));
    }
    let meter_id = u16at(blob, 6);
    if usize::from(meter_id) >= MAX_SLOTS {
        return Err(bad(format!(
            "meter id {meter_id} is outside the {MAX_SLOTS} pack slots"
        )));
    }
    let image_size = u32at(blob, 8);
    let origin = u32at(blob, 12);
    if origin != ORIGIN_BASE + u32::from(meter_id) * SLOT_SIZE {
        return Err(bad(format!(
            "a pack for meter {meter_id} must be linked at {:#x}, this one at {origin:#x}",
            ORIGIN_BASE + u32::from(meter_id) * SLOT_SIZE
        )));
    }
    if image_size == 0 || blob.len() != HEADER_LEN + image_size as usize {
        return Err(bad("a pack's size does not match its header"));
    }
    let crc = u32at(blob, 20);
    if crc32fast::hash(&blob[HEADER_LEN..]) != crc {
        return Err(bad("a pack's CRC does not match its image"));
    }
    Ok(PackInfo {
        meter_id,
        abi_major: major,
        abi_minor: minor,
        image_size,
        origin,
        entry_offset: u32at(blob, 16),
        image_crc: crc,
    })
}

/// The meter's manifest id (the registry's `id`, which names its `tau-presets/<id>/` folder) read from the descriptor a pack carries:
/// the image ends with `u32 total length, u32 'DSC1'` and the descriptor (`'MDSC'`, version, length, ...) starts that many bytes before the end,
/// with the manifest id at byte 32 and the table slot at 34. `None` when the pack has no readable descriptor.
pub fn manifest_id(blob: &[u8]) -> Option<u16> {
    let image = blob.get(HEADER_LEN..)?;
    let end = image.len().checked_sub(8)?;
    let total = u32at(image, end) as usize;
    if &image[end + 4..] != b"DSC1" || total < 36 || total > image.len() {
        return None;
    }
    let desc = &image[image.len() - total..];
    (&desc[..4] == b"MDSC").then(|| u16at(desc, 32))
}

/// The table slot to manifest id of every pack in a bundle (the saved `MPRE`/`MPID` fields are indexed by table slot, the preset
/// folders by manifest id). Packs without a readable descriptor are left out.
pub fn slot_ids(bundle: &[u8]) -> Result<std::collections::BTreeMap<u16, u16>, TauError> {
    let mut out = std::collections::BTreeMap::new();
    for blob in split_bundle(bundle)? {
        if let (Ok(info), Some(id)) = (check_pack(&blob), manifest_id(&blob)) {
            out.insert(info.meter_id, id);
        }
    }
    Ok(out)
}

/// Reads a bundle, verifying the file as the firmware does: magic and version, each pack, packs back to back in increasing slot order with no
/// repeated id, and no bytes after the last pack. Returns each pack's facts.
pub fn parse_bundle(d: &[u8]) -> Result<Vec<PackInfo>, TauError> {
    Ok(split_bundle(d)?
        .iter()
        .map(|p| check_pack(p).expect("checked while splitting"))
        .collect())
}

/// The individual packs (`.tmpk` blobs) of a bundle, each checked.
pub fn split_bundle(d: &[u8]) -> Result<Vec<Vec<u8>>, TauError> {
    if d.len() < 8 || &d[..4] != BUNDLE_MAGIC {
        return Err(bad("bad TPKB magic"));
    }
    if u16at(d, 4) != BUNDLE_VERSION {
        return Err(bad(format!("unsupported bundle version {}", u16at(d, 4))));
    }
    let count = usize::from(u16at(d, 6));
    if count > MAX_SLOTS {
        return Err(bad(format!(
            "a bundle holds at most {MAX_SLOTS} packs, this one says {count}"
        )));
    }
    let (mut off, mut out, mut last): (usize, Vec<Vec<u8>>, Option<u16>) = (8, Vec::new(), None);
    for _ in 0..count {
        let head = d
            .get(off..off + HEADER_LEN)
            .ok_or_else(|| bad("a pack header is missing or malformed"))?;
        let size = u32at(head, 8) as usize;
        let blob = d
            .get(off..off + HEADER_LEN + size)
            .ok_or_else(|| bad("a pack runs past the end"))?;
        let info = check_pack(blob)?;
        if last.is_some_and(|l| info.meter_id <= l) {
            return Err(bad("packs must come in increasing slot order, once each"));
        }
        last = Some(info.meter_id);
        out.push(blob.to_vec());
        off += HEADER_LEN + size;
    }
    if off != d.len() {
        return Err(bad("bytes after the last pack"));
    }
    Ok(out)
}

/// Composes a bundle from chosen packs: each is checked, they are put in slot order, an id used twice is refused, at most 8. An empty choice gives a
/// valid bundle with no packs (the built-in meter only; Tau Alpha has not tested `count = 0` on a Pocket yet, so callers should ask before writing one).
pub fn compose_bundle(packs: &[Vec<u8>]) -> Result<Vec<u8>, TauError> {
    if packs.len() > MAX_SLOTS {
        return Err(bad(format!("a bundle holds at most {MAX_SLOTS} packs")));
    }
    let mut keyed = Vec::new();
    for p in packs {
        keyed.push((check_pack(p)?.meter_id, p));
    }
    keyed.sort_by_key(|(id, _)| *id);
    if keyed.windows(2).any(|w| w[0].0 == w[1].0) {
        return Err(bad("two packs are for the same meter"));
    }
    let mut out = BUNDLE_MAGIC.to_vec();
    out.extend_from_slice(&BUNDLE_VERSION.to_le_bytes());
    out.extend_from_slice(&(keyed.len() as u16).to_le_bytes());
    for (_, p) in keyed {
        out.extend_from_slice(p);
    }
    Ok(out)
}
