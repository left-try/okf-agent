use std::{fs, path::Path};

pub fn create(root: &Path, file_count: usize) -> std::io::Result<()> {
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"index-bench\"\nversion = \"0.1.0\"\n",
    )?;
    for index in 0..file_count {
        let module = if index > 0 {
            format!("use crate::module_{};\n", index - 1)
        } else {
            String::new()
        };
        let content = format!(
            "{module}pub struct BenchType{index} {{ pub value: usize }}\npub fn bench_function_{index}() -> usize {{ {} }}\n// marker benchmark_search_token\n",
            index % 97
        );
        fs::write(
            root.join("src").join(format!("module_{index:05}.rs")),
            content,
        )?;
    }
    Ok(())
}
