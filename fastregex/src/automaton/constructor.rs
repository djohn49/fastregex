use crate::automaton::state_enum::StateEnum;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use regexlib::automata::Automaton;
use syn::{Lit, LitInt};

pub struct AutomatonConstructor {
    initial_states: Vec<TokenStream>,
    initial_states_count: Lit,
}

impl AutomatonConstructor {
    pub fn new(automaton: &Automaton, state_enum: &StateEnum) -> Self {
        let mut initial_states: Vec<_> = automaton
            .start_states()
            .iter()
            .map(|start_state_id| state_enum.reference_id(*start_state_id))
            .collect();

        while initial_states.len() < automaton.state_count() {
            initial_states.push(state_enum.reference_id(0));
        }

        let initial_states_count = Lit::Int(LitInt::new(
            &format!("{}", automaton.start_states().len()),
            Span::call_site(),
        ));

        Self {
            initial_states,
            initial_states_count,
        }
    }
}

impl ToTokens for AutomatonConstructor {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let initial_states = &self.initial_states;
        let initial_states_count = &self.initial_states_count;

        tokens.append_all(quote! {
            pub fn new() -> Self{
                Self{
                    states: [#(#initial_states),*],
                    valid_state_count: #initial_states_count
                }
            }
        });
    }
}
