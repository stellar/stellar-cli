use std::{fs, io, path::Path};
use stellar_xdr::curr::ScSpecEntry;
use crate::types::{Entry, Type};
use crate::{type_to_ts, wrapper::type_to_js_xdr};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

pub struct McpServerGenerator {
    used_imports: std::collections::HashSet<String>,
}

impl McpServerGenerator {
    pub fn new() -> Self {
        Self {
            used_imports: std::collections::HashSet::new(),
        }
    }

    fn add_import(&mut self, import: &str) {
        self.used_imports.insert(import.to_string());
    }

    fn get_imports(&self) -> String {
        let mut imports = vec![
            // Base imports that are always needed
            "import { McpServer } from \"@modelcontextprotocol/sdk/server/mcp.js\";",
            "import { StdioServerTransport } from \"@modelcontextprotocol/sdk/server/stdio.js\";",
            "import { z } from 'zod';",
            "import { config as dotenvConfig } from 'dotenv';",
        ];

        // Add stellar-sdk imports based on usage
        let mut stellar_imports = vec!["Contract", "nativeToScVal", "xdr", "rpc as SorobanRpc"];
        if self.used_imports.contains("Address") { stellar_imports.push("Address"); }
        if self.used_imports.contains("BASE_FEE") { stellar_imports.push("BASE_FEE"); }
        if self.used_imports.contains("Keypair") { stellar_imports.push("Keypair"); }
        
        imports.push(&format!(
            "import {{ {} }} from '@stellar/stellar-sdk';",
            stellar_imports.join(", ")
        ));

        // Add helper imports based on usage
        let mut helper_imports = vec!["createSACClient"];
        if self.used_imports.contains("addressToScVal") { helper_imports.push("addressToScVal"); }
        if self.used_imports.contains("i128ToScVal") { helper_imports.push("i128ToScVal"); }
        if self.used_imports.contains("u128ToScVal") { helper_imports.push("u128ToScVal"); }
        if self.used_imports.contains("stringToSymbol") { helper_imports.push("stringToSymbol"); }
        if self.used_imports.contains("numberToU64") { helper_imports.push("numberToU64"); }
        if self.used_imports.contains("numberToI128") { helper_imports.push("numberToI128"); }
        if self.used_imports.contains("boolToScVal") { helper_imports.push("boolToScVal"); }
        if self.used_imports.contains("u32ToScVal") { helper_imports.push("u32ToScVal"); }
        if self.used_imports.contains("submitTransaction") { helper_imports.push("submitTransaction"); }

        imports.push(&format!(
            "import {{ {} }} from './helper.js';",
            helper_imports.join(",\n  ")
        ));

        imports.join("\n")
    }

