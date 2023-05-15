use proc_macro::TokenStream;
use proc_macro2::Span;

use quote::quote;
use syn::{parse_macro_input, Lit, LitInt, LitStr};

use regexlib::automata::Automaton;
use regexlib::parser::RegexEntry;

use crate::automata::EmittableAutomaton;
use crate::matcher_declaration::MatcherDeclaration;

mod automata;
mod matcher_declaration;

#[proc_macro]
pub fn matcher(input: TokenStream) -> TokenStream {
    let matcher_declaration = parse_macro_input!(input as MatcherDeclaration);
    let regex = match RegexEntry::parse(&matcher_declaration.regex) {
        Ok(regex) => regex,
        Err(e) => {
            return syn::parse::Error::new(
                matcher_declaration.regex_span,
                format!("Failed to parse as regex: {}", e),
            )
            .to_compile_error()
            .into();
        }
    };

    let automaton = {
        let mut automaton = Automaton::from_regex(regex);
        automaton.simplify();
        automaton
    };

    let emittable_automata = EmittableAutomaton::new(&automaton);

    let prefix_check = if automaton.prefix().is_empty() {
        quote!()
    } else {
        let prefix_literal = Lit::Str(LitStr::new(automaton.prefix(), Span::call_site()));
        let prefix_length_literal = Lit::Int(LitInt::new(
            &format!("{}", automaton.prefix().len()),
            Span::call_site(),
        ));

        quote! {
            if string.len() < #prefix_length_literal{
                return false;
            }

            let (prefix, string) = string.split_at(#prefix_length_literal);

            if prefix != #prefix_literal{
                return false;
            }
        }
    };

    let function_name = matcher_declaration.function_name;
    quote!(
        fn #function_name(string: impl ::core::convert::AsRef<str>) -> bool{
            #emittable_automata

            let string = ::core::convert::AsRef::as_ref(&string);
            #prefix_check
            let mut chars = str::chars(string);

            let mut automaton = Automoton::new();

            while let Some(char) = chars.next(){
                automaton = automaton.advance(char);

                if(automaton.is_failed()){
                    return false;
                }
            }

            automaton.is_terminated()
        }
    )
    .into()
}
