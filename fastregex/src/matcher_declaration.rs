use proc_macro2::Span;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, Lit, Token};

pub struct MatcherDeclaration {
    pub function_name: Ident,
    pub regex: String,
    pub regex_span: Span,
}

impl Parse for MatcherDeclaration {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let function_name = input.parse()?;

        input.parse::<Token![,]>()?;

        let (regex, regex_span) = {
            let regex = input.parse::<Lit>()?;

            let (regex, regex_span) = match regex {
                Lit::Str(lit_str) => (lit_str.value(), lit_str.span()),
                error => return Err(syn::Error::new(error.span(), "Expected string literal")),
            };

            (regex, regex_span)
        };

        Ok(Self {
            function_name,
            regex,
            regex_span,
        })
    }
}
