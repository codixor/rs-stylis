use convert_case::Casing;
use proc_macro::{self, TokenStream};
use proc_macro2::{Literal, Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Data, DataEnum, DeriveInput, Fields, Ident, Type, parse_macro_input};

use crate::parse::CssOptions;

pub fn derive_to_css(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident,
        data,
        generics,
        attrs,
        ..
    } = parse_macro_input!(input);

    let opts = CssOptions::parse_attributes(&attrs).unwrap();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let imp = match &data {
        Data::Enum(data) => derive_enum(data, &opts),
        _ => todo!(),
    };

    let output = quote! {
      impl #impl_generics ToCss for #ident #ty_generics #where_clause {
        fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
        where
          W: std::fmt::Write,
        {
          #imp
        }
      }
    };

    output.into()
}

fn derive_enum(data: &DataEnum, opts: &CssOptions) -> TokenStream2 {
    let variants =
        data.variants
            .iter()
            .map(|variant| {
                let name = &variant.ident;
                let fields = variant
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        field.ident.as_ref().map_or_else(
                            || Ident::new(&format!("_{}", index), Span::call_site()),
                            |ident| ident.clone(),
                        )
                    })
                    .collect::<Vec<_>>();

                let writes = if fields.is_empty() {
                    quote! { Ok(()) }
                } else if fields.len() == 1 {
                    let field = &fields[0];
                    quote! { #field.to_css(dest) }
                } else {
                    let writes = variant.fields.iter().zip(&fields).enumerate().map(
                        |(index, (field, name))| {
                            let mark_written =
                                (index + 1 < fields.len()).then(|| quote! { separator = " "; });
                            if is_option(&field.ty) {
                                quote! {
                                  if let Some(value) = #name {
                                    dest.write_str(separator)?;
                                    value.to_css(dest)?;
                                    #mark_written
                                  }
                                }
                            } else {
                                quote! {
                                  dest.write_str(separator)?;
                                  #name.to_css(dest)?;
                                  #mark_written
                                }
                            }
                        },
                    );
                    quote! { let mut separator = ""; #(#writes)* Ok(()) }
                };

                match &variant.fields {
                    Fields::Unit => {
                        let s = Literal::string(&variant.ident.to_string().to_case(opts.case));
                        quote! {
                          Self::#name => dest.write_str(#s)
                        }
                    }
                    Fields::Named(_) => {
                        quote! {
                          Self::#name { #(#fields),* } => {
                            #writes
                          }
                        }
                    }
                    Fields::Unnamed(_) => {
                        quote! {
                          Self::#name(#(#fields),*) => {
                            #writes
                          }
                        }
                    }
                }
            })
            .collect::<Vec<_>>();

    let output = quote! {
      match self {
        #(#variants),*
      }
    };

    output
}

fn is_option(ty: &Type) -> bool {
    matches!(ty, Type::Path(p) if p.qself.is_none() && p.path.segments.last().is_some_and(|s| s.ident == "Option"))
}
