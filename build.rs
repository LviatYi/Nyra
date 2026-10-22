use std::{
    any::TypeId,
    collections::{BTreeMap, HashSet},
    env, fs,
    path::PathBuf,
};

use sdk_types::*;
use ts_rs::{Config, TS, TypeVisitor};

#[allow(dead_code)]
#[path = "src/reaction/sdk_types.rs"]
mod sdk_types;

const RUNTIME_FILES: [&str; 5] = [
    "runner.ts",
    "sdk.ts",
    "context.d.ts",
    "bun.json",
    "bunfig.toml",
];

// Only the method interface is project-specific. Data shapes come from ts-rs.
macro_rules! define_sdk {
    (
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
                        $execution:ident, $run_id:ident, $line:ident;
                        $($argument:ident: $argument_type:ty),* $(,)?
                    ) $body:block
                )*
            }
        }
    ) => {
        fn generate_context_declaration() -> String {
            let config = Config::new().with_large_int("number");
            let mut output = String::from(
                "// AUTO-GENERATED from src/reaction/sdk_types.rs and sdk.rs by build.rs.\n\
                 // Run `cargo check` (or any Cargo build) to regenerate. Do not edit directly.\n\n",
            );
            let mut types = TypeDeclarations {
                config: &config,
                visited: HashSet::new(),
                declarations: BTreeMap::new(),
            };
            $(types.visit::<$property_type>();)*
            $($(types.visit::<$argument_type>();)*)*
            for declaration in types.declarations.into_values() {
                output.push_str(&declaration);
            }

            output.push_str("/** Context supplied to the default macro entry point. */\n");
            output.push_str("export interface ");
            output.push_str(stringify!($context));
            output.push_str(" {\n");
            $(
                push_docs(&mut output, "    ", &[$($property_doc),*]);
                output.push_str("    readonly ");
                output.push_str(&lower_camel(stringify!($property)));
                output.push_str(": ");
                output.push_str(&<$property_type as TS>::name(&config));
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
                    output.push_str(&<$argument_type as TS>::name(&config));
                )*
                let _ = separator;
                output.push_str("): void;\n");
                let _ = stringify!($execution);
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
    println!("cargo:rerun-if-changed=src/reaction/sdk_types.rs");
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

struct TypeDeclarations<'a> {
    config: &'a Config,
    visited: HashSet<TypeId>,
    declarations: BTreeMap<String, String>,
}

impl TypeVisitor for TypeDeclarations<'_> {
    fn visit<T: TS + 'static + ?Sized>(&mut self) {
        if !self.visited.insert(TypeId::of::<T>()) {
            return;
        }
        if T::output_path().is_some() {
            let mut declaration = String::new();
            push_type::<T>(&mut declaration, self.config);
            let name = T::name(self.config);
            assert!(
                self.declarations
                    .insert(name.clone(), declaration)
                    .is_none(),
                "duplicate TypeScript type name: {name}",
            );
        }
        T::visit_dependencies(self);
    }
}

fn push_type<T: TS + ?Sized>(output: &mut String, config: &Config) {
    if let Some(docs) = T::docs() {
        output.push_str(&docs);
    }
    output.push_str("export ");
    output.push_str(&T::decl(config));
    output.push_str("\n\n");
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
        } else if output.is_empty() {
            output.extend(character.to_lowercase());
        } else {
            output.push(character);
        }
    }
    output
}
