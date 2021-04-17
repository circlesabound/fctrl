use std::{env, fs::{self, File}, path::{Path, PathBuf}};
use std::io::Write;
use std::process::Command;

fn main() {
    let proj_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("openapi");
    for entry in fs::read_dir(proj_dir).unwrap() {
        let entry = entry.unwrap();
        if let Ok(ft) = entry.file_type() {
            if ft.is_file() {
                let path = entry.path();
                let ext = path.extension().unwrap_or_default();
                if ext == "yaml" || ext == "yml" || ext == "json" {
                    let doc_name = path.file_stem().unwrap();
                    codegen_and_concat(&path, doc_name.to_str().unwrap());
                }
            }
        }
    }
    println!("cargo:rerun-if-changed=openapi");
}

/// Given an OpenAPI spec file, generate models at {openapi_doc_name}.rs
fn codegen_and_concat<P: AsRef<Path>>(openapi_file: P, openapi_doc_name: &str) {
    let base_out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let intermediate_out_dir = base_out_dir.join(openapi_doc_name);
    let final_out_file = base_out_dir.join(format!("{}.rs", openapi_doc_name));
    codegen(openapi_file, &intermediate_out_dir);
    generate_single_include(intermediate_out_dir, final_out_file)
}

fn codegen<P1: AsRef<Path>, P2: AsRef<Path>>(spec_file: P1, out_dir: P2) {
    let _ = fs::remove_dir_all(out_dir.as_ref());
    let _ = Command::new("npx")
        .arg("openapi-generator-cli")
        .arg("generate")
        .args(&[ "--global-property", "models,modelDocs=false" ])
        .args(&[ "-g", "rust" ])
        .arg("-i")
        .arg(spec_file.as_ref())
        .arg("-o")
        .arg(out_dir.as_ref())
        .status()
        .unwrap();
}

fn generate_single_include<P1: AsRef<Path>, P2: AsRef<Path>>(from_dir: P1, out_file: P2) {
    let mut single = File::create(out_file).unwrap();
    writeln!(&mut single, "use serde::{{Deserialize, Serialize}};").unwrap();
    for file in fs::read_dir(PathBuf::from(from_dir.as_ref()).join("src").join("models")).unwrap() {
        let file = file.unwrap();
        if file.file_name().to_str().unwrap().ends_with(".rs") {
            // let mod_name = file.file_name().to_str().unwrap().trim_end_matches(".rs");
            let contents = fs::read_to_string(file.path()).unwrap();
            // writeln!(&mut single, "mod {} {{\n{}\n}}", mod_name, contents).unwrap();
            writeln!(&mut single, "{}", contents).unwrap();
        }
    }
}
