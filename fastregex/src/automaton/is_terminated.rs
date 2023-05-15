use crate::automaton::state_enum::StateEnum;
use proc_macro2::TokenStream;
use quote::quote;
use regexlib::automata::Automaton;

pub fn emit_is_terminated_function(automata: &Automaton, state_enum: &StateEnum) -> TokenStream {
    let terminal_state_match_arms = automata
        .terminal_state_ids()
        .iter()
        .map(|terminal_state_id| {
            let state_identifier = state_enum.reference_id(*terminal_state_id);
            quote! {#state_identifier => return true,}
        })
        .collect::<Vec<_>>();

    quote! {
        fn is_terminated(&self) -> bool{
            for from_state in self.states.iter().take(self.valid_state_count){
                match from_state{
                    #(#terminal_state_match_arms)*
                    _ => {}
                }
            }

            false
        }
    }
}
