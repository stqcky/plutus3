use darling::{Error, FromDeriveInput, FromField, FromMeta};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use syn::{DeriveInput, parse::Parser, parse_macro_input};

#[derive(FromMeta, Debug, Clone, Copy)]
struct Slot(u64);

#[derive(FromMeta, Debug, Clone)]
struct ImmutableAttribute(syn::Path);

#[derive(Debug, FromField, Clone)]
#[darling(attributes(contract))]
struct SmartContractField {
    ident: Option<syn::Ident>,
    vis: syn::Visibility,
    ty: syn::Type,

    slot: Option<Slot>,
    immutable: Option<ImmutableAttribute>,
}

#[derive(Debug, FromDeriveInput, Clone)]
#[darling(supports(struct_named))]
struct SmartContractStruct {
    ident: syn::Ident,
    vis: syn::Visibility,
    generics: syn::Generics,
    data: darling::ast::Data<(), SmartContractField>,
}

struct SlotField {
    slot: u64,
    ident: syn::Ident,
}

struct ImmutableField {
    call: syn::Path,
    ident: syn::Ident,
}

pub fn smart_contract_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let smart_contract = match SmartContractStruct::from_derive_input(&input) {
        Ok(x) => x,
        Err(e) => return TokenStream::from(Error::from(e).write_errors()),
    };

    smart_contract.to_token_stream().into()
}

impl ToTokens for SmartContractStruct {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let SmartContractStruct {
            ident,
            vis,
            generics,
            data,
            ..
        } = self;

        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        let fields = data.as_ref().take_struct().unwrap().fields;

        let slot_fields = get_slot_fields(&fields);
        let immutable_fields = get_immutable_fields(&fields);

        let smart_contract_impl =
            create_smart_contract_impl(ident, &slot_fields, &immutable_fields);

        let mut fields: Vec<_> = fields
            .into_iter()
            .map(|field| {
                let SmartContractField { ident, ty, vis, .. } = field;

                syn::Field::parse_named
                    .parse2(quote! { #vis #ident: #ty })
                    .expect("field is correct")
            })
            .collect();

        fields.insert(0, create_address_field());
        fields.insert(0, create_storage_field());

        tokens.extend(quote! {
            #vis struct #impl_generics #ident #ty_generics #where_clause {
                #(#fields),*
            }

            #smart_contract_impl
        });
    }
}

fn create_address_field() -> syn::Field {
    syn::Field::parse_named
        .parse2(quote! { pub address: ::plutus_evm::revm::primitives::Address })
        .expect("address field is correct")
}

fn create_storage_field() -> syn::Field {
    syn::Field::parse_named
        .parse2(quote! { __storage: ::plutus_evm::storage::SmartContractStorage })
        .expect("storage field is correct")
}

fn create_smart_contract_impl(
    ident: &syn::Ident,
    slot_fields: &[SlotField],
    immutable_fields: &[ImmutableField],
) -> TokenStream2 {
    let new_impl = create_new(slot_fields, immutable_fields);
    let decoder = create_decoder(slot_fields);

    quote! {
        impl ::plutus_evm::contract::SmartContract for #ident {
            #new_impl
            #decoder
        }
    }
}

fn create_new(slot_fields: &[SlotField], immutable_fields: &[ImmutableField]) -> TokenStream2 {
    let u256 = quote! { ::plutus_evm::revm::primitives::U256 };

    let slot_field_getters: Vec<_> = slot_fields
        .iter()
        .map(|field| {
            let SlotField { slot, ident } = field;

            quote! {
                #ident: ::plutus_evm::storage::FromStorageValue::from_storage_value(
                    storage.get(#u256::from_limbs([#slot, 0, 0, 0]), evm)
                ),
            }
        })
        .collect();

    let immutable_field_getters: Vec<_> = immutable_fields
        .iter()
        .map(|field| {
            let ImmutableField { call, ident } = field;

            quote! {
                #ident: evm.call(address, #call::new(()))?.output._0,
            }
        })
        .collect();

    quote! {
        fn new<P: ::plutus_evm::alloy::providers::Provider>(address: Address, evm: &mut EVM<P>) -> Result<Self, ::plutus_evm::errors::EvmCallError<P>> {
            let mut storage = ::plutus_evm::storage::SmartContractStorage::new(address);

            Ok(
                Self {
                    address,
                    #(#immutable_field_getters)*
                    #(#slot_field_getters)*
                    __storage: storage
                }
            )
        }
    }
}

fn create_decoder(fields: &[SlotField]) -> TokenStream2 {
    let u256 = quote! { ::plutus_evm::revm::primitives::U256 };

    let individual_decoders: Vec<_> = fields.iter().map(|field| {
        let SlotField { slot, ident } = field;

        quote! {
            _ if slot == #u256::from_limbs([#slot, 0, 0, 0]) => { self.#ident = ::plutus_evm::storage::FromStorageValue::from_storage_value(value) }
        }
    }).collect();

    quote! {
        fn apply_storage_changes(&mut self, changes: ::hashbrown::HashMap<#u256, #u256>) {
            for (slot, value) in changes {
                match slot {
                    #(#individual_decoders)*
                    _ => { self.__storage.insert(slot, value) }
                }
            }
        }
    }
}

fn get_slot_fields(fields: &[&SmartContractField]) -> Vec<SlotField> {
    fields
        .iter()
        .filter_map(|field| {
            let slot = field.slot?;

            Some(SlotField {
                slot: slot.0,
                ident: field.ident.clone().unwrap(),
            })
        })
        .collect()
}

fn get_immutable_fields(fields: &[&SmartContractField]) -> Vec<ImmutableField> {
    fields
        .iter()
        .filter_map(|field| {
            let call = field.immutable.clone()?;

            Some(ImmutableField {
                call: call.0,
                ident: field.ident.clone().unwrap(),
            })
        })
        .collect()
}
