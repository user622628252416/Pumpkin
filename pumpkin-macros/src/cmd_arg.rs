use proc_macro::TokenStream;
use quote::ToTokens;

struct FindArgAttr {
    ty: syn::Type,
    pat: syn::Pat,
    pat_body: Box<syn::Expr>,
}

impl syn::parse::Parse for FindArgAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ty: syn::Type = input.parse()?;
        input.parse::<syn::Token![,]>()?;
        let arm: syn::Arm = input.parse()?;
        Ok(FindArgAttr { ty, pat: arm.pat, pat_body: arm.body, })
    }
}

pub(crate) fn find_arg_impl(attr: TokenStream, item: TokenStream) -> TokenStream {

    let item = syn::parse_macro_input!(item as syn::DeriveInput);
    let struct_def = item.clone().into_token_stream();
    let ident = item.ident.into_token_stream();

    let attr: FindArgAttr = syn::parse_macro_input!(attr as FindArgAttr);
    let ty = attr.ty.into_token_stream();
    let pat = attr.pat.into_token_stream();
    let pat_body = attr.pat_body.into_token_stream();

    let tokens: proc_macro2::TokenStream = quote::quote! {
        #struct_def

        impl <'a> FindArg<'a> for #ident {
            type Data = #ty;
        
            fn find_optional_arg(args: &'a super::ConsumedArgs, name: &'a str) -> Option<Result<Self::Data, CommandError>> {
                match args.get(name) {
                    Some( #pat ) => Some(Ok( #pat_body )),
                    Some(_) => Some(Err(CommandError::InvalidConsumption(Some(name.to_string())))),
                    None => None
                }
            }
        }
    };

    tokens.into()
}