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
        let _xdr_converter = type_to_js_xdr(value);
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

    pub fn generate(&self, output_dir: &Path, name: &str, spec: &[ScSpecEntry], contract_id: &str) -> Result<(), Error> {
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

        // Replace imports section with our enhanced version
        let imports = r#"import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { Contract, nativeToScVal, xdr, TransactionBuilder, SorobanRpc, Keypair, Address, BASE_FEE } from '@stellar/stellar-sdk';
import { z } from 'zod';
import { 
  addressToScVal, 
  i128ToScVal, 
  u128ToScVal, 
  stringToSymbol, 
  numberToU64, 
  numberToI128, 
  boolToScVal, 
  u32ToScVal,
  submitTransaction 
} from './helper.js';"#;

        index_content = index_content.replace(
            r#"import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { Contract, nativeToScVal, xdr, TransactionBuilder, SorobanRpc, Keypair } from '@stellar/stellar-sdk';
import { z } from 'zod';"#,
            imports
        );

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
{}    simulate: z.boolean().optional().default(true).describe("If true, simulate the transaction instead of submitting it"),
    signAndSubmit: z.boolean().optional().default(false).describe("If true, sign and submit the transaction"),
    publicKey: z.string().describe("Public key in strkey format (G... for public keys) - Converts to: addressToScVal(i)"),
    secretKey: z.string().optional().describe("Secret key in strkey format (S... for secret keys)")
  }},
  async (params) => {{
    try {{
      const {{ simulate, signAndSubmit, publicKey, secretKey{} }} = params;{}
      
      // Get the account
      const account = await server.getAccount(publicKey);
      const transaction = new TransactionBuilder(account, {{
        networkPassphrase: config.networkPassphrase,
        fee: BASE_FEE,
      }})
      .addOperation(contract.call("{}"{}))
      .setTimeout(30)
      .build();

      if (!simulate) {{
        return {{
          content: [
            {{ type: "text", text: "Transaction XDR:" }},
            {{ type: "text", text: transaction.toXDR() }}
          ]
        }};
      }}

      // Prepare the transaction
      const preparedTx = await server.prepareTransaction(transaction);
      const preparedXdr = preparedTx.toXDR();

      // Simulate the transaction using the server
      const simulateResult = await server.simulateTransaction(preparedTx);
      if(!signAndSubmit) {{
        return {{
          content: [
            {{ type: "text", text: "Transaction XDR" }},
            {{ type: "text", text: preparedXdr }}
          ]
        }};
      }}

      if(!secretKey) {{
        throw new Error("secretKey is required when signAndSubmit is true");
      }}

      const keypair = Keypair.fromSecret(secretKey);
      const tx = TransactionBuilder.fromXDR(preparedXdr, config.networkPassphrase);
      tx.sign(keypair);
      
      try {{
        const result = await submitTransaction(tx.toXDR(), {{
          server,
          networkPassphrase: config.networkPassphrase
        }});
        
        if (result.status === 'SUCCESS') {{
          return {{
            content: [
              {{ type: "text", text: "Transaction completed successfully!" }},
              {{ type: "text", text: `Transaction hash: ${{result.hash}}` }},
              {{ type: "text", text: "Full response:" }},
              {{ type: "text", text: JSON.stringify(result.response, null, 2) }}
            ]
          }};
        }} else if (result.status === 'FAILED') {{
          throw new Error(`Transaction failed: ${{result.resultXdr}}`);
        }} else {{
          // Instead of throwing an error on timeout, return a message with explorer link
          return {{
            content: [
              {{ type: "text", text: "Transaction submitted but still processing." }},
              {{ type: "text", text: `Transaction hash: ${{result.hash}}` }},
              {{ type: "text", text: `You can view the transaction status at: https://stellar.expert/explorer/${{config.network === 'testnet' ? 'testnet' : 'public'}}/tx/${{result.hash}}` }}
            ]
          }};
        }}
        
      }} catch (error: any) {{
        return {{
          content: [
            {{ type: "text", text: "Transaction failed!" }},
            {{ type: "text", text: error.message }},
            {{ type: "text", text: "Please check the transaction hash on the network explorer." }}
          ]
        }};
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
                    if has_params { ", ...functionParams" } else { "" },
                    if has_params {
                        format!(r#"
      // Ensure parameters are in the correct order as defined in the contract
      const orderedParams = [{}];
      const scValParams = orderedParams.map(paramName => {{
        const value = functionParams[paramName as keyof typeof functionParams];
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
                                format!("case '{}':\n            return {}(value as {});",
                                    input.name,
                                    match input.value {
                                        Type::Address => "addressToScVal",
                                        Type::I128 => "i128ToScVal",
                                        Type::U128 => "u128ToScVal",
                                        Type::U32 => "u32ToScVal",
                                        Type::Bool => "boolToScVal",
                                        Type::Symbol => "stringToSymbol",
                                        _ => "nativeToScVal",
                                    },
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
                    if has_params { ", ...scValParams" } else { "" },
                    name
                ))
            }
            _ => None,
        }
    }
} 