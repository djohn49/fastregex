use proc_macro::TokenStream;

use quote::quote;
use syn::parse_macro_input;

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

    let function_name = matcher_declaration.function_name;
    quote!(
        fn #function_name(string: impl ::core::convert::AsRef<str>) -> bool{
            #emittable_automata

            let string = ::core::convert::AsRef::as_ref(&string);
            let mut chars = str::chars(string);

            let mut automaton = Automoton::new();

            while let Some(char) = chars.next(){
                automaton = automaton.advance(char);
            }

            automaton.is_terminated()
        }
    )
    .into()
}