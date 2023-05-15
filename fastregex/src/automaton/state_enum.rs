use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use regexlib::automata::{Automaton, State};

pub struct StateEnum {
    states: Vec<Ident>,
}

impl StateEnum {
    pub fn new(automaton: &Automaton) -> Self {
        Self {
            states: automaton
                .states()
                .iter()
                .map(|state| Ident::new(&format!("State{}", state.id), Span::call_site()))
                .collect(),
        }
    }

    pub fn reference_id(&self, id: usize) -> TokenStream {
        let ident = &self.states[id];
        quote!(State::#ident)
    }

    pub fn reference_state(&self, state: &State) -> TokenStream {
        self.reference_id(state.id)
    }
}

impl ToTokens for StateEnum {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let states = &self.states;
        tokens.append_all(quote! {
            enum State{
                #(#states),*
            }
        })
    }
}
