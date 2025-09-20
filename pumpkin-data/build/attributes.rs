use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;

#[derive(Deserialize)]
struct Attribute {
    id: u8,
    default_value: f64,
}

pub(crate) fn build() -> TokenStream {
    println!("cargo:rerun-if-changed=../assets/attributes.json");

    let attributes: BTreeMap<String, Attribute> =
        serde_json::from_str(&fs::read_to_string("../assets/attributes.json").unwrap())
            .expect("Failed to parse attributes.json");

    let mut consts = TokenStream::new();
    let mut name_to_attr = TokenStream::new();
    let mut id_to_fallback = TokenStream::new();

    let mut data_component_vec = attributes.iter().collect::<Vec<_>>();
    data_component_vec.sort_by_key(|(_, i)| i.id);

    for (raw_name, raw_value) in &data_component_vec {
        let pascal_case = format_ident!("{}", raw_name.to_uppercase());
        // using minecraft namespace to avoid conflicts with potential future plugin namespaces
        let qualified_name = format!("minecraft:{raw_name}");

        let id = raw_value.id;
        let default_value = raw_value.default_value;
        consts.extend(quote! {
            pub const #pascal_case: Self = Self(#id);
        });

        name_to_attr.extend(quote! {
            #qualified_name => Some(Attribute(#id)),
        });

        id_to_fallback.extend(quote! {
            #id => #default_value,
        });
    }

    quote! {
        use std::hash::Hash;
        #[derive(Clone, Copy, Debug)]
        pub struct Attribute(u8);
        impl PartialEq for Attribute {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        impl Eq for Attribute {}
        impl Hash for Attribute {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }
        impl Attribute {
            pub fn find_by_name(name: &str) -> Option<Attribute> {
                match name {
                    #name_to_attr
                    _ => None
                }
            }

            pub fn get_fallback(&self) -> f64 {
                match self.0 {
                    #id_to_fallback
                    _ => panic!("Attribute with id {} does not have a fallback value", self.0)
                }
            }

            #consts
        }
    }
}
