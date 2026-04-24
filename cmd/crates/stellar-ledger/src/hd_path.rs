use crate::{Error, HD_PATH_ELEMENTS_COUNT};

const HARDENED_OFFSET: u32 = 1 << 31;
const PURPOSE: u32 = 44;
const COIN_TYPE: u32 = 148;

#[derive(Clone, Copy)]
pub struct HdPath(pub u32);

impl HdPath {
    #[must_use]
    pub fn depth(&self) -> u8 {
        HD_PATH_ELEMENTS_COUNT
    }
}

impl From<u32> for HdPath {
    fn from(index: u32) -> Self {
        HdPath(index)
    }
}

impl From<&u32> for HdPath {
    fn from(index: &u32) -> Self {
        HdPath(*index)
    }
}

impl HdPath {
    /// # Errors
    ///
    /// Could fail to convert the path to bytes
    pub fn to_vec(&self) -> Result<Vec<u8>, Error> {
        hd_path_to_bytes(*self)
    }
}

fn hd_path_to_bytes(hd_path: HdPath) -> Result<Vec<u8>, Error> {
    let index = hardened(hd_path.0, hd_path)?;
    let result = [
        hardened(PURPOSE, hd_path)?,
        hardened(COIN_TYPE, hd_path)?,
        index,
    ];
    Ok(result.into_iter().flat_map(u32::to_be_bytes).collect())
}

fn hardened(value: u32, hd_path: HdPath) -> Result<u32, Error> {
    value
        .checked_add(HARDENED_OFFSET)
        .ok_or_else(|| Error::Bip32PathError(path_string(hd_path)))
}

fn path_string(hd_path: HdPath) -> String {
    format!("m/{PURPOSE}'/{COIN_TYPE}'/{}'", hd_path.0)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_depth() {
        assert_eq!(HdPath(7).depth(), HD_PATH_ELEMENTS_COUNT);
    }

    #[test]
    fn test_to_vec() {
        assert_eq!(
            HdPath(7).to_vec().unwrap(),
            vec![0x80, 0x00, 0x00, 0x2c, 0x80, 0x00, 0x00, 0x94, 0x80, 0x00, 0x00, 0x07,]
        );
    }

    #[test]
    fn test_to_vec_rejects_out_of_range_index() {
        let err = HdPath(HARDENED_OFFSET).to_vec().unwrap_err();
        assert!(matches!(err, Error::Bip32PathError(_)));
        assert_eq!(
            err.to_string(),
            format!(
                "Error occurred while parsing BIP32 path: {}",
                path_string(HdPath(HARDENED_OFFSET))
            )
        );
    }
}
