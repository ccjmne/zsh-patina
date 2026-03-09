use std::{env, io::Cursor, path::PathBuf};

use anyhow::{Context, Result};
use syntect::{
    dumps::{dump_to_file, dump_to_uncompressed_file},
    highlighting::ThemeSet,
    parsing::SyntaxSetBuilder,
};

fn main() -> Result<()> {
    let out_dir = env::var_os("OUT_DIR").unwrap();

    // load shell syntax into a syntax set and dump it to a file
    let mut syntax_set_builder = SyntaxSetBuilder::new();
    syntax_set_builder.add_from_folder("assets/Packages/ShellScript", true)?;
    let syntax_set = syntax_set_builder.build();
    let syntax_dest_path = PathBuf::from(&out_dir).join("syntax_set.packdump");
    dump_to_uncompressed_file(&syntax_set, syntax_dest_path)
        .context("Unable to dump syntax to file")?;

    // load theme and dump it to a compressed file
    let theme_str = include_str!("assets/theme.tmTheme");
    let mut cursor = Cursor::new(theme_str);
    let theme = ThemeSet::load_from_reader(&mut cursor).expect("Unable to load theme");
    let theme_dest_path = PathBuf::from(&out_dir).join("theme.themedump");
    dump_to_file(&theme, theme_dest_path).context("Unable to dump theme to compressed file")?;

    Ok(())
}
