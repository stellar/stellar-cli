use std::{
    env,
    path::{Path, PathBuf},
};
// use clap::{Command, Arg};
// use clap_markdown::MarkdownOptions;
type DynError = Box<dyn std::error::Error>;

fn main() -> Result<(), DynError> {
    doc_gen()?;
    Ok(())
}

fn doc_gen() -> std::io::Result<()> {
    let out_dir = project_root();
    let options = clap_markdown::MarkdownOptions::new()
        .show_footer(false)
        .show_table_of_contents(false)
        .title("Stellar CLI Manual".to_string());

        let content = generate_markdown_with_aliases::<soroban_cli::Root>(&options);

    std::fs::write(out_dir.join("FULL_HELP_DOCS.md"), content)?;

    Ok(())
}

fn generate_markdown_with_aliases<C: clap::CommandFactory>(options: &clap_markdown::MarkdownOptions) -> String {
    let command = C::command();
    let mut markdown_content = clap_markdown::help_markdown_custom::<C>(options);
    add_aliases_recursively(&command, &mut markdown_content, 2);
    markdown_content
}

fn add_aliases_recursively(command: &clap::Command, content: &mut String, level: usize) {
    let header = "#".repeat(level);
    
    // Add command aliases
    let cmd_aliases: Vec<_> = command.get_visible_aliases().collect();
    if !cmd_aliases.is_empty() {
        content.push_str(&format!("\n\n{} Aliases for `{}`: {}\n", 
            header, command.get_name(), cmd_aliases.join(", ")));
    }
    
    for arg in command.get_arguments() {
        // Assuming `arg.get_short_and_visible_aliases()` returns Vec<Vec<char>> 
        let arg_aliases: Vec<String> = arg.get_short_and_visible_aliases()
            .into_iter()
            .map(|alias| alias.iter().collect::<String>()) // Convert Vec<char> to String
            .collect();
    
        if !arg_aliases.is_empty() {
            content.push_str(&format!(
                "\n\n**Aliases for argument `{}`**: {}\n", 
                arg.get_id(), 
                arg_aliases.join(", ")
            ));
        }
    }
    
    // Recursively process subcommands
    for subcommand in command.get_subcommands() {
        add_aliases_recursively(subcommand, content, level + 1);
    }
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}