    fn type_to_zod(&mut self, value: &Type) -> String {
        let _xdr_converter = type_to_js_xdr(value);
        match value {
            // Numbers
            Type::U64 | Type::I64 | Type::U32 | Type::I32 | Type::Timepoint | Type::Duration => {
                self.add_import("numberToU64");
                "z.number()".to_string()
            },

            // Large numbers and addresses as strings
            Type::U128 => {
                self.add_import("u128ToScVal");
                "z.string()".to_string()
            },
            Type::I128 => {
                self.add_import("i128ToScVal");
                "z.string()".to_string()
            },
            Type::Address => {
                self.add_import("addressToScVal");
                "z.string()".to_string()
            },
            Type::Symbol => {
                self.add_import("stringToSymbol");
                "z.string()".to_string()
            },
            Type::String => "z.string()".to_string(),

            // Boolean
            Type::Bool => {
                self.add_import("boolToScVal");
                "z.boolean()".to_string()
            },

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
            Type::Option { value } => format!("{}.optional()", self.type_to_zod(value)),
            Type::Vec { element } => format!("z.array({})", self.type_to_zod(element)),
            Type::Map { key, value } => format!("z.map({}, {})", self.type_to_zod(key), self.type_to_zod(value)),
            Type::Tuple { elements } => {
                let element_types = elements.iter()
                    .map(|e| self.type_to_zod(e))
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
        let _xdr_converter = type_to_js_xdr(value);
        match value {
            Type::U64 => format!("Unsigned 64-bit integer (0 to 18,446,744,073,709,551,615) - Converts to: {}", _xdr_converter),
            Type::I64 => format!("Signed 64-bit integer (-9,223,372,036,854,775,808 to 9,223,372,036,854,775,807) - Converts to: {}", _xdr_converter),
            Type::U32 => format!("Unsigned 32-bit integer (0 to 4,294,967,295) - Converts to: {}", _xdr_converter),
            Type::I32 => format!("Signed 32-bit integer (-2,147,483,648 to 2,147,483,647) - Converts to: {}", _xdr_converter),
            Type::U128 => format!("Unsigned 128-bit integer as string (0 to 340,282,366,920,938,463,463,374,607,431,768,211,455) - Converts to: {}", _xdr_converter),
            Type::I128 => format!("Signed 128-bit integer as string (-170,141,183,460,469,231,731,687,303,715,884,105,728 to 170,141,183,460,469,231,731,687,303,715,884,105,727) - Converts to: {}", _xdr_converter),
            Type::U256 => format!("Unsigned 256-bit integer as string - Converts to: {}", _xdr_converter),
            Type::I256 => format!("Signed 256-bit integer as string - Converts to: {}", _xdr_converter),
            Type::Address => format!("Stellar address in strkey format (G... for public keys, C... for contract) - Converts to: {}", _xdr_converter),
            Type::Symbol => format!("Stellar symbol/enum value - Converts to: {}", _xdr_converter),
            Type::String => format!("UTF-8 encoded string - Converts to: {}", _xdr_converter),
            Type::Timepoint => format!("Unix timestamp in seconds - Converts to: {}", _xdr_converter),
            Type::Duration => format!("Time duration in seconds - Converts to: {}", _xdr_converter),
            Type::Bool => format!("Boolean value (true/false) - Converts to: {}", _xdr_converter),
            Type::Bytes => format!("Binary data as Buffer, byte array, or base64 string - Converts to: {}", _xdr_converter),
            Type::BytesN { n } => format!("Fixed-length binary data of {} bytes as Buffer, byte array, or base64 string - Converts to: {}", n, _xdr_converter),
            Type::Option { value } => format!("Optional {} - Converts to: {}", type_to_ts(value), _xdr_converter),
            Type::Vec { element } => format!("Array of {} - Converts to: {}", type_to_ts(element), _xdr_converter),
            Type::Map { key, value } => format!("Map of {} to {} - Converts to: {}", type_to_ts(key), type_to_ts(value), _xdr_converter),
            Type::Tuple { elements } => format!("Tuple of [{}] - Converts to: {}", elements.iter().map(type_to_ts).collect::<Vec<_>>().join(", "), _xdr_converter),
            Type::Custom { name } => format!("Custom type: {} - Converts to: {}", name, _xdr_converter),
            _ => format!("Any value - Converts to: {}", _xdr_converter),
        }
    }

    pub fn generate(&mut self, output_dir: &Path, name: &str, spec: &[ScSpecEntry], contract_id: &str) -> Result<(), Error> {
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

        // Read the template files
        let template_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/mcp_server_template");
        let template = fs::read_to_string(template_dir.join("src/index.ts"))?;
        let helper_content = fs::read_to_string(template_dir.join("src/helper.ts"))?;
        
        // Replace placeholders in the template
        let mut index_content = template
            .replace("INSERT_NAME_HERE", name)
            .replace("INSERT_TOOLS_HERE", &tools);

        // Replace imports section with our dynamic imports
        let old_imports = r#"import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { Contract, nativeToScVal, xdr, TransactionBuilder, SorobanRpc, Keypair } from '@stellar/stellar-sdk';
import { z } from 'zod';"#;

        index_content = index_content.replace(old_imports, &self.get_imports());

        // Write the generated code to index.ts
        let index_path = output_dir.join("src/index.ts");
        fs::create_dir_all(index_path.parent().unwrap())?;
        fs::write(index_path, index_content)?;

        // Write helper.ts
        fs::write(output_dir.join("src/helper.ts"), helper_content)?;

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

        // Print success message with next steps
        println!("\n✨ Generated MCP server in {}", output_dir.display());
        println!("\n📝 Next steps:");
        println!("1. Install dependencies and build the project:");
        println!("   ```");
        println!("   cd {}", output_dir.display());
        println!("   npm install");
        println!("   npm run build");
        println!("   ```");
        println!("\n2. Set up your environment variables:");
        println!("   ```");
        println!("   cp .env.example .env");
        println!("   # Edit .env with your configuration");
        println!("   ```");
        println!("\n3. Add the following configuration to your MCP config file:");
        println!("   ```json");
        println!("   \"{}\": {{", name);
        println!("     \"command\": \"node\",");
        println!("     \"args\": [");
        println!("       \"{}/build/index.js\"", output_dir.display());
        println!("     ],");
        println!("     \"env\": {{");
        println!("       \"NETWORK\": \"testnet\",");
        println!("       \"NETWORK_PASSPHRASE\": \"Test SDF Network ; September 2015\",");
        println!("       \"RPC_URL\": \"https://soroban-testnet.stellar.org\",");
        println!("       \"CONTRACT_ID\": \"{}\"", contract_id);
        println!("     }}");
        println!("   }}");
        println!("   ```");
        println!("\n📚 For more information, check the README.md file in the generated project.");

        Ok(())
    }

    fn generate_tool(&mut self, entry: &ScSpecEntry) -> Option<String> {
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
                            let type_info = self.type_to_zod(&input.value);
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
{}  }},
  async (params) => {{
    try {{
      {}
      // Get the SAC client
      const sacClient = await createSACClient(config.contractId, config.networkPassphrase, config.rpcUrl);

      let txXdr: string;
      const functionName = '{}';

      const functionToCall = sacClient[functionName];
      const result = await functionToCall(params);
      txXdr = result.toXDR();

      return {{
        content: [
          {{ type: "text", text: "UnsignedTransaction XDR:" }},
          {{ type: "text", text: txXdr }},
          {{ type: "text", text: "Next steps:" }},
          {{ type: "text", text: "1. Sign the transaction" }},
          {{ type: "text", text: "2. Submit the transaction" }},
        ]
      }}
      
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
                    name,
                    description,
                    params,
                    if has_params {
                        format!(r#"
      // Ensure parameters are in the correct order as defined in the contract
      const orderedParams = [{}];
      const scValParams = orderedParams.map(paramName => {{
        const value = params[paramName as keyof typeof params];
        if (value === undefined) {{
          throw new Error(`Missing required parameter: ${{paramName}}`);
        }}
        // Use appropriate conversion based on parameter type
        switch(paramName) {{
          {}
          default:
            return nativeToScVal(value);
        }}
      }});"#,
                            inputs.iter().map(|input| format!("'{}'", input.name)).collect::<Vec<_>>().join(", "),
                            inputs.iter().map(|input| {
                                let converter = match input.value {
                                    Type::Address => {
                                        self.add_import("addressToScVal");
                                        "addressToScVal"
                                    },
                                    Type::I128 => {
                                        self.add_import("i128ToScVal");
                                        "i128ToScVal"
                                    },
                                    Type::U128 => {
                                        self.add_import("u128ToScVal");
                                        "u128ToScVal"
                                    },
                                    Type::U32 => {
                                        self.add_import("u32ToScVal");
                                        "u32ToScVal"
                                    },
                                    Type::Bool => {
                                        self.add_import("boolToScVal");
                                        "boolToScVal"
                                    },
                                    Type::Symbol => {
                                        self.add_import("stringToSymbol");
                                        "stringToSymbol"
                                    },
                                    _ => "nativeToScVal",
                                };
                                format!("case '{}':\n            return {}(value as {});",
                                    input.name,
                                    converter,
                                    match input.value {
                                        Type::Address => "string",
                                        Type::I128 | Type::U128 => "string",
                                        Type::U32 => "number",
                                        Type::Bool => "boolean",
                                        Type::Symbol => "string",
                                        _ => "any",
                                    }
                                )
                            }).collect::<Vec<_>>().join("\n          ")
                        )
                    } else {
                        String::new()
                    },
                    name,
                    name
                ))
            }
            _ => None,
        }
    }
} 