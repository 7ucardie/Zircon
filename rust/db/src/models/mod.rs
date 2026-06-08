//! Typed Rust representations of MirDB system models.

pub mod item_info;
pub mod magic_info;
pub mod map_info;
pub mod monster_info;

pub use item_info::ItemInfo;
pub use magic_info::MagicInfo;
pub use map_info::MapInfo;
pub use monster_info::MonsterInfo;

use crate::{
    binary::BinReader,
    error::DbError,
    mapping::{skip_value, PropDef},
    session::{RawCollection, RawObject},
};

/// Load all objects of type `T` from the matching collection in `cols`.
///
/// The collection is identified by the type-name suffix (e.g. "ItemInfo").
pub fn load_typed<T, F>(
    cols: &[RawCollection],
    type_suffix: &str,
    loader: F,
) -> Result<Vec<T>, DbError>
where
    F: Fn(&mut BinReader<'_>, &[PropDef]) -> Result<T, DbError>,
{
    for col in cols {
        if col.mapping.type_name.ends_with(type_suffix) {
            let props = &col.mapping.properties;
            let mut out = Vec::with_capacity(col.objects.len());
            for obj in &col.objects {
                out.push(load_object(&obj, props, &loader)?);
            }
            return Ok(out);
        }
    }
    Ok(Vec::new())
}

fn load_object<T, F>(
    obj: &RawObject,
    props: &[PropDef],
    loader: &F,
) -> Result<T, DbError>
where
    F: Fn(&mut BinReader<'_>, &[PropDef]) -> Result<T, DbError>,
{
    let mut r = BinReader::new(&obj.raw_data);
    loader(&mut r, props)
}

/// Read one field using the mapping's type name and apply it if the property
/// name matches `target`.  Returns the typed value if matched.
///
/// On a name miss, the value is read and discarded using `skip_value`.
pub fn read_field<T, F>(
    r: &mut BinReader<'_>,
    prop: &PropDef,
    target: &str,
    read_fn: F,
) -> Result<Option<T>, DbError>
where
    F: FnOnce(&mut BinReader<'_>) -> Result<T, DbError>,
{
    if prop.name == target {
        Ok(Some(read_fn(r)?))
    } else {
        skip_value(&prop.type_name, r)?;
        Ok(None)
    }
}
