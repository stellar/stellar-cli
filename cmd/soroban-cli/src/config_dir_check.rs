use crate::{config::locator::Args as Locator, print::Print};

pub async fn config_dir_check(locator: Locator, printer: Print) {
    let Ok(config_dir) = locator.config_dir() else {
        return;
    };

    // Do not bother warning users when the config directory doesn't exist.
    if !config_dir.exists() {
        return;
    };

    let Some(dirname) = config_dir.file_name() else {
        return;
    };

    let Some(parent_dir) = config_dir.parent() else {
        return;
    };

    let new_config_dir = if locator.global {
        parent_dir.join("stellar")
    } else {
        parent_dir.join(".stellar")
    };

    let message = format!(
        "The config directory {config_dir:?} is deprecated and should be \
        renamed to {new_config_dir:?}.",
    );

    if (locator.global && dirname == "soroban") || (!locator.global && dirname == ".soroban") {
        printer.warnln(message);
    }
}
