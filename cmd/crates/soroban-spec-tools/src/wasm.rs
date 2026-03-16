/// Replaces a custom section in WASM bytes with new content.
///
/// This function parses the WASM to find the target custom section, then rebuilds
/// the WASM by copying all other sections verbatim and appending the new custom
/// section at the end. If multiple custom sections with the same name exist, all
/// are removed and replaced with a single new section at the end.
///
/// # Arguments
///
/// * `wasm_bytes` - The original WASM binary
/// * `section_name` - The name of the custom section to replace
/// * `new_content` - The new content for the custom section
///
/// # Returns
///
/// A new WASM binary with the custom section replaced.
pub fn replace_custom_section(
    wasm_bytes: &[u8],
    section_name: &str,
    new_content: &[u8],
) -> Result<Vec<u8>, wasmparser::BinaryReaderError> {
    use wasm_encoder::{CustomSection, Module, RawSection};
    use wasmparser::Payload;

    let mut module = Module::new();

    let parser = wasmparser::Parser::new(0);
    for payload in parser.parse_all(wasm_bytes) {
        let payload = payload?;

        // Skip the target custom section - we'll append the new one at the end
        let is_target_section =
            matches!(&payload, Payload::CustomSection(section) if section.name() == section_name);
        if !is_target_section {
            // For all other payloads that represent sections, copy them verbatim
            if let Some((id, range)) = payload.as_section() {
                let raw = RawSection {
                    id,
                    data: &wasm_bytes[range],
                };
                module.section(&raw);
            }
        }
    }

    // Append the new custom section
    let custom = CustomSection {
        name: section_name.into(),
        data: new_content.into(),
    };
    module.section(&custom);

    Ok(module.finish())
}
