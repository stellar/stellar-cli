use std::{fs, io, path::Path};
use stellar_xdr::curr::ScSpecEntry;
use crate::types::{Entry, Type};
use crate::{type_to_ts, wrapper::type_to_js_xdr};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

pub struct McpServerGenerator;

impl McpServerGenerator {
    pub fn new() -> Self {
        Self
    }

    fn type_to_zod(value: &Type) -> String {
        let xdr_converter = type_to_js_xdr(value);
        match value {
            // Numbers
            Type::U64 | Type::I64 | Type::U32 | Type::I32 | Type::Timepoint | Type::Duration => {
                "z.number()".to_string()
            },

            // Large numbers and addresses as strings
            Type::U128 | Type::I128 | Type::U256 | Type::I256 | Type::Address | Type::Symbol | Type::String => {
                "z.string()".to_string()
            },

            // Boolean
            Type::Bool => "z.boolean()".to_string(),

            // Buffer types
            Type::Bytes => format!(
                "z.preprocess((val) => {{
    if (typeof val === 'string' && val.startsWith('[') && val.endsWith(']')) {{
      try {{ return JSON.parse(val); }} catch (_) {{ return val; }}
    }}
    return val;
  }}, z.union([
    z.instanceof(Buffer),
    z.array(z.number().min(0).max(255)),
    z.string().transform((str) => Buffer.from(str, 'base64'))
  ]))"
            ),
            Type::BytesN { n } => {
                format!(
                    "z.preprocess((val) => {{
    if (typeof val === 'string' && val.startsWith('[') && val.endsWith(']')) {{
      try {{ return JSON.parse(val); }} catch (_) {{ return val; }}
    }}
    return val;
  }}, z.union([
    z.instanceof(Buffer).refine((buf) => buf.length === {}, 'Buffer must be exactly {} bytes'),
    z.array(z.number().min(0).max(255)).length({}),
    z.string().transform((str) => {{
      const buf = Buffer.from(str, 'base64');
      if (buf.length !== {}) throw new Error('Buffer must be exactly {} bytes');
      return buf;
    }})
  ]))",
                    n, n, n, n, n
                )
            },

            // Compound types
            Type::Option { value } => format!("{}.optional()", Self::type_to_zod(value)),
            Type::Vec { element } => format!("z.array({})", Self::type_to_zod(element)),
            Type::Map { key, value } => format!("z.map({}, {})", Self::type_to_zod(key), Self::type_to_zod(value)),
            Type::Tuple { elements } => {
                let element_types = elements.iter()
                    .map(Self::type_to_zod)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("z.tuple([{}])", element_types)
            },

            // Custom types
            Type::Custom { .. } => "z.any()".to_string(),

            // Fallback
            _ => "z.any()".to_string(),
        }
    }

    fn get_type_description(value: &Type) -> String {
        let xdr_converter = type_to_js_xdr(value);
        match value {
            Type::U64 => format!("Unsigned 64-bit integer (0 to 18,446,744,073,709,551,615) - Converts to: {}", xdr_converter),
            Type::I64 => format!("Signed 64-bit integer (-9,223,372,036,854,775,808 to 9,223,372,036,854,775,807) - Converts to: {}", xdr_converter),
            Type::U32 => format!("Unsigned 32-bit integer (0 to 4,294,967,295) - Converts to: {}", xdr_converter),
            Type::I32 => format!("Signed 32-bit integer (-2,147,483,648 to 2,147,483,647) - Converts to: {}", xdr_converter),
            Type::U128 => format!("Unsigned 128-bit integer as string (0 to 340,282,366,920,938,463,463,374,607,431,768,211,455) - Converts to: {}", xdr_converter),
            Type::I128 => format!("Signed 128-bit integer as string (-170,141,183,460,469,231,731,687,303,715,884,105,728 to 170,141,183,460,469,231,731,687,303,715,884,105,727) - Converts to: {}", xdr_converter),
            Type::U256 => format!("Unsigned 256-bit integer as string - Converts to: {}", xdr_converter),
            Type::I256 => format!("Signed 256-bit integer as string - Converts to: {}", xdr_converter),
            Type::Address => format!("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: {}", xdr_converter),
            Type::Symbol => format!("Stellar symbol/enum value - Converts to: {}", xdr_converter),
            Type::String => format!("UTF-8 encoded string - Converts to: {}", xdr_converter),
            Type::Timepoint => format!("Unix timestamp in seconds - Converts to: {}", xdr_converter),
            Type::Duration => format!("Time duration in seconds - Converts to: {}", xdr_converter),
            Type::Bool => format!("Boolean value (true/false) - Converts to: {}", xdr_converter),
            Type::Bytes => format!("Binary data as Buffer, byte array, or base64 string - Converts to: {}", xdr_converter),
            Type::BytesN { n } => format!("Fixed-length binary data of {} bytes as Buffer, byte array, or base64 string - Converts to: {}", n, xdr_converter),
            Type::Option { value } => format!("Optional {} - Converts to: {}", type_to_ts(value), xdr_converter),
            Type::Vec { element } => format!("Array of {} - Converts to: {}", type_to_ts(element), xdr_converter),
            Type::Map { key, value } => format!("Map of {} to {} - Converts to: {}", type_to_ts(key), type_to_ts(value), xdr_converter),
            Type::Tuple { elements } => format!("Tuple of [{}] - Converts to: {}", elements.iter().map(type_to_ts).collect::<Vec<_>>().join(", "), xdr_converter),
            Type::Custom { name } => format!("Custom type: {} - Converts to: {}", name, xdr_converter),
            _ => format!("Any value - Converts to: {}", xdr_converter),
        }
    }

    pub fn generate(&self, output_dir: &Path, name: &str, spec: &[ScSpecEntry]) -> Result<(), Error> {
        // Create the output directory if it doesn't exist
        fs::create_dir_all(output_dir)?;

        // Generate the MCP server code
        let mut tools = String::new();
        for entry in spec {
            if let Some(tool) = self.generate_tool(entry) {
                tools.push_str(&tool);
                tools.push('\n');
            }
        }

        // Read the template file
        let template_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/mcp_server_template");
        let template = fs::read_to_string(template_dir.join("src/index.ts"))?;

        // Replace placeholders in the template
        let index_content = template
            .replace("INSERT_NAME_HERE", name)
            .replace("INSERT_TOOLS_HERE", &tools);

        // Write the generated code to index.ts
        let index_path = output_dir.join("src/index.ts");
        fs::create_dir_all(index_path.parent().unwrap())?;
        fs::write(index_path, index_content)?;

        // Copy and update package.json
        let package_json = fs::read_to_string(template_dir.join("package.json"))?;
        let package_json = package_json.replace("INSERT_NAME_HERE", name);
        fs::write(output_dir.join("package.json"), package_json)?;

        // Copy and update README.md
        let readme = fs::read_to_string(template_dir.join("README.md"))?;
        let readme = readme.replace("INSERT_NAME_HERE", name);
        fs::write(output_dir.join("README.md"), readme)?;

        // Copy tsconfig.json
        fs::copy(
            template_dir.join("tsconfig.json"),
            output_dir.join("tsconfig.json"),
        )?;

        // Copy .env.example
        fs::copy(
            template_dir.join(".env.example"),
            output_dir.join(".env.example"),
        )?;

        Ok(())
    }

    fn generate_tool(&self, entry: &ScSpecEntry) -> Option<String> {
        let entry = Entry::from(entry);
        match entry {
            Entry::Function { name, doc, inputs, .. } => {
                let description = if doc.is_empty() {
                    format!("Call the {} function", name)
                } else {
                    doc.replace("\n\n", "{{DOUBLE_NEWLINE}}")  
                        .replace('\n', " ")                     
                        .replace("{{DOUBLE_NEWLINE}}", "\\n\\n") 
                        .replace("`", "\\`")                    
                        .replace("\"", "\\\"")                  
                };

                let has_params = !inputs.is_empty();
                let params = if has_params {
                    let params_str = inputs
                        .iter()
                        .map(|input| {
                            let type_info = Self::type_to_zod(&input.value);
                            let type_desc = Self::get_type_description(&input.value);
                            
                            format!("    {}: {}.describe(\"{}\")", 
                                input.name,
                                type_info,
                                if input.doc.is_empty() {
                                    type_desc
                                } else {
                                    format!("{} - {}", 
                                        input.doc.replace('\n', " ")
                                              .replace("`", "\\`")
                                              .replace("\"", "\\\""),
                                        type_desc
                                    )
                                }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",\n");
                    format!("{},\n", params_str)
                } else {
                    String::new()
                };

                Some(format!(
                    r#"mcpServer.tool(
  "{}",
  "{}",
  {{
{}    simulate: z.boolean().optional().describe("If true, simulate the transaction instead of submitting it"),
    signAndSubmit: z.boolean().optional().describe("If true, sign and submit the transaction"),
    secretKey: z.string().optional().describe("Secret key in strkey format (S... for secret keys)")
  }},
  async (params) => {{
    try {{
      const {{ simulate, signAndSubmit, secretKey{} }} = params;{}
      
      // Call the contract to get the assembled transaction
      const tx = await contract.call("{}"{});
      
      // Get the XDR of the transaction and convert to base64 string
      const xdr = tx.toXDR('base64');
      
      // Create a new TransactionBuilder from the XDR
      const transaction = TransactionBuilder.fromXDR(xdr, config.networkPassphrase);
      
      if (simulate) {{
        // Simulate the transaction using the server
        const simulateResult = await server.simulateTransaction(transaction);
        
        return {{
          content: [
            {{ type: "text", text: "Simulation Results:" }},
            {{ type: "text", text: JSON.stringify(simulateResult, null, 2) }}
          ]
        }};
      }}

      if (signAndSubmit) {{
        if (!secretKey) {{
          throw new Error("secretKey is required when signAndSubmit is true");
        }}
        
        // Sign the transaction with the provided secret key
        transaction.sign(Keypair.fromSecret(secretKey));
        
        // Submit the transaction
        const submittedTx = await server.sendTransaction(transaction);
        
        return {{
          content: [
            {{ type: "text", text: "Transaction submitted!" }},
            {{ type: "text", text: `Transaction hash: ${{submittedTx.hash}}` }},
            {{ type: "text", text: "Full response:" }},
            {{ type: "text", text: JSON.stringify(submittedTx, null, 2) }}
          ]
        }};
      }}

      // If neither simulate nor signAndSubmit, return the XDR
      return {{
        content: [
          {{ type: "text", text: "Transaction XDR:" }},
          {{ type: "text", text: xdr }}
        ]
      }};
    }} catch (error: any) {{
      return {{
        content: [{{ 
          type: "text", 
          text: `Error executing {}: ${{error.message}}${{error.cause ? `\nCause: ${{error.cause}}` : ''}}` 
        }}]
      }};
    }}
  }}
);"#,
                    name, description, params,
                    if has_params { ", ...functionParams" } else { "" },
                    if has_params {
                        format!("\n      // Ensure parameters are in the correct order as defined in the contract\n      const orderedParams = [{}];\n      const scValParams = orderedParams.map(paramName => {{\n        const value = functionParams[paramName as keyof typeof functionParams];\n        if (value === undefined) {{\n          throw new Error(`Missing required parameter: ${{paramName}}`);\n        }}\n        return nativeToScVal(value);\n      }});",
                            inputs.iter().map(|input| format!("'{}'", input.name)).collect::<Vec<_>>().join(", "))
                    } else {
                        String::new()
                    },
                    name,
                    if has_params { ", ...scValParams" } else { "" },
                    name
                ))
            }
            _ => None,
        }
    }
} 