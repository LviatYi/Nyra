use std::{env, fs, path::PathBuf};

const RUNTIME_FILES: [&str; 5] = [
    "runner.ts",
    "sdk.ts",
    "context.d.ts",
    "bun.json",
    "bunfig.toml",
];

macro_rules! define_sdk {
    (
        types {
            $(
                $(#[doc = $type_doc:literal])*
                struct $type_name:ident {
                    $(
                        $(#[doc = $field_doc:literal])*
                        $field:ident: $field_type:ty
                    ),* $(,)?
                }
            )*
        }
        context $context:ident {
            properties {
                $(
                    $(#[doc = $property_doc:literal])*
                    readonly $property:ident: $property_type:ty;
                )*
            }
            methods {
                $(
                    $(#[doc = $method_doc:literal])*
                    $method:literal(
                        $run_id:ident, $line:ident;
                        $($argument:ident: $argument_type:ty),* $(,)?
                    ) $body:block
                )*
            }
        }
    ) => {
        fn generate_context_declaration() -> String {
            let mut output = String::from(
                "// AUTO-GENERATED from src/reaction/sdk.rs by build.rs.\n\
                 // Run `cargo check` (or any Cargo build) to regenerate. Do not edit directly.\n\n",
            );
            $(
                push_docs(&mut output, "", &[$($type_doc),*]);
                output.push_str("export interface ");
                output.push_str(stringify!($type_name));
                output.push_str(" {\n");
                $(
                    push_docs(&mut output, "    ", &[$($field_doc),*]);
                    output.push_str("    readonly ");
                    output.push_str(&lower_camel(stringify!($field)));
                    output.push_str(": ");
                    output.push_str(&typescript_type(stringify!($field_type)));
                    output.push_str(";\n");
                )*
                output.push_str("}\n\n");
            )*
            output.push_str("/** Context supplied to the default macro entry point. */\n");
            output.push_str("export interface ");
            output.push_str(stringify!($context));
            output.push_str(" {\n");
            $(
                push_docs(&mut output, "    ", &[$($property_doc),*]);
                output.push_str("    readonly ");
                output.push_str(&lower_camel(stringify!($property)));
                output.push_str(": ");
                output.push_str(&typescript_type(stringify!($property_type)));
                output.push_str(";\n");
            )*
            $(
                push_docs(&mut output, "    ", &[$($method_doc),*]);
                output.push_str("    ");
                output.push_str($method);
                output.push('(');
                let mut separator = "";
                $(
                    output.push_str(separator);
                    separator = ", ";
                    output.push_str(&lower_camel(stringify!($argument)));
                    output.push_str(": ");
                    output.push_str(&typescript_type(stringify!($argument_type)));
                )*
                let _ = separator;
                output.push_str("): void;\n");
                let _ = stringify!($run_id);
                let _ = stringify!($line);
                let _ = stringify!($body);
            )*
            output.push_str("}\n");
            output
        }
    };
}

include!("src/reaction/sdk.rs");

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/reaction/sdk.rs");
    for file in RUNTIME_FILES {
        if file != "context.d.ts" {
            println!("cargo:rerun-if-changed=runtime/{file}");
        }
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let path = manifest_dir.join("runtime/context.d.ts");
    let declaration = generate_context_declaration();
    if fs::read_to_string(&path).ok().as_deref() != Some(&declaration) {
        fs::write(&path, declaration).expect("failed to generate runtime/context.d.ts");
    }

    let mut profile_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for _ in 0..3 {
        profile_dir.pop();
    }
    let runtime_dir = profile_dir.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("failed to create Cargo runtime directory");
    for file in RUNTIME_FILES {
        copy_if_changed(
            &manifest_dir.join("runtime").join(file),
            &runtime_dir.join(file),
        );
    }
}

fn copy_if_changed(source: &std::path::Path, destination: &std::path::Path) {
    let contents = fs::read(source)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source.display()));
    if fs::read(destination).ok().as_deref() != Some(contents.as_slice()) {
        fs::write(destination, contents)
            .unwrap_or_else(|error| panic!("failed to write {}: {error}", destination.display()));
    }
}

fn push_docs(output: &mut String, indent: &str, docs: &[&str]) {
    if docs.is_empty() {
        return;
    }
    output.push_str(indent);
    output.push_str("/** ");
    for (index, doc) in docs.iter().enumerate() {
        if index > 0 {
            output.push(' ');
        }
        output.push_str(doc.trim());
    }
    output.push_str(" */\n");
}

fn lower_camel(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    let mut uppercase = false;
    for character in name.chars() {
        if character == '_' {
            uppercase = true;
        } else if uppercase {
            output.extend(character.to_uppercase());
            uppercase = false;
        } else {
            output.push(character);
        }
    }
    output
}

fn typescript_type(rust_type: &str) -> String {
    let compact = rust_type.replace(' ', "");
    match compact.as_str() {
        "String" | "str" | "&str" | "char" => "string".into(),
        "bool" => "boolean".into(),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" | "f32" | "f64" => "number".into(),
        "serde_json::Value" => "unknown".into(),
        _ if compact.starts_with("Option<") && compact.ends_with('>') => {
            format!("{} | null", typescript_type(&compact[7..compact.len() - 1]))
        }
        _ if compact.starts_with("Vec<") && compact.ends_with('>') => {
            format!("Array<{}>", typescript_type(&compact[4..compact.len() - 1]))
        }
        _ => compact,
    }
}
