//! The shape of a block only its mod knows.
//!
//! Horizon's registry holds one shape per block state, exported from the mod with no world and
//! no block entity around it. That is the whole truth for most blocks and none of it for a
//! block whose shape is its block entity's: LibMultiPart's container answers *nothing* without
//! the parts inside it, so every one of its 3072 states is recorded as shapeless, and a Wired
//! Redstone cable placed in the world could not be looked at, aimed at or mined.
//!
//! So the shape travels from where it is known. The server asks the mod for it at the position
//! (`java_mods_120`), and keeps it on the block entity as a synced attachment, which reaches
//! every client that can see the block by the road attachments already take and is saved with
//! the chunk, so a reloaded world knows the shape before the mod has ticked once.
//!
//! Boxes are inside the block, `[x0, y0, z0, x1, y1, z1]` each, as a mod's `getShape` gives
//! them. An empty list means the mod says there is nothing to aim at, which is not the same as
//! having no attachment at all -- that one means nobody has asked.

use horizon_world::attachments::{AttachmentCodec, AttachmentError, AttachmentType, Attachments};
use horizon_world::nbt::{Compound, Tag};

/// One block's boxes, as a mod gives them.
pub type Boxes = Vec<[f64; 6]>;

fn encode(boxes: &Boxes) -> Result<Compound, AttachmentError> {
    let flat: Vec<Tag> = boxes
        .iter()
        .flatten()
        .map(|v| Tag::Double(v.to_bits()))
        .collect();
    let mut c = Compound::new();
    c.insert("boxes".into(), Tag::List(flat));
    Ok(c)
}

fn decode(c: &Compound) -> Result<Boxes, AttachmentError> {
    let Some(Tag::List(list)) = c.get(&"boxes".into()) else {
        return Err(AttachmentError::Decode);
    };
    if list.len() % 6 != 0 {
        return Err(AttachmentError::Decode);
    }
    let mut flat = Vec::with_capacity(list.len());
    for tag in list {
        let Tag::Double(bits) = tag else {
            return Err(AttachmentError::Decode);
        };
        flat.push(f64::from_bits(*bits));
    }
    Ok(flat
        .chunks_exact(6)
        .map(|b| [b[0], b[1], b[2], b[3], b[4], b[5]])
        .collect())
}

/// `horizon:block_shape`: the boxes a mod gives for the block at a block entity's position.
pub static BLOCK_SHAPE: AttachmentType<Boxes> = AttachmentType {
    namespace: "horizon",
    name: "block_shape",
    default: Vec::new,
    codec: Some(AttachmentCodec { encode, decode }),
    copy_on_death: false,
    sync: true,
};

/// `horizon:block_collision`: what of the block at a block entity's position is solid, as its
/// mod gives it (`getCollisionShape`): the outline's twin, for walking rather than aiming.
pub static BLOCK_COLLISION: AttachmentType<Boxes> = AttachmentType {
    namespace: "horizon",
    name: "block_collision",
    default: Vec::new,
    codec: Some(AttachmentCodec { encode, decode }),
    copy_on_death: false,
    sync: true,
};

/// The boxes a holder carries, or `None` when nothing has asked the mod about it.
pub fn of(attachments: &Attachments) -> Option<Boxes> {
    attachments.peek(&BLOCK_SHAPE).ok().flatten()
}

impl super::Game<'_> {
    /// The boxes the block at `pos` has because its mod said so, or `None` when no mod has:
    /// then the registry's shape is the whole answer, as it is for every vanilla block.
    ///
    /// A window reads what the server synced to it; a server (and a window serving its own
    /// world) reads its own block entities. An empty list is an answer too -- the mod says
    /// there is nothing there to aim at -- and is not the same as having no attachment.
    pub(super) fn mod_block_shape(&self, pos: [i32; 3]) -> Option<Boxes> {
        let [x, y, z] = pos;
        if let Some(r) = self.remote.as_ref() {
            let holder = horizon_world::attachments::Holder::BlockEntity([x, y, z]);
            if let Some(found) = r.attachments.get(&holder).and_then(of) {
                return Some(found);
            }
        }
        self.world
            .block_entities
            .attachments((x, y, z))
            .and_then(of)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_survive_the_trip_to_a_client() {
        let cable: Boxes = vec![[0.125, 0.0, 0.125, 0.875, 0.25, 0.875]];
        assert_eq!(decode(&encode(&cable).unwrap()).unwrap(), cable);
        let none: Boxes = Vec::new();
        assert_eq!(decode(&encode(&none).unwrap()).unwrap(), none);
    }

    #[test]
    fn a_half_written_box_list_is_refused_rather_than_aimed_at() {
        let mut c = Compound::new();
        c.insert(
            "boxes".into(),
            Tag::List(vec![
                Tag::Double(0f64.to_bits()),
                Tag::Double(1f64.to_bits()),
                Tag::Double(0.5f64.to_bits()),
            ]),
        );
        assert!(decode(&c).is_err());
        assert!(decode(&Compound::new()).is_err());
    }
}
