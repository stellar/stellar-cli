use std::{fs, io, path::Path};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("reading wasm file: {0}")]
    ReadingWasmFile(io::Error),

    #[error("deleting existing artifact: {0}")]
    DeletingArtifact(io::Error),

    #[error("writing wasm file: {0}")]
    WritingWasmFile(io::Error),

    #[error(transparent)]
    Meta(#[from] soroban_meta::read::FromWasmError),

    #[error(transparent)]
    Shake(#[from] soroban_spec::strip::ShakeError),
}

pub fn shake_file_if_v2(wasm_path: &Path) -> Result<(), Error> {
    let wasm = fs::read(wasm_path).map_err(Error::ReadingWasmFile)?;
    let Some(shaken) = shake_if_v2(&wasm)? else {
        return Ok(());
    };

    fs::remove_file(wasm_path).map_err(Error::DeletingArtifact)?;
    fs::write(wasm_path, shaken).map_err(Error::WritingWasmFile)
}

fn shake_if_v2(wasm: &[u8]) -> Result<Option<Vec<u8>>, Error> {
    let meta = soroban_meta::read::from_wasm(wasm)?;
    let version = soroban_spec::shaking::spec_shaking_version_for_meta(&meta);

    if version != 2 {
        return Ok(None);
    }

    soroban_spec::strip::shake_contract_spec(wasm)
        .map(Some)
        .map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellar_xdr::curr::{
        Limits, ScMetaEntry, ScMetaV0, ScSpecEntry, ScSpecFunctionV0, StringM, VecM, WriteXdr,
    };

    const CONTRACT_META_SECTION: &str = "contractmetav0";
    const CONTRACT_SPEC_SECTION: &str = "contractspecv0";

    #[test]
    fn leaves_non_v2_wasm_unchanged() {
        let wasm = wasm_with_sections(None, true);

        assert!(shake_if_v2(&wasm).unwrap().is_none());
    }

    #[test]
    fn shakes_v2_wasm_and_removes_sidecar() {
        let wasm = wasm_with_sections(Some(soroban_spec::shaking::META_VALUE_V2), true);

        let shaken = shake_if_v2(&wasm).unwrap().unwrap();

        let section_names = custom_section_names(&shaken);
        assert!(section_names.contains(&CONTRACT_META_SECTION.to_string()));
        assert!(section_names.contains(&CONTRACT_SPEC_SECTION.to_string()));
        assert!(!section_names.contains(&soroban_spec::shaking::GRAPH_SECTION.to_string()));

        let spec = soroban_spec::read::from_wasm(&shaken).unwrap();
        assert_eq!(spec.len(), 1);
    }

    fn wasm_with_sections(meta_version: Option<&str>, include_graph: bool) -> Vec<u8> {
        let mut wasm = b"\0asm\x01\0\0\0".to_vec();
        wasm_gen::write_custom_section(&mut wasm, CONTRACT_META_SECTION, &meta_xdr(meta_version));
        wasm_gen::write_custom_section(&mut wasm, CONTRACT_SPEC_SECTION, &spec_xdr());
        if include_graph {
            wasm_gen::write_custom_section(
                &mut wasm,
                soroban_spec::shaking::GRAPH_SECTION,
                b"sidecar",
            );
        }
        wasm
    }

    fn meta_xdr(version: Option<&str>) -> Vec<u8> {
        let Some(version) = version else {
            return Vec::new();
        };

        ScMetaEntry::ScMetaV0(ScMetaV0 {
            key: soroban_spec::shaking::META_KEY.try_into().unwrap(),
            val: version.try_into().unwrap(),
        })
        .to_xdr(Limits::none())
        .unwrap()
    }

    fn spec_xdr() -> Vec<u8> {
        ScSpecEntry::FunctionV0(ScSpecFunctionV0 {
            doc: StringM::default(),
            name: "hello".try_into().unwrap(),
            inputs: VecM::default(),
            outputs: VecM::default(),
        })
        .to_xdr(Limits::none())
        .unwrap()
    }

    fn custom_section_names(wasm: &[u8]) -> Vec<String> {
        let mut names = Vec::new();
        for payload in wasmparser::Parser::new(0).parse_all(wasm) {
            if let Ok(wasmparser::Payload::CustomSection(section)) = payload {
                names.push(section.name().to_string());
            }
        }
        names
    }
}
